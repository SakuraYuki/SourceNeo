use sourceneo_lib::{acquisition, model::HistoryQuery, service::Service};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
};
use tempfile::TempDir;

fn git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Feature Test",
            "-c",
            "user.email=feature@example.test",
            "-c",
            "commit.gpgsign=false",
            "-c",
            "core.hooksPath=/dev/null",
        ])
        .args(args)
        .current_dir(path)
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "{:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}
fn repo(root: &Path, name: &str) -> PathBuf {
    let path = root.join(name);
    fs::create_dir_all(&path).unwrap();
    git(&path, &["init", "-b", "main"]);
    fs::write(path.join("file.txt"), "base\n").unwrap();
    git(&path, &["add", "."]);
    git(&path, &["commit", "-m", "initial"]);
    path
}
fn query(field: &str, text: &str) -> HistoryQuery {
    HistoryQuery {
        field: field.into(),
        text: text.into(),
    }
}

#[test]
fn history_pages_pin_refs_and_search_unloaded_commits() {
    let temp = TempDir::new().unwrap();
    let path = repo(temp.path(), "repo");
    let tree = git(&path, &["rev-parse", "HEAD^{tree}"]);
    let first = git(&path, &["rev-parse", "HEAD"]);
    let mut head = first.clone();
    for i in 0..120 {
        head = git(
            &path,
            &[
                "commit-tree",
                &tree,
                "-p",
                &head,
                "-m",
                &format!("page {i}"),
            ],
        );
    }
    git(&path, &["update-ref", "refs/heads/main", &head]);
    let service = Service::new(temp.path().join("config/repositories.json")).unwrap();
    let repo = service.add(path.to_str().unwrap()).unwrap();
    let page = service
        .history_page(&repo.path, false, None, 0, HistoryQuery::default())
        .unwrap();
    assert!(service
        .history_page(
            &repo.path,
            false,
            Some(&page.snapshot),
            5000,
            HistoryQuery::default()
        )
        .is_err());
    assert_eq!(page.commits.len(), 100);
    assert!(page.has_more);
    let new_head = git(
        &path,
        &[
            "commit-tree",
            &tree,
            "-p",
            &head,
            "-m",
            "new after snapshot",
        ],
    );
    git(&path, &["update-ref", "refs/heads/main", &new_head]);
    let second = service
        .history_page(
            &repo.path,
            false,
            Some(&page.snapshot),
            100,
            HistoryQuery::default(),
        )
        .unwrap();
    assert_eq!(second.commits.len(), 21);
    assert!(!second.has_more);
    assert!(!second
        .commits
        .iter()
        .any(|c| page.commits.iter().any(|p| p.hash == c.hash)));
    assert_eq!(second.commits.last().unwrap().hash, first);
    let search = service
        .history_page(&repo.path, true, None, 0, query("message", "initial"))
        .unwrap();
    assert_eq!(search.commits[0].hash, first);
    assert_eq!(
        service
            .history_page(&repo.path, true, None, 0, query("sha", &first[..10]))
            .unwrap()
            .commits
            .len(),
        1
    );
    assert!(service
        .history_page(&repo.path, true, None, 0, query("sha", "--all"))
        .is_err());
    assert!(service
        .history_page(
            &repo.path,
            true,
            Some("unknown"),
            0,
            HistoryQuery::default()
        )
        .is_err());
    let other = service.add(repo_fn(temp.path()).to_str().unwrap()).unwrap();
    assert!(service
        .history_page(
            &other.path,
            true,
            Some(&page.snapshot),
            0,
            HistoryQuery::default()
        )
        .is_err());
}
fn repo_fn(root: &Path) -> PathBuf {
    repo(root, "other")
}

#[test]
fn scan_skips_dependencies_and_links_and_honors_depth() {
    let temp = TempDir::new().unwrap();
    let a = repo(temp.path(), "a");
    repo(temp.path(), "nested/b");
    repo(temp.path(), "node_modules/hidden");
    #[cfg(unix)]
    std::os::unix::fs::symlink(&a, temp.path().join("alias")).unwrap();
    git(
        &a,
        &[
            "worktree",
            "add",
            "-b",
            "feature",
            temp.path().join("worktree").to_str().unwrap(),
        ],
    );
    let scan = acquisition::scan(temp.path().to_str().unwrap(), 1).unwrap();
    assert_eq!(scan.repos.len(), 2);
    assert_eq!(
        acquisition::scan(temp.path().to_str().unwrap(), 2)
            .unwrap()
            .repos
            .len(),
        3
    );
    assert!(acquisition::scan(temp.path().to_str().unwrap(), 7).is_err());
}

#[test]
fn clone_validation_never_overwrites_and_rejects_local_or_credential_urls() {
    let temp = TempDir::new().unwrap();
    let parent = temp.path().to_str().unwrap();
    for url in [
        "https://example.test/team/repo.git",
        "git@example.test:team/repo.git",
        "ssh://git@example.test:2222/repo",
        "ssh://example.test:2222/repo",
    ] {
        assert_eq!(
            acquisition::clone_target(url, parent, "new").unwrap(),
            dunce::canonicalize(temp.path()).unwrap().join("new")
        );
    }
    for url in [
        "/tmp/local",
        "file:///tmp/repo",
        "ext::echo unsafe",
        "--upload-pack=bad",
        "https://token@example.test/repo",
        "https://example.test/repo?token=secret",
    ] {
        assert!(
            acquisition::clone_target(url, parent, "new").is_err(),
            "{url}"
        );
    }
    for name in ["../escape", ".", "..", "a/b", "a\\b", ""] {
        assert!(acquisition::clone_target("https://example.test/repo", parent, name).is_err());
    }
    fs::create_dir(temp.path().join("existing")).unwrap();
    assert!(acquisition::clone_target("https://example.test/repo", parent, "existing").is_err());
}

