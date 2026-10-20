use sourceneo_lib::{git, model::StatusKind, service::Service};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use tempfile::TempDir;
fn g(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args(args)
        .current_dir(path)
        .env("GIT_CONFIG_NOSYSTEM", "1")
        .env(
            "GIT_CONFIG_GLOBAL",
            if cfg!(windows) { "NUL" } else { "/dev/null" },
        )
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GIT_AUTHOR_NAME", "SourceNeo Test")
        .env("GIT_AUTHOR_EMAIL", "test@example.invalid")
        .env("GIT_COMMITTER_NAME", "SourceNeo Test")
        .env("GIT_COMMITTER_EMAIL", "test@example.invalid")
        .output()
        .unwrap();
    assert!(
        out.status.success(),
        "git {:?}: {}",
        args,
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8_lossy(&out.stdout).trim().into()
}
fn commit(path: &Path, file: &str, content: &str) -> String {
    fs::write(path.join(file), content).unwrap();
    g(path, &["add", "--", file]);
    g(
        path,
        &["-c", "commit.gpgsign=false", "commit", "-m", content],
    );
    g(path, &["rev-parse", "HEAD"])
}
struct Fixture {
    _tmp: TempDir,
    root: PathBuf,
    a: PathBuf,
    b: PathBuf,
    remote: PathBuf,
    service: Arc<Service>,
}
impl Fixture {
    fn new() -> Self {
        static ENV: std::sync::Once = std::sync::Once::new();
        ENV.call_once(|| {
            std::env::set_var("GIT_CONFIG_NOSYSTEM", "1");
            std::env::set_var(
                "GIT_CONFIG_GLOBAL",
                if cfg!(windows) { "NUL" } else { "/dev/null" },
            );
        });
        let tmp = TempDir::new().unwrap();
        let root = dunce::canonicalize(tmp.path()).unwrap();
        let remote = root.join("remote.git");
        let a = root.join("alpha");
        let b = root.join("beta");
        g(
            &root,
            &[
                "init",
                "--bare",
                "--initial-branch=main",
                remote.to_str().unwrap(),
            ],
        );
        g(
            &root,
            &["clone", remote.to_str().unwrap(), a.to_str().unwrap()],
        );
        commit(&a, "shared.txt", "base\n");
        g(&a, &["push", "-u", "origin", "main"]);
        g(
            &root,
            &["clone", remote.to_str().unwrap(), b.to_str().unwrap()],
        );
        let service = Arc::new(Service::new(root.join("config/repos.json")).unwrap());
        service.add(a.to_str().unwrap()).unwrap();
        service.add(b.to_str().unwrap()).unwrap();
        Self {
            _tmp: tmp,
            root,
            a,
            b,
            remote,
            service,
        }
    }
    fn status(&self, p: &Path) -> sourceneo_lib::model::RepoStatus {
        self.service.status(p.to_str().unwrap()).unwrap()
    }
    fn sync(&self, p: &Path) -> sourceneo_lib::model::RepoStatus {
        self.service.sync(p.to_str().unwrap()).unwrap()
    }
}
#[test]
fn synced_ahead_push_behind_fast_forward() {
    let f = Fixture::new();
    assert_eq!(f.status(&f.a).kind, StatusKind::Synced);
    let oid = commit(&f.a, "local.txt", "ahead");
    assert_eq!(f.status(&f.a).kind, StatusKind::Ahead);
    let s = f.sync(&f.a);
    assert_eq!(s.kind, StatusKind::Synced);
    assert!(s.last_fetched.is_some());
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), oid);
    g(&f.b, &["fetch"]);
    assert_eq!(f.status(&f.b).kind, StatusKind::Behind);
    let s = f.sync(&f.b);
    assert_eq!(s.kind, StatusKind::Synced);
    assert_eq!(g(&f.b, &["rev-parse", "HEAD"]), oid);
}
#[test]
fn divergence_does_not_merge_or_rebase() {
    let f = Fixture::new();
    let local = commit(&f.a, "local.txt", "local");
    let remote = commit(&f.b, "remote.txt", "remote");
    g(&f.b, &["push"]);
    let s = f.sync(&f.a);
    assert_eq!(s.kind, StatusKind::Diverged);
    assert!(s.conflicts.is_empty());
    assert_eq!(s.ahead, 1);
    assert_eq!(s.behind, 1);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), local);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
    assert!(!f.a.join(".git/MERGE_HEAD").exists());
}
#[test]
fn real_conflict_is_separate_and_writes_are_paused() {
    let f = Fixture::new();
    let head = commit(&f.a, "shared.txt", "local edit\n");
    commit(&f.b, "shared.txt", "remote edit\n");
    g(&f.b, &["push"]);
    g(&f.a, &["fetch"]);
    let out = Command::new("git")
        .args([
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "merge",
            "origin/main",
        ])
        .current_dir(&f.a)
        .output()
        .unwrap();
    assert!(!out.status.success());
    let s = f.status(&f.a);
    assert_eq!(s.kind, StatusKind::Conflict);
    assert_eq!(s.conflicts, vec!["shared.txt"]);
    assert_eq!(s.operation.as_deref(), Some("merge"));
    let before = fs::read(f.a.join("shared.txt")).unwrap();
    assert_eq!(f.sync(&f.a).kind, StatusKind::Conflict);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(f.a.join("shared.txt")).unwrap(), before);
    let diagnostic = f.service.diagnostic(f.a.to_str().unwrap()).unwrap();
    assert!(diagnostic.contains("shared.txt"));
}
#[test]
fn dirty_changes_are_preserved_and_rename_counts_once() {
    let f = Fixture::new();
    commit(&f.b, "remote.txt", "remote change");
    g(&f.b, &["push"]);
    fs::write(f.a.join("shared.txt"), "keep this\n").unwrap();
    let old = g(&f.a, &["rev-parse", "HEAD"]);
    let s = f.sync(&f.a);
    assert_eq!(s.kind, StatusKind::Attention);
    assert_eq!(s.changed, 1);
    assert_eq!(s.behind, 1);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), old);
    assert_eq!(
        fs::read_to_string(f.a.join("shared.txt")).unwrap(),
        "keep this\n"
    );
    fs::write(f.a.join("shared.txt"), "base\n").unwrap();
    g(&f.a, &["mv", "shared.txt", "renamed file.txt"]);
    assert_eq!(f.status(&f.a).changed, 1);
}
#[test]
fn missing_upstream_empty_and_detached_are_actionable() {
    let f = Fixture::new();
    g(&f.a, &["branch", "--unset-upstream"]);
    assert_eq!(f.sync(&f.a).kind, StatusKind::Attention);
    let empty = f.root.join("empty");
    fs::create_dir(&empty).unwrap();
    g(&empty, &["init", "--initial-branch=main"]);
    let r = f.service.add(empty.to_str().unwrap()).unwrap();
    assert_eq!(f.status(&empty).kind, StatusKind::Attention);
    assert!(f.service.history(&r.path, 100).unwrap().commits.is_empty());
    g(&f.b, &["checkout", "--detach"]);
    assert_eq!(f.sync(&f.b).kind, StatusKind::Attention);
}
#[test]
fn worktrees_share_coordination_and_survive_reload() {
    let f = Fixture::new();
    let wt = f.root.join("linked-worktree");
    g(
        &f.a,
        &["worktree", "add", "-b", "feature", wt.to_str().unwrap()],
    );
    let r = f.service.add(wt.to_str().unwrap()).unwrap();
    let main = f
        .service
        .list()
        .into_iter()
        .find(|r| r.path == f.a.to_string_lossy())
        .unwrap();
    assert_eq!(r.common_dir, main.common_dir);
    assert_ne!(r.path, main.path);
    assert_eq!(f.status(&wt).branch, "feature");
    let svc = f.service.clone();
    let path = wt.to_string_lossy().into_owned();
    let a = std::thread::spawn(move || svc.status(&path));
    let svc = f.service.clone();
    let path = f.a.to_string_lossy().into_owned();
    let b = std::thread::spawn(move || svc.sync(&path));
    assert!(a.join().unwrap().is_ok());
    assert!(b.join().unwrap().is_ok());
    let reloaded = Service::new(f.root.join("config/repos.json")).unwrap();
    assert_eq!(reloaded.list().len(), 3);
    assert!(reloaded
        .status(f.a.to_str().unwrap())
        .unwrap()
        .last_fetched
        .is_some());
    reloaded.remove(&r.path).unwrap();
    assert!(wt.exists());
    assert_eq!(reloaded.list().len(), 2);
}
#[test]
fn failed_repo_does_not_prevent_other_repo_sync() {
    let f = Fixture::new();
    g(
        &f.a,
        &[
            "remote",
            "set-url",
            "origin",
            f.root.join("missing-remote.git").to_str().unwrap(),
        ],
    );
    let oid = commit(&f.b, "good.txt", "still push");
    let s1 = f.service.clone();
    let path = f.a.to_string_lossy().into_owned();
    let bad = std::thread::spawn(move || s1.sync(&path));
    let s2 = f.service.clone();
    let path = f.b.to_string_lossy().into_owned();
    let good = std::thread::spawn(move || s2.sync(&path));
    assert!(bad.join().unwrap().is_err());
    assert_eq!(good.join().unwrap().unwrap().kind, StatusKind::Synced);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), oid);
    assert!(f.service.logs().iter().any(|l| l.level == "error"));
    assert_eq!(f.status(&f.a).kind, StatusKind::Error);
}
#[test]
fn existing_git_lock_is_not_deleted() {
    let f = Fixture::new();
    fs::write(f.a.join(".git/index.lock"), "").unwrap();
    let error = f.service.sync(f.a.to_str().unwrap()).unwrap_err();
    assert_eq!(error.kind, "busy");
    assert!(f.a.join(".git/index.lock").exists());
}
#[test]
fn merge_graph_has_real_parents_refs_and_pagination() {
    let f = Fixture::new();
    g(&f.a, &["checkout", "-b", "feature"]);
    let feature = commit(&f.a, "feature.txt", "feature change");
    g(&f.a, &["checkout", "main"]);
    let main = commit(&f.a, "main.txt", "main change");
    g(
        &f.a,
        &[
            "-c",
            "commit.gpgsign=false",
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "merge",
            "--no-ff",
            "feature",
            "-m",
            "merge feature",
        ],
    );
    let h = f.service.history(f.a.to_str().unwrap(), 100).unwrap();
    assert_eq!(h.commits[0].parents, vec![main, feature]);
    assert!(h.commits[0].refs.contains(&"refs/heads/main".into()));
    assert!(h.commits[0].refs.contains(&"HEAD".into()));
    assert!(h
        .commits
        .iter()
        .any(|c| c.refs.contains(&"refs/remotes/origin/main".into())));
    let page = f.service.history(f.a.to_str().unwrap(), 2).unwrap();
    assert_eq!(page.commits.len(), 2);
    assert!(page.has_more);
}
#[test]
fn diagnostic_redacts_credentials() {
    let text="https://alice:my-password@example.com/repo?token=supersecret&access_token=abc ssh://git:secret@host/repo Authorization: Bearer-secret password=hunter2";
    let redacted = git::redact(text);
    for secret in [
        "alice",
        "my-password",
        "supersecret",
        "abc",
        "git:secret",
        "Bearer-secret",
        "hunter2",
    ] {
        assert!(!redacted.contains(secret), "leaked {secret}: {redacted}")
    }
    assert!(redacted.contains("example.com"));
}
#[test]
fn url_credentials_are_redacted_through_last_authority_at_sign() {
    for (input, expected) in [
        (
            "https://probe-user:prefix@MIDDLE@SECRETTAIL@example.invalid/repo.git",
            "https://[redacted]@example.invalid/repo.git",
        ),
        (
            "fatal: unable to access 'https://MIDDLE@SECRETTAIL@127.0.0.1:1/repo.git/': URL rejected: Bad hostname",
            "fatal: unable to access 'https://[redacted]@127.0.0.1:1/repo.git/': URL rejected: Bad hostname",
        ),
        (
            "http://probe-user:prefix%40SECRETTAIL@example.invalid:8080/repo.git",
            "http://[redacted]@example.invalid:8080/repo.git",
        ),
        (
            "ssh://probe-user:prefix@MIDDLE@SECRETTAIL@[::1]:2222/repo.git",
            "ssh://[redacted]@[::1]:2222/repo.git",
        ),
        (
            "https://probe-user@example.invalid/path@revision?owner=a@b#c@d",
            "https://[redacted]@example.invalid/path@revision?owner=a@b#c@d",
        ),
        (
            "https://example.invalid/path@revision?owner=a@b#c@d",
            "https://example.invalid/path@revision?owner=a@b#c@d",
        ),
        (
            "http://example.invalid?owner=a@b https://example.invalid#c@d probe@example.invalid",
            "http://example.invalid?owner=a@b https://example.invalid#c@d probe@example.invalid",
        ),
    ] {
        let redacted = git::redact(input);
        assert_eq!(redacted, expected);
        assert_eq!(git::redact(&redacted), expected);
    }
}
#[test]
fn failed_sync_redacts_url_credentials_in_status_logs_and_diagnostic() {
    let f = Fixture::new();
    let head = g(&f.a, &["rev-parse", "HEAD"]);
    let remote_head = g(&f.remote, &["rev-parse", "main"]);
    g(&f.a, &["config", "credential.helper", ""]);
    g(&f.a, &["config", "http.proxy", ""]);
    g(
        &f.a,
        &[
            "remote",
            "set-url",
            "origin",
            "https://probe-user:prefix@MIDDLE@SECRETTAIL@127.0.0.1:1/repo.git",
        ],
    );

    let path = f.a.to_str().unwrap();
    let error = f.service.sync(path).unwrap_err();
    let status = f.service.status(path).unwrap();
    let logs = f.service.logs();
    let diagnostic = f.service.diagnostic(path).unwrap();
    assert_eq!(status.kind, StatusKind::Error);
    assert!(status.error_kind.is_some());
    assert!(status.last_fetched.is_none());
    assert!(logs.iter().any(|entry| entry.level == "error"));
    for output in [
        error.message,
        status.message,
        serde_json::to_string(&logs).unwrap(),
        diagnostic,
    ] {
        for secret in ["probe-user", "prefix", "MIDDLE", "SECRETTAIL"] {
            assert!(!output.contains(secret), "leaked {secret}: {output}");
        }
        assert!(output.contains("127.0.0.1:1"));
    }
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), head);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote_head);
    assert_eq!(
        fs::read_to_string(f.a.join("shared.txt")).unwrap(),
        "base\n"
    );
}
#[test]
fn ignored_files_blocking_a_fast_forward_are_preserved() {
    let f = Fixture::new();
    fs::write(f.a.join(".git/info/exclude"), "precious.txt\n").unwrap();
    fs::write(f.a.join("precious.txt"), "local ignored content").unwrap();
    commit(&f.b, "precious.txt", "remote tracked content");
    g(&f.b, &["push"]);
    let old = g(&f.a, &["rev-parse", "HEAD"]);
    assert!(f.service.sync(f.a.to_str().unwrap()).is_err());
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), old);
    assert_eq!(
        fs::read_to_string(f.a.join("precious.txt")).unwrap(),
        "local ignored content"
    );
}
#[test]
fn unfinished_merge_without_conflicts_is_paused() {
    let f = Fixture::new();
    g(&f.a, &["checkout", "-b", "side"]);
    commit(&f.a, "side.txt", "side");
    g(&f.a, &["checkout", "main"]);
    commit(&f.a, "main.txt", "main");
    g(
        &f.a,
        &[
            "-c",
            "user.name=Test",
            "-c",
            "user.email=test@example.invalid",
            "merge",
            "--no-ff",
            "--no-commit",
            "side",
        ],
    );
    let old = g(&f.a, &["rev-parse", "HEAD"]);
    let s = f.sync(&f.a);
    assert!(s.conflicts.is_empty());
    assert_eq!(s.kind, StatusKind::Attention);
    assert_eq!(s.operation.as_deref(), Some("merge"));
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), old);
}
#[test]
fn external_lock_and_cross_instance_common_dir_lock_are_respected() {
    use fs2::FileExt;
    let f = Fixture::new();
    let file = fs::OpenOptions::new()
        .create(true)
        .truncate(false)
        .write(true)
        .read(true)
        .open(f.a.join(".git/sourceneo.lock"))
        .unwrap();
    file.lock_exclusive().unwrap();
    let e = f.service.sync(f.a.to_str().unwrap()).unwrap_err();
    assert_eq!(e.kind, "busy");
}
#[test]
fn branch_push_ignores_unrelated_push_refspecs() {
    let f = Fixture::new();
    g(&f.a, &["branch", "unrelated"]);
    g(
        &f.a,
        &[
            "config",
            "remote.origin.push",
            "+refs/heads/unrelated:refs/heads/unrelated",
        ],
    );
    let oid = commit(&f.a, "push.txt", "current branch only");
    let s = f.sync(&f.a);
    assert_eq!(s.kind, StatusKind::Synced);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), oid);
    let output = Command::new("git")
        .args(["show-ref", "--verify", "refs/heads/unrelated"])
        .current_dir(&f.remote)
        .output()
        .unwrap();
    assert!(!output.status.success());
}
#[cfg(unix)]
#[test]
fn native_pre_push_hooks_are_not_bypassed() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    commit(&f.a, "hook.txt", "hook check");
    let hook = f.a.join(".git/hooks/pre-push");
    fs::write(&hook, "#!/bin/sh\necho 'test hook refused' >&2\nexit 1\n").unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    let old = g(&f.remote, &["rev-parse", "main"]);
    assert!(f.service.sync(f.a.to_str().unwrap()).is_err());
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), old);
    assert!(f
        .service
        .diagnostic(f.a.to_str().unwrap())
        .unwrap()
        .contains("test hook refused"));
}
#[test]
fn upstream_removed_and_subdirectory_duplicates_are_handled() {
    let f = Fixture::new();
    fs::create_dir(f.a.join("nested")).unwrap();
    f.service.add(f.a.join("nested").to_str().unwrap()).unwrap();
    assert_eq!(f.service.list().len(), 2);
    g(&f.a, &["update-ref", "-d", "refs/remotes/origin/main"]);
    let s = f.status(&f.a);
    assert_eq!(s.kind, StatusKind::Attention);
    assert!(s.message.contains("失效"));
}
#[test]
fn nested_repo_losing_git_metadata_never_operates_on_parent() {
    let f = Fixture::new();
    fs::write(f.a.join(".git/info/exclude"), "nested/\n").unwrap();
    let nested = f.a.join("nested");
    fs::create_dir(&nested).unwrap();
    g(&nested, &["init", "--initial-branch=child"]);
    commit(&nested, "child.txt", "child commit");
    let child = f.service.add(nested.to_str().unwrap()).unwrap();
    fs::remove_dir_all(nested.join(".git")).unwrap();
    let parent_head = commit(&f.a, "parent.txt", "parent must not be pushed");
    let remote_head = g(&f.remote, &["rev-parse", "main"]);

    let error = f.service.sync(&child.path).unwrap_err();
    assert_eq!(error.kind, "path");
    assert!(f.service.status(&child.path).is_err());
    assert!(f.service.history(&child.path, 100).is_err());
    assert!(f.service.diagnostic(&child.path).is_err());
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote_head);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), parent_head);
    assert_eq!(f.service.list().last().unwrap().path, child.path);
}
#[test]
fn corruption_does_not_replace_config() {
    let temp = TempDir::new().unwrap();
    let config = temp.path().join("repositories.json");
    fs::write(&config, "broken config").unwrap();
    assert!(Service::new(config.clone()).is_err());
    assert_eq!(fs::read_to_string(config).unwrap(), "broken config");
}
#[test]
fn utf8_spaces_and_multiline_subjects_parse_without_losing_topology() {
    let f = Fixture::new();
    commit(&f.a, "中文 file.txt", "中文提交主题");
    let s = f.status(&f.a);
    assert_eq!(s.changed, 0);
    let h = f.service.history(f.a.to_str().unwrap(), 100).unwrap();
    assert_eq!(h.commits[0].subject, "中文提交主题");
    fs::write(f.a.join("中文 file.txt"), "dirty").unwrap();
    assert_eq!(f.status(&f.a).changed, 1);
}
#[test]
fn untracked_directories_count_individual_files() {
    let f = Fixture::new();
    fs::create_dir(f.a.join("drafts")).unwrap();
    fs::write(f.a.join("drafts/a.txt"), "a").unwrap();
    fs::write(f.a.join("drafts/b.txt"), "b").unwrap();
    assert_eq!(f.status(&f.a).changed, 2);
}
#[cfg(unix)]
#[test]
fn timeout_also_covers_pipes_held_by_a_hook_descendant() {
    let temp = TempDir::new().unwrap();
    let start = std::time::Instant::now();
    let e = git::run_timeout(
        temp.path(),
        &[
            "-c",
            "alias.sourceneo-test=!sleep 5 & exit 0",
            "sourceneo-test",
        ],
        std::time::Duration::from_millis(150),
    )
    .unwrap_err();
    assert_eq!(e.kind, "timeout");
    assert!(start.elapsed() < std::time::Duration::from_secs(2));
}
#[test]
fn auth_headers_are_redacted_in_full() {
    let redacted=git::redact("Authorization: Bearer abc123\nhttps://user:secret@host/a?access_token=token123\npassword=hunter2");
    for s in ["abc123", "user:secret", "token123", "hunter2"] {
        assert!(!redacted.contains(s))
    }
}
#[test]
fn shallow_history_exposes_a_clone_boundary() {
    let f = Fixture::new();
    let shallow = f.root.join("shallow");
    let normalized = f.remote.to_string_lossy().replace('\\', "/");
    let url = format!("file:///{}", normalized.trim_start_matches('/'));
    g(
        &f.root,
        &["clone", "--depth=1", &url, shallow.to_str().unwrap()],
    );
    let r = f.service.add(shallow.to_str().unwrap()).unwrap();
    assert!(f.service.history(&r.path, 100).unwrap().shallow);
    assert!(f.status(&shallow).shallow);
}

