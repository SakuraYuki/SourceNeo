//! Disposable local-only benchmark. Run with --no-default-features --example benchmark -- 20 1000.
use sourceneo_lib::{model::HistoryQuery, service::Service};
use std::{fs, path::Path, process::Command, sync::Arc, time::Instant};
fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Benchmark",
            "-c",
            "user.email=benchmark@example.test",
            "-c",
            "commit.gpgsign=false",
        ])
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn main() {
    let args: Vec<_> = std::env::args().collect();
    let count = args
        .get(1)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(20)
        .clamp(1, 200);
    let commits = args
        .get(2)
        .and_then(|v| v.parse::<usize>().ok())
        .unwrap_or(1000)
        .clamp(100, 5000);
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    let seed = root.join("seed");
    fs::create_dir(&seed).unwrap();
    git(&seed, &["init", "-b", "main"]);
    fs::write(seed.join("file"), "benchmark\n").unwrap();
    git(&seed, &["add", "."]);
    git(&seed, &["commit", "-m", "initial"]);
    let tree = git(&seed, &["rev-parse", "HEAD^{tree}"]);
    let mut head = git(&seed, &["rev-parse", "HEAD"]);
    for i in 1..commits {
        head = git(
            &seed,
            &[
                "commit-tree",
                &tree,
                "-p",
                &head,
                "-m",
                &format!("commit {i}"),
            ],
        );
    }
    git(&seed, &["update-ref", "refs/heads/main", &head]);
    let service = Arc::new(Service::new(root.join("config/repositories.json")).unwrap());
    for i in 0..count {
        let path = root.join(format!("repo-{i}"));
        git(
            root,
            &["clone", seed.to_str().unwrap(), path.to_str().unwrap()],
        );
        service.add(path.to_str().unwrap()).unwrap();
    }
    let repos = service.list();
    let mut sweeps = vec![];
    for _ in 0..3 {
        let start = Instant::now();
        std::thread::scope(|scope| {
            for chunk in repos.chunks(count.div_ceil(3)) {
                let service = &service;
                scope.spawn(move || {
                    for repo in chunk {
                        service.status(&repo.path).unwrap();
                    }
                });
            }
        });
        sweeps.push(start.elapsed().as_millis());
    }
    let start = Instant::now();
    let mut cumulative_rows = 0;
    for limit in (100..=commits).step_by(100) {
        cumulative_rows += service
            .history(&repos[0].path, limit, false)
            .unwrap()
            .commits
            .len();
    }
    let legacy_ms = start.elapsed().as_millis();
    let start = Instant::now();
    let mut snapshot = None;
    let mut rows = 0;
    loop {
        let page = service
            .history_page(
                &repos[0].path,
                false,
                snapshot.as_deref(),
                rows,
                HistoryQuery::default(),
            )
            .unwrap();
        rows += page.commits.len();
        snapshot = Some(page.snapshot);
        if !page.has_more {
            break;
        }
    }
    println!("{}", serde_json::to_string_pretty(&serde_json::json!({"platform": std::env::consts::OS, "repos": count, "commits": commits, "status_sweep_ms": sweeps, "legacy_history_ms": legacy_ms, "paged_history_ms": start.elapsed().as_millis(), "legacy_transferred_rows": cumulative_rows, "paged_transferred_rows": rows, "network": false})).unwrap());
}