#[test]
fn preview_and_comparison_are_read_only_and_logs_survive_restart() {
    let temp = TempDir::new().unwrap();
    let path = repo(temp.path(), "repo");
    let config = temp.path().join("config/repositories.json");
    let service = Service::new(config.clone()).unwrap();
    let r = service.add(path.to_str().unwrap()).unwrap();
    assert!(!service.preview(&r.path).unwrap().blockers.is_empty());
    git(
        &path,
        &["remote", "add", "origin", "https://example.test/repo"],
    );
    git(&path, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    git(&path, &["branch", "--set-upstream-to=origin/main"]);
    git(&path, &["config", "pull.rebase", "true"]);
    fs::write(path.join("file.txt"), "local\n").unwrap();
    git(&path, &["commit", "-am", "local"]);
    let head = git(&path, &["rev-parse", "HEAD"]);
    let preview = service.preview(&r.path).unwrap();
    assert!(preview.blockers.is_empty());
    assert_eq!(preview.status.ahead, 1);
    assert!(preview.policy.contains("rebase"));
    let compare = service.compare(&r.path).unwrap();
    assert_eq!(compare.local.commits.len(), 1);
    assert!(compare.remote.commits.is_empty());
    assert_eq!(compare.files[0].path, "file.txt");
    assert!(service
        .compare_diff(&r.path, &compare.head, &compare.target, "file.txt")
        .unwrap()
        .contains("+local"));
    assert!(service
        .compare_diff(&r.path, &compare.head, &compare.target, "../outside")
        .is_err());
    fs::write(path.join("dirty"), "keep").unwrap();
    assert!(!service.preview(&r.path).unwrap().blockers.is_empty());
    assert_eq!(git(&path, &["rev-parse", "HEAD"]), head);
    assert!(!path.join(".git/sourceneo.lock").exists());
    let logs = service.logs();
    assert!(!logs.is_empty());
    drop(service);
    let restarted = Service::new(config).unwrap();
    assert_eq!(restarted.logs().len(), logs.len());
}

#[cfg(unix)]
#[test]
fn clone_over_local_ssh_adapter_adds_repo_and_preserves_failed_destination() {
    if let Some(root) = std::env::var_os("SOURCENEO_CLONE_TEST_ROOT") {
        let root = PathBuf::from(root);
        let service = Service::new(root.join("config/repositories.json")).unwrap();
        let cloned = service
            .clone_repo("git@example.test:source", root.to_str().unwrap(), "clone")
            .unwrap();
        assert!(Path::new(&cloned.path).join("file.txt").exists());
        assert_eq!(service.list().len(), 1);
        assert!(service
            .clone_repo("git@example.test:source", root.to_str().unwrap(), "clone")
            .is_err());
        assert!(service
            .clone_repo("git@example.test:fail", root.to_str().unwrap(), "failed")
            .is_err());
        assert!(root.join("failed").is_dir());
        assert_eq!(service.list().len(), 1);
        assert!(service.progress().is_empty());
        std::thread::scope(|scope| {
            let handle = scope.spawn(|| {
                service.clone_with_id(
                    "cancel-test",
                    "git@example.test:slow",
                    root.to_str().unwrap(),
                    "cancelled",
                )
            });
            for _ in 0..200 {
                if service
                    .progress()
                    .iter()
                    .any(|p| p.path.ends_with("cancelled"))
                {
                    break;
                }
                std::thread::sleep(std::time::Duration::from_millis(10));
            }
            let started = std::time::Instant::now();
            service.cancel_clone("cancel-test").unwrap();
            let error = handle.join().unwrap().unwrap_err();
            assert_eq!(error.kind, "cancelled");
            assert!(started.elapsed().as_secs() < 2);
            assert!(root.join("cancelled").is_dir());
        });
        assert_eq!(service.list().len(), 1);
        assert!(service.progress().is_empty());
        assert!(service
            .logs()
            .iter()
            .any(|l| l.operation.as_deref() == Some("克隆") && l.elapsed_ms.is_some()));
        return;
    }
    let temp = TempDir::new().unwrap();
    let source = repo(temp.path(), "source");
    let script = temp.path().join("ssh-test.sh");
    fs::write(&script, "#!/bin/sh\ncase \"$*\" in *fail*) exit 1;; *slow*) sleep 5;; esac\nexec git-upload-pack \"$SOURCENEO_CLONE_TEST_SOURCE\"\n").unwrap();
    let output = Command::new(std::env::current_exe().unwrap())
        .args([
            "--exact",
            "clone_over_local_ssh_adapter_adds_repo_and_preserves_failed_destination",
            "--nocapture",
        ])
        .env("SOURCENEO_CLONE_TEST_ROOT", temp.path())
        .env("SOURCENEO_CLONE_TEST_SOURCE", source)
        .env("GIT_SSH_COMMAND", format!("sh '{}'", script.display()))
        .env("GIT_SSH_VARIANT", "ssh")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{} {}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}