#[test]
fn signature_display_does_not_corrupt_machine_history() {
    use std::io::Write;
    use std::process::Stdio;
    fn write_commit(path: &Path, raw: &str) -> String {
        let raw = raw.replace("\r\n", "\n");
        let mut child = Command::new("git")
            .args(["hash-object", "-t", "commit", "-w", "--stdin"])
            .current_dir(path)
            .stdin(Stdio::piped())
            .stdout(Stdio::piped())
            .spawn()
            .unwrap();
        child
            .stdin
            .take()
            .unwrap()
            .write_all(raw.as_bytes())
            .unwrap();
        let output = child.wait_with_output().unwrap();
        assert!(output.status.success());
        String::from_utf8(output.stdout).unwrap().trim().into()
    }
    let f = Fixture::new();
    let path = f.root.join("signed-history");
    fs::create_dir(&path).unwrap();
    g(&path, &["init", "--initial-branch=main"]);
    g(&path, &["hash-object", "-t", "tree", "-w", "--stdin"]);
    let base = write_commit(&path, include_str!("fixtures/signature-base.commit"));
    assert_eq!(base, "d2968426b7d33e569502c115a3577519c6dcc542");
    let tip = write_commit(&path, include_str!("fixtures/signature-tip.commit"));
    g(&path, &["update-ref", "refs/heads/main", &tip]);
    g(&path, &["tag", "signed-tip", &tip]);
    g(&path, &["config", "log.showSignature", "true"]);
    // Machine history must not depend on an installed signature verifier.
    g(
        &path,
        &[
            "config",
            "gpg.ssh.program",
            f.root.join("missing-signature-verifier").to_str().unwrap(),
        ],
    );
    let repo = f.service.add(path.to_str().unwrap()).unwrap();
    let history = f.service.history(&repo.path, 100).unwrap();
    assert_eq!(history.commits.len(), 2);
    assert_eq!(history.commits[0].hash, tip);
    assert_eq!(history.commits[0].parents, vec![base.clone()]);
    assert_eq!(history.commits[0].subject, "signature test tip");
    for label in ["HEAD", "refs/heads/main", "refs/tags/signed-tip"] {
        assert!(history.commits[0].refs.iter().any(|r| r == label));
    }
    assert_eq!(history.commits[1].hash, base);
    assert!(history.commits[1].parents.is_empty());
}

#[test]
fn invalid_working_directories_are_path_errors() {
    let tmp = TempDir::new().unwrap();
    let file = tmp.path().join("ordinary-file");
    fs::write(&file, "not a directory").unwrap();
    for path in [tmp.path().join("missing"), file] {
        let error = git::discover(path.to_str().unwrap()).unwrap_err();
        assert_eq!(error.kind, "path");
        assert!(!error.message.contains("PATH"));
    }
}

#[cfg(unix)]
#[test]
fn inaccessible_working_directory_is_a_permission_error() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return; // Root bypasses directory access checks.
    }
    let tmp = TempDir::new().unwrap();
    let path = tmp.path().join("inaccessible");
    fs::create_dir(&path).unwrap();
    fs::set_permissions(&path, fs::Permissions::from_mode(0o000)).unwrap();
    let result = git::run(&path, &["--version"]);
    fs::set_permissions(&path, fs::Permissions::from_mode(0o700)).unwrap();
    let error = result.unwrap_err();
    assert_eq!(error.kind, "permission");
}

#[test]
fn missing_git_is_still_an_environment_error() {
    const CHILD: &str = "SOURCENEO_TEST_MISSING_GIT";
    let tmp = TempDir::new().unwrap();
    if std::env::var_os(CHILD).is_some() {
        let error = git::run(tmp.path(), &["--version"]).unwrap_err();
        assert_eq!(error.kind, "environment");
        assert!(error.message.contains("PATH"));
        return;
    }
    // Keep PATH changes inside a subprocess so parallel Git tests stay isolated.
    let output = Command::new(std::env::current_exe().unwrap())
        .args(["--exact", "missing_git_is_still_an_environment_error"])
        .env(CHILD, "1")
        .env("PATH", tmp.path())
        .current_dir(tmp.path())
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}\n{}",
        String::from_utf8_lossy(&output.stdout),
        String::from_utf8_lossy(&output.stderr)
    );
}

#[cfg(unix)]
#[test]
fn read_only_git_metadata_allows_status_history_and_diagnostic() {
    use std::os::unix::fs::PermissionsExt;
    if unsafe { libc::geteuid() } == 0 {
        return;
    }
    // Cover both a fresh read-only repository and a leftover read-only app lock.
    for existing_lock in [false, true] {
        let f = Fixture::new();
        let dir = f.a.join(".git");
        let lock = dir.join("sourceneo.lock");
        if existing_lock {
            fs::write(&lock, "preserve this lock file").unwrap();
            fs::set_permissions(&lock, fs::Permissions::from_mode(0o444)).unwrap();
        }
        let head = g(&f.a, &["rev-parse", "HEAD"]);
        let original = fs::metadata(&dir).unwrap().permissions();
        fs::set_permissions(&dir, fs::Permissions::from_mode(0o555)).unwrap();
        let path = f.a.to_str().unwrap();
        let status = f.service.status(path);
        let history = f.service.history(path, 100);
        let diagnostic = f.service.diagnostic(path);
        let sync = f.service.sync(path);
        fs::set_permissions(&dir, original).unwrap();
        if existing_lock {
            fs::set_permissions(&lock, fs::Permissions::from_mode(0o644)).unwrap();
        }
        assert_eq!(status.unwrap().kind, StatusKind::Synced);
        assert_eq!(history.unwrap().commits[0].hash, head);
        assert!(diagnostic.unwrap().contains(&head));
        assert_eq!(sync.unwrap_err().kind, "permission");
        assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), head);
        assert_eq!(g(&f.remote, &["rev-parse", "main"]), head);
        if existing_lock {
            assert_eq!(
                fs::read_to_string(&lock).unwrap(),
                "preserve this lock file"
            );
        } else {
            assert!(!lock.exists());
        }
    }
}

#[test]
fn machine_history_uses_utf8_despite_configured_log_encoding() {
    let f = Fixture::new();
    let subject = "修复中文提交历史";
    let author = "测试作者";
    let parent = g(&f.a, &["rev-parse", "HEAD"]);
    g(
        &f.a,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "--allow-empty",
            "--author=测试作者 <encoding@example.invalid>",
            "-m",
            subject,
        ],
    );
    let head = g(&f.a, &["rev-parse", "HEAD"]);
    g(&f.a, &["tag", "encoding-tip"]);
    for encoding in ["GB18030", "GBK"] {
        g(&f.a, &["config", "i18n.logOutputEncoding", encoding]);
        let history = f.service.history(f.a.to_str().unwrap(), 1).unwrap();
        assert_eq!(history.commits.len(), 1);
        assert!(history.has_more);
        let tip = &history.commits[0];
        assert_eq!(tip.hash, head);
        assert_eq!(tip.parents, vec![parent.clone()]);
        assert_eq!(tip.subject, subject, "output encoding: {encoding}");
        assert_eq!(tip.author, author, "output encoding: {encoding}");
        for label in ["HEAD", "refs/heads/main", "refs/tags/encoding-tip"] {
            assert!(tip.refs.iter().any(|r| r == label));
        }
    }
}

#[test]
fn multiple_upstream_merge_targets_never_fetch_or_push() {
    for (first, second) in [("main", "other"), ("other", "main"), ("main", "main")] {
        let f = Fixture::new();
        let base = g(&f.remote, &["rev-parse", "main"]);
        g(&f.a, &["branch", "other"]);
        g(&f.a, &["push", "origin", "other"]);
        let head = commit(
            &f.a,
            "local.txt",
            "local commit must not go to another branch",
        );
        g(
            &f.a,
            &[
                "config",
                "--replace-all",
                "branch.main.merge",
                &format!("refs/heads/{first}"),
            ],
        );
        g(
            &f.a,
            &[
                "config",
                "--add",
                "branch.main.merge",
                &format!("refs/heads/{second}"),
            ],
        );
        let before = f.status(&f.a);
        assert_eq!(
            before.upstream.as_deref(),
            Some(format!("origin/{first}").as_str())
        );
        assert_eq!(before.ahead, 1);
        fs::write(
            f.a.join(".git/FETCH_HEAD"),
            "preserve existing fetch metadata\n",
        )
        .unwrap();
        let error = f.service.sync(f.a.to_str().unwrap()).unwrap_err();
        assert_eq!(error.kind, "configuration");
        assert!(error.message.contains("多个"));
        assert!(error.message.contains("upstream"));
        assert_eq!(g(&f.remote, &["rev-parse", "main"]), base);
        assert_eq!(g(&f.remote, &["rev-parse", "other"]), base);
        assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), head);
        assert_eq!(
            fs::read_to_string(f.a.join(".git/FETCH_HEAD")).unwrap(),
            "preserve existing fetch metadata\n"
        );
        assert_eq!(
            fs::read_to_string(f.a.join("local.txt")).unwrap(),
            "local commit must not go to another branch"
        );
        let after = f.status(&f.a);
        assert_eq!(after.upstream, before.upstream);
        assert_eq!(after.ahead, 1);
        assert!(after.last_fetched.is_none());
        assert_eq!(after.error_kind.as_deref(), Some("configuration"));
        assert!(f
            .service
            .diagnostic(f.a.to_str().unwrap())
            .unwrap()
            .contains("多个"));
    }
}

#[test]
fn single_upstream_sync_recovers_after_ambiguous_configuration_is_fixed() {
    let f = Fixture::new();
    let base = g(&f.remote, &["rev-parse", "main"]);
    g(&f.a, &["branch", "other"]);
    g(&f.a, &["push", "origin", "other"]);
    let head = commit(&f.a, "local.txt", "push only after upstream is unambiguous");
    g(
        &f.a,
        &["config", "--add", "branch.main.merge", "refs/heads/other"],
    );
    let error = f.service.sync(f.a.to_str().unwrap()).unwrap_err();
    assert_eq!(error.kind, "configuration");
    g(
        &f.a,
        &[
            "config",
            "--replace-all",
            "branch.main.merge",
            "refs/heads/main",
        ],
    );
    let after = f.sync(&f.a);
    assert_eq!(after.kind, StatusKind::Synced);
    assert!(after.error_kind.is_none());
    assert!(after.last_fetched.is_some());
    assert_eq!(after.upstream.as_deref(), Some("origin/main"));
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), head);
    assert_eq!(g(&f.remote, &["rev-parse", "other"]), base);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), head);
}

#[test]
fn missing_padded_input_falls_back_to_trimmed_existing_repository() {
    let f = Fixture::new();
    let padded = format!("{}  ", f.a.display());
    let added = f.service.add(&padded).unwrap();
    assert_eq!(added.path, f.a.to_string_lossy());
    assert_eq!(f.service.list().len(), 2);
    assert_eq!(
        f.service.status(&added.path).unwrap().kind,
        StatusKind::Synced
    );
    #[cfg(unix)]
    {
        let padded = format!(" \t{} \n", f.a.display());
        assert!(!Path::new(&padded).exists());
        assert_eq!(f.service.add(&padded).unwrap().path, added.path);
    }
}

#[cfg(unix)]
#[test]
fn existing_space_suffixed_repository_wins_over_trimmed_sibling() {
    let f = Fixture::new();
    let ordinary_head = commit(
        &f.a,
        "ordinary.txt",
        "keep ordinary repository local commit",
    );
    let spaced = f.root.join("alpha ");
    g(
        &f.root,
        &[
            "clone",
            f.remote.to_str().unwrap(),
            spaced.to_str().unwrap(),
        ],
    );
    let spaced_head = commit(&spaced, "spaced.txt", "push the exact selected repository");
    let added = f.service.add(spaced.to_str().unwrap()).unwrap();
    assert_eq!(added.path, spaced.to_string_lossy());
    assert_eq!(
        added.common_dir,
        dunce::canonicalize(spaced.join(".git"))
            .unwrap()
            .to_string_lossy()
    );
    assert_eq!(f.service.status(&added.path).unwrap().head, spaced_head);
    assert_eq!(
        f.service.history(&added.path, 100).unwrap().commits[0].hash,
        spaced_head
    );
    let reloaded = Service::new(f.root.join("config/repos.json")).unwrap();
    assert_eq!(reloaded.sync(&added.path).unwrap().kind, StatusKind::Synced);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), spaced_head);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), ordinary_head);
    assert!(!f.a.join("spaced.txt").exists());
    assert_eq!(
        fs::read_to_string(f.a.join("ordinary.txt")).unwrap(),
        "keep ordinary repository local commit"
    );
}

#[cfg(unix)]
#[test]
fn separate_git_directory_preserves_trailing_space_and_real_git_locks() {
    let f = Fixture::new();
    let work = f.root.join("separate-work");
    let metadata = f.root.join("metadata ");
    let sibling = f.root.join("metadata");
    g(&f.root, &["init", "--bare", sibling.to_str().unwrap()]);
    g(
        &f.root,
        &[
            "init",
            "--initial-branch=main",
            "--separate-git-dir",
            metadata.to_str().unwrap(),
            work.to_str().unwrap(),
        ],
    );
    let head = commit(&work, "file.txt", "separate metadata commit");
    let added = f.service.add(work.to_str().unwrap()).unwrap();
    let expected = dunce::canonicalize(&metadata).unwrap();
    assert_eq!(added.common_dir, expected.to_string_lossy());
    assert_eq!(git::git_dir(&added).unwrap(), expected);
    assert_eq!(
        f.service.history(&added.path, 100).unwrap().commits[0].hash,
        head
    );
    fs::write(metadata.join("index.lock"), "existing lock").unwrap();
    assert_eq!(f.service.sync(&added.path).unwrap_err().kind, "busy");
    assert_eq!(
        fs::read_to_string(metadata.join("index.lock")).unwrap(),
        "existing lock"
    );
}

#[cfg(unix)]
#[test]
fn existing_padded_non_directory_is_not_replaced_by_trimmed_repository() {
    let f = Fixture::new();
    let file = f.root.join("alpha ");
    fs::write(&file, "preserve existing path").unwrap();
    let error = f.service.add(file.to_str().unwrap()).unwrap_err();
    assert_eq!(error.kind, "path");
    assert!(error.message.contains("不是目录"));
    assert_eq!(fs::read_to_string(&file).unwrap(), "preserve existing path");
    assert_eq!(f.service.list().len(), 2);
}
