use sourceneo_lib::{git, model::StatusKind, service::Service};
use std::{
    fs,
    path::{Path, PathBuf},
    process::Command,
    sync::Arc,
};
use tempfile::TempDir;

#[test]
fn working_preview_is_read_only_and_distinguishes_index_from_worktree() {
    let f = Fixture::new();
    let repo = git::discover(f.a.to_str().unwrap()).unwrap();
    fs::write(f.a.join("shared.txt"), "staged-only\n").unwrap();
    g(&f.a, &["add", "shared.txt"]);
    fs::write(f.a.join("shared.txt"), "worktree-only\n").unwrap();
    let index = fs::read(f.a.join(".git/index")).unwrap();
    let head = g(&f.a, &["rev-parse", "HEAD"]);
    let staged = git::working_diff(&repo, "shared.txt", "staged").unwrap();
    assert!(staged.contains("+staged-only"));
    assert!(!staged.contains("worktree-only"));
    let unstaged = git::working_diff(&repo, "shared.txt", "unstaged").unwrap();
    assert!(unstaged.contains("-staged-only"));
    assert!(unstaged.contains("+worktree-only"));
    let special = "中文 空格 & '$ [x].txt";
    fs::write(f.a.join(special), "new content\n").unwrap();
    assert_eq!(
        git::working_diff(&repo, special, "untracked").unwrap(),
        "new content"
    );
    fs::write(f.a.join("binary"), [0, 1, 2]).unwrap();
    assert!(git::working_diff(&repo, "binary", "untracked")
        .unwrap()
        .contains("二进制"));
    fs::write(f.a.join("empty"), "").unwrap();
    assert!(git::working_diff(&repo, "empty", "untracked")
        .unwrap()
        .contains("空文件"));
    fs::write(f.a.join("large"), "x\n".repeat(2100)).unwrap();
    assert!(git::working_diff(&repo, "large", "untracked")
        .unwrap()
        .contains("已截断"));
    assert!(git::working_diff(&repo, "../outside", "untracked").is_err());
    assert_eq!(fs::read(f.a.join(".git/index")).unwrap(), index);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), head);
    assert_eq!(
        fs::read_to_string(f.a.join("shared.txt")).unwrap(),
        "worktree-only\n"
    );
    assert!(!Path::new(&repo.common_dir).join("sourceneo.lock").exists());
    g(&f.a, &["update-index", "--chmod=+x", "shared.txt"]);
    assert!(git::working_diff(&repo, "shared.txt", "staged")
        .unwrap()
        .contains("new mode 100755"));
}

#[test]
fn working_preview_initial_rename_delete_binary_conflict_and_external_diff() {
    let f = Fixture::new();
    let empty = f.root.join("initial");
    g(&f.root, &["init", empty.to_str().unwrap()]);
    fs::write(empty.join("first"), "first\n").unwrap();
    g(&empty, &["add", "first"]);
    let repo = git::discover(empty.to_str().unwrap()).unwrap();
    assert!(git::working_diff(&repo, "first", "staged")
        .unwrap()
        .contains("+first"));
    let repo = git::discover(f.a.to_str().unwrap()).unwrap();
    g(&f.a, &["mv", "shared.txt", "renamed.txt"]);
    assert!(git::working_diff(&repo, "renamed.txt", "staged")
        .unwrap()
        .contains("rename from shared.txt"));
    fs::remove_file(f.a.join("renamed.txt")).unwrap();
    assert!(git::working_diff(&repo, "renamed.txt", "unstaged")
        .unwrap()
        .contains("deleted file"));
    fs::write(f.a.join("binary"), [0, 4, 2]).unwrap();
    g(&f.a, &["add", "binary"]);
    assert!(git::working_diff(&repo, "binary", "staged")
        .unwrap()
        .contains("Binary files"));
    g(
        &f.a,
        &["config", "diff.external", "sourceneo-must-never-run"],
    );
    assert!(git::working_diff(&repo, "renamed.txt", "staged").is_ok());
    // Build a real unmerged index without changing the current working tree.
    let oid = g(&f.a, &["rev-parse", "HEAD:shared.txt"]);
    let mut child = Command::new("git")
        .args(["update-index", "--index-info"])
        .current_dir(&f.a)
        .stdin(std::process::Stdio::piped())
        .spawn()
        .unwrap();
    use std::io::Write;
    child.stdin.take().unwrap().write_all(format!("100644 {oid} 1\tconflict.txt\n100644 {oid} 2\tconflict.txt\n100644 {oid} 3\tconflict.txt\n").as_bytes()).unwrap();
    assert!(child.wait().unwrap().success());
    assert!(git::working_diff(&repo, "conflict.txt", "staged")
        .unwrap()
        .contains("冲突尚未解决"));
}

#[test]
fn working_preview_staged_delete_and_recreated_untracked_path_are_independent() {
    let f = Fixture::new();
    let repo = git::discover(f.a.to_str().unwrap()).unwrap();
    g(&f.a, &["rm", "shared.txt"]);
    fs::write(f.a.join("shared.txt"), "recreated untracked content\n").unwrap();
    let before = fs::read(f.a.join(".git/index")).unwrap();
    assert!(git::working_diff(&repo, "shared.txt", "staged")
        .unwrap()
        .contains("deleted file"));
    assert!(git::working_diff(&repo, "shared.txt", "untracked")
        .unwrap()
        .contains("recreated untracked content"));
    assert_eq!(fs::read(f.a.join(".git/index")).unwrap(), before);
}

#[test]
fn fetch_and_sync_refresh_safe_custom_tracking_maps_without_touching_local_branches() {
    let f = Fixture::new();
    g(
        &f.a,
        &[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/remotes/company/*",
        ],
    );
    g(&f.a, &["fetch", "origin"]);
    g(
        &f.a,
        &[
            "config",
            "--add",
            "remote.origin.fetch",
            "+refs/heads/*:refs/heads/*",
        ],
    );
    g(
        &f.a,
        &[
            "config",
            "--add",
            "remote.origin.fetch",
            "^refs/heads/excluded",
        ],
    );
    g(&f.b, &["branch", "excluded"]);
    g(&f.b, &["push", "origin", "excluded"]);
    let new_head = commit(&f.b, "remote-new", "remote change\n");
    g(&f.b, &["push"]);
    let before = g(&f.a, &["rev-parse", "HEAD"]);
    let fetched = f.service.fetch(f.a.to_str().unwrap(), None).unwrap();
    assert_eq!(fetched.behind, 1);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), before);
    assert_eq!(
        g(&f.a, &["rev-parse", "refs/remotes/company/main"]),
        new_head
    );
    assert_eq!(f.sync(&f.a).head, new_head);
    let branches = g(
        &f.a,
        &["for-each-ref", "--format=%(refname)", "refs/remotes/"],
    );
    assert!(!branches.lines().any(|name| name.ends_with("/excluded")));
}

#[test]
fn fetch_only_ignores_dangerous_refspecs_and_preserves_dirty_local_state() {
    let f = Fixture::new();
    let remote_head = commit(&f.b, "remote-new", "remote\n");
    g(&f.b, &["push"]);
    fs::write(f.a.join("shared.txt"), "dirty\n").unwrap();
    g(&f.a, &["add", "shared.txt"]);
    fs::write(f.a.join("shared.txt"), "dirty worktree\n").unwrap();
    let head = g(&f.a, &["rev-parse", "HEAD"]);
    let index = fs::read(f.a.join(".git/index")).unwrap();
    g(
        &f.a,
        &[
            "config",
            "remote.origin.fetch",
            "+refs/heads/*:refs/heads/*",
        ],
    );
    g(&f.a, &["config", "fetch.prune", "true"]);
    g(&f.a, &["update-ref", "refs/remotes/origin/keep", &head]);
    let s = f.service.fetch(f.a.to_str().unwrap(), None).unwrap();
    assert!(s.last_fetched.is_some());
    assert_eq!(
        g(&f.a, &["rev-parse", "refs/remotes/origin/main"]),
        remote_head
    );
    assert_eq!(g(&f.a, &["rev-parse", "refs/remotes/origin/keep"]), head);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(f.a.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(f.a.join("shared.txt")).unwrap(),
        "dirty worktree\n"
    );
    let old_time = s.last_fetched;
    g(
        &f.a,
        &[
            "remote",
            "set-url",
            "origin",
            f.root.join("missing.git").to_str().unwrap(),
        ],
    );
    assert!(f.service.fetch(f.a.to_str().unwrap(), None).is_err());
    assert_eq!(f.status(&f.a).last_fetched, old_time);
    assert!(f.service.fetch(f.b.to_str().unwrap(), None).is_ok());
}

#[test]
fn fetch_remote_selection_empty_detached_and_worktree_time() {
    let f = Fixture::new();
    g(&f.a, &["config", "--unset", "branch.main.remote"]);
    g(
        &f.a,
        &["remote", "add", "second", f.remote.to_str().unwrap()],
    );
    assert_eq!(
        f.service
            .fetch(f.a.to_str().unwrap(), None)
            .unwrap_err()
            .kind,
        "remote"
    );
    assert!(f
        .service
        .fetch(f.a.to_str().unwrap(), Some("second"))
        .is_ok());
    g(&f.a, &["checkout", "--detach"]);
    assert!(f.service.fetch(f.a.to_str().unwrap(), None).is_err());
    g(&f.a, &["remote", "remove", "second"]);
    assert!(f.service.fetch(f.a.to_str().unwrap(), None).is_ok());
    let wt = f.root.join("linked");
    g(
        &f.a,
        &["worktree", "add", "-b", "linked", wt.to_str().unwrap()],
    );
    f.service.add(wt.to_str().unwrap()).unwrap();
    assert_eq!(f.status(&wt).last_fetched, f.status(&f.a).last_fetched);
    let empty = f.root.join("empty-fetch");
    g(&f.root, &["init", empty.to_str().unwrap()]);
    f.service.add(empty.to_str().unwrap()).unwrap();
    assert!(f.service.fetch(empty.to_str().unwrap(), None).is_err());
    g(
        &empty,
        &["remote", "add", "origin", f.remote.to_str().unwrap()],
    );
    assert!(f.service.fetch(empty.to_str().unwrap(), None).is_ok());
    assert_eq!(f.status(&empty).head, "(initial)");
}
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

fn sync_identity(path: &Path) {
    g(path, &["config", "user.name", "SourceNeo Test"]);
    g(path, &["config", "user.email", "test@example.invalid"]);
    g(path, &["config", "commit.gpgsign", "false"]);
}

#[cfg(windows)]
#[test]
fn review_ignored_case_variant_file_and_parent_are_preserved() {
    for parent in [false, true] {
        let f = Fixture::new();
        sync_identity(&f.a);
        g(&f.a, &["config", "pull.rebase", "true"]);
        g(&f.a, &["config", "core.ignoreCase", "true"]);
        let local = commit(&f.a, "local.txt", "local work\n");
        fs::write(f.a.join(".git/info/exclude"), "config.json\nartifact\n").unwrap();
        let (saved, remote) = if parent {
            fs::create_dir(f.b.join("Artifact")).unwrap();
            (
                "artifact",
                commit(&f.b, "Artifact/remote.txt", "remote work\n"),
            )
        } else {
            (
                "config.json",
                commit(&f.b, "Config.json", "remote config\n"),
            )
        };
        fs::write(f.a.join(saved), "precious local config\n").unwrap();
        g(&f.b, &["push"]);
        let result = f.service.sync(f.a.to_str().unwrap());
        assert!(
            result.is_err(),
            "case variant must not overwrite ignored content: {result:?}"
        );
        assert_eq!(
            fs::read_to_string(f.a.join(saved)).unwrap(),
            "precious local config\n"
        );
        assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), local);
        assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
    }
}

#[test]
fn review_rebase_merges_config_is_honored_without_pull_merges_flag() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    g(&f.a, &["config", "rebase.rebaseMerges", "true"]);
    g(&f.a, &["switch", "-c", "side"]);
    commit(&f.a, "side.txt", "side work\n");
    g(&f.a, &["switch", "main"]);
    commit(&f.a, "local.txt", "local work\n");
    g(&f.a, &["merge", "--no-ff", "--no-edit", "side"]);
    let remote = commit(&f.b, "remote.txt", "remote work\n");
    g(&f.b, &["push"]);
    assert_eq!(f.sync(&f.a).kind, StatusKind::Synced);
    assert_eq!(
        g(
            &f.a,
            &[
                "rev-list",
                "--merges",
                "--count",
                &format!("{remote}..HEAD")
            ]
        ),
        "1"
    );
}

#[test]
fn review_rebase_excludes_local_commits_already_contained_in_fetched_upstream() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    commit(&f.a, "shared.txt", "intermediate shared change\n");
    // A colleague adopts this commit without updating alpha's upstream reflog.
    g(&f.b, &["fetch", f.a.to_str().unwrap(), "main"]);
    g(&f.b, &["merge", "--ff-only", "FETCH_HEAD"]);
    let remote = commit(&f.b, "shared.txt", "newer shared change\n");
    g(&f.b, &["push"]);
    commit(&f.a, "local.txt", "only local work to replay\n");
    let result = f.sync(&f.a);
    assert_eq!(
        result.kind,
        StatusKind::Synced,
        "already upstream commit must not be replayed into a conflict"
    );
    assert_eq!(g(&f.a, &["rev-parse", "HEAD^"]), remote);
    assert_eq!(
        fs::read_to_string(f.a.join("shared.txt")).unwrap(),
        "newer shared change\n"
    );
}

#[test]
fn review_rebase_remote_rewind_does_not_restore_withdrawn_commits() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    let base = g(&f.a, &["rev-parse", "HEAD"]);
    commit(&f.a, "withdrawn.txt", "withdraw this upstream commit\n");
    g(&f.a, &["push"]);
    commit(&f.a, "local.txt", "keep unpublished work\n");
    // beta still points at base; only this temporary bare remote is rewound.
    g(&f.b, &["push", "--force", "origin", "main"]);
    let result = f.sync(&f.a);
    assert_eq!(result.kind, StatusKind::Synced);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD^"]), base);
    assert!(!f.a.join("withdrawn.txt").exists());
    assert_eq!(
        fs::read_to_string(f.a.join("local.txt")).unwrap(),
        "keep unpublished work\n"
    );
}

#[test]
fn review_rebase_remote_rewind_still_protects_ignored_files() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    let base = g(&f.a, &["rev-parse", "HEAD"]);
    g(&f.a, &["rm", "shared.txt"]);
    g(&f.a, &["commit", "-m", "delete shared file upstream"]);
    g(&f.a, &["push"]);
    let local = commit(&f.a, "local.txt", "keep local work\n");
    fs::write(f.a.join(".git/info/exclude"), "shared.txt\n").unwrap();
    fs::write(f.a.join("shared.txt"), "precious local ignored contents\n").unwrap();
    g(&f.b, &["push", "--force", "origin", "main"]);
    let result = f.service.sync(f.a.to_str().unwrap()).unwrap_err();
    assert_eq!(result.kind, "changed");
    assert!(result.message.contains("被忽略文件"));
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), local);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), base);
    assert_eq!(
        fs::read_to_string(f.a.join("shared.txt")).unwrap(),
        "precious local ignored contents\n"
    );
}

#[test]
fn review_fast_forward_hook_head_change_stops_before_push() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    let remote = commit(&f.b, "remote.txt", "remote work\n");
    g(&f.b, &["push"]);
    let hook = f.a.join(".git/hooks/post-merge");
    fs::write(
        &hook,
        "#!/bin/sh\ngit -c commit.gpgsign=false commit --allow-empty -m 'unexpected hook commit'\n",
    )
    .unwrap();
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();
    }
    let result = f.service.sync(f.a.to_str().unwrap());
    assert!(
        result.is_err(),
        "unexpected hook commit must not be pushed: {result:?}"
    );
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
    assert_ne!(g(&f.a, &["rev-parse", "HEAD"]), remote);
}

#[test]
fn configured_rebase_integrates_divergence_then_pushes_only_current_branch() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    g(&f.a, &["config", "rebase.updateRefs", "true"]);
    let original = commit(&f.a, "local.txt", "local\n");
    g(&f.a, &["branch", "keep-local", &original]);
    let remote = commit(&f.b, "remote.txt", "remote\n");
    g(&f.b, &["push"]);
    let config = fs::read(f.a.join(".git/config")).unwrap();
    let result = f.sync(&f.a);
    assert_eq!(result.kind, StatusKind::Synced);
    assert_ne!(result.head, original);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD^"]), remote);
    assert_eq!(g(&f.a, &["rev-parse", "keep-local"]), original);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), result.head);
    assert_eq!(
        g(
            &f.remote,
            &["for-each-ref", "--format=%(refname)", "refs/heads/"]
        ),
        "refs/heads/main"
    );
    assert_eq!(fs::read(f.a.join(".git/config")).unwrap(), config);
    assert!(f.a.join("remote.txt").exists() && f.a.join("local.txt").exists());
}

#[test]
fn branch_rebase_overrides_pull_and_included_configuration() {
    for (pull, branch, expect_merge) in [("true", "false", true), ("false", "true", false)] {
        let f = Fixture::new();
        sync_identity(&f.a);
        let included = f.root.join("sync-config");
        fs::write(&included, format!("[pull]\n\trebase = {pull}\n")).unwrap();
        g(
            &f.a,
            &["config", "include.path", included.to_str().unwrap()],
        );
        g(&f.a, &["config", "branch.main.rebase", branch]);
        let local = commit(&f.a, "local.txt", "local\n");
        let remote = commit(&f.b, "remote.txt", "remote\n");
        g(&f.b, &["push"]);
        let result = f.sync(&f.a);
        assert_eq!(result.kind, StatusKind::Synced);
        if expect_merge {
            assert_eq!(g(&f.a, &["rev-parse", "HEAD^1"]), local);
            assert_eq!(g(&f.a, &["rev-parse", "HEAD^2"]), remote);
        } else {
            assert_eq!(g(&f.a, &["rev-parse", "HEAD^"]), remote);
        }
    }
}

#[test]
fn rebase_and_merge_conflicts_pause_preserve_state_and_never_push() {
    for (config, operation) in [("true", "rebase"), ("false", "merge")] {
        let f = Fixture::new();
        sync_identity(&f.a);
        g(&f.a, &["config", "pull.rebase", config]);
        let local = commit(&f.a, "shared.txt", "local conflict\n");
        let remote = commit(&f.b, "shared.txt", "remote conflict\n");
        g(&f.b, &["push"]);
        let result = f.sync(&f.a);
        assert_eq!(result.kind, StatusKind::Conflict);
        assert_eq!(result.conflicts, vec!["shared.txt"]);
        assert_eq!(result.operation.as_deref(), Some(operation));
        assert!(result.message.contains(&format!("{operation} 发生冲突")));
        assert!(result.message.contains("未推送"));
        assert_eq!(g(&f.a, &["rev-parse", "main"]), local);
        assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
        let contents = fs::read(f.a.join("shared.txt")).unwrap();
        let index = fs::read(f.a.join(".git/index")).unwrap();
        assert_eq!(f.sync(&f.a).kind, StatusKind::Conflict);
        assert_eq!(fs::read(f.a.join("shared.txt")).unwrap(), contents);
        assert_eq!(fs::read(f.a.join(".git/index")).unwrap(), index);
        let refreshed = f.status(&f.a);
        assert_eq!(refreshed.kind, StatusKind::Conflict);
        assert!(refreshed.message.contains(operation));
        assert!(f
            .service
            .diagnostic(f.a.to_str().unwrap())
            .unwrap()
            .contains(operation));
        // Simulate resolving and finishing in an external Git tool. A later sync
        // can push normally; the paused call above never changed the remote.
        fs::write(f.a.join("shared.txt"), "resolved contents\n").unwrap();
        g(&f.a, &["add", "shared.txt"]);
        let staged = fs::read(f.a.join(".git/index")).unwrap();
        let paused = f.sync(&f.a);
        assert_eq!(paused.kind, StatusKind::Attention);
        assert_eq!(paused.operation.as_deref(), Some(operation));
        assert_eq!(fs::read(f.a.join(".git/index")).unwrap(), staged);
        assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
        if operation == "rebase" {
            g(&f.a, &["-c", "core.editor=true", "rebase", "--continue"]);
        } else {
            g(&f.a, &["commit", "-m", "resolve merge"]);
        }
        let completed = f.sync(&f.a);
        assert_eq!(completed.kind, StatusKind::Synced);
        assert_eq!(g(&f.remote, &["rev-parse", "main"]), completed.head);
    }
}

#[test]
fn pull_ff_only_overrides_rebase_and_no_ff_overrides_merge_ff() {
    let f = Fixture::new();
    g(&f.a, &["config", "pull.rebase", "true"]);
    g(&f.a, &["config", "pull.ff", "only"]);
    let local = commit(&f.a, "local.txt", "local\n");
    let remote = commit(&f.b, "remote.txt", "remote\n");
    g(&f.b, &["push"]);
    let result = f.sync(&f.a);
    assert_eq!(result.kind, StatusKind::Diverged);
    assert!(result.message.contains("pull.ff=only"));
    assert_eq!(result.head, local);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);

    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "false"]);
    g(&f.a, &["config", "pull.ff", "false"]);
    g(&f.a, &["config", "merge.ff", "only"]);
    let old = g(&f.a, &["rev-parse", "HEAD"]);
    let remote = commit(&f.b, "remote.txt", "remote\n");
    g(&f.b, &["push"]);
    let result = f.sync(&f.a);
    assert_eq!(result.kind, StatusKind::Synced);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD^1"]), old);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD^2"]), remote);
}

#[test]
fn configured_rebase_without_local_commits_fast_forwards() {
    let f = Fixture::new();
    g(&f.a, &["config", "pull.rebase", "true"]);
    g(&f.a, &["config", "pull.ff", "false"]);
    let remote = commit(&f.b, "remote.txt", "remote\n");
    g(&f.b, &["push"]);
    assert_eq!(f.sync(&f.a).head, remote);
}

#[test]
fn interactive_and_invalid_policy_never_rewrite_or_push() {
    for (key, value) in [
        ("pull.rebase", "interactive"),
        ("pull.rebase", "invalid"),
        ("pull.ff", "invalid"),
    ] {
        let f = Fixture::new();
        g(&f.a, &["config", key, value]);
        let local = commit(&f.a, "local.txt", "local\n");
        let remote = commit(&f.b, "remote.txt", "remote\n");
        g(&f.b, &["push"]);
        let result = f.service.sync(f.a.to_str().unwrap());
        if value == "interactive" {
            let result = result.unwrap();
            assert_eq!(result.kind, StatusKind::Attention);
            assert!(result.message.contains("interactive rebase"));
        } else {
            assert!(result.is_err());
        }
        assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), local);
        assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
        assert_eq!(g(&f.a, &["status", "--porcelain"]), "");
    }
}

#[test]
fn rebase_protects_dirty_and_ignored_files_even_with_autostash_configured() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    g(&f.a, &["config", "rebase.autoStash", "true"]);
    let local = commit(&f.a, "local.txt", "local\n");
    let remote = commit(&f.b, "precious.txt", "remote tracked content\n");
    g(&f.b, &["push"]);
    fs::write(f.a.join("shared.txt"), "dirty content\n").unwrap();
    assert_eq!(f.sync(&f.a).kind, StatusKind::Attention);
    assert_eq!(
        fs::read_to_string(f.a.join("shared.txt")).unwrap(),
        "dirty content\n"
    );
    fs::write(f.a.join("shared.txt"), "base\n").unwrap();
    fs::write(f.a.join(".git/info/exclude"), "precious.txt\n").unwrap();
    fs::write(f.a.join("precious.txt"), "precious ignored content\n").unwrap();
    let error = f.service.sync(f.a.to_str().unwrap()).unwrap_err();
    assert!(error.message.contains("被忽略文件"));
    assert_eq!(
        fs::read_to_string(f.a.join("precious.txt")).unwrap(),
        "precious ignored content\n"
    );
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), local);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
    fs::remove_file(f.a.join("precious.txt")).unwrap();
    fs::write(f.a.join(".git/info/exclude"), "build-cache/\n").unwrap();
    fs::create_dir(f.a.join("build-cache")).unwrap();
    fs::write(f.a.join("build-cache/keep.bin"), "unrelated ignored cache").unwrap();
    assert_eq!(f.sync(&f.a).kind, StatusKind::Synced);
    assert_eq!(
        fs::read_to_string(f.a.join("build-cache/keep.bin")).unwrap(),
        "unrelated ignored cache"
    );
}

#[test]
fn rebase_ignored_large_unrelated_tree_does_not_limit_sync_or_hide_real_blockers() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    g(&f.a, &["config", "core.longpaths", "true"]);
    let cache = format!("ignored-cache-{}", "c".repeat(90));
    fs::create_dir(f.a.join(&cache)).unwrap();
    fs::write(f.a.join(".git/info/exclude"), format!("{cache}/\n")).unwrap();
    let mut listed_bytes = 0;
    for index in 0..45_000 {
        let name = format!("{index:05}-{}.bin", "x".repeat(100));
        listed_bytes += cache.len() + name.len() + 2;
        fs::File::create(f.a.join(&cache).join(name)).unwrap();
    }
    assert!(listed_bytes > 8 * 1024 * 1024);
    commit(&f.a, "local.txt", "local work\n");
    commit(&f.b, "remote.txt", "remote work\n");
    g(&f.b, &["push"]);
    assert_eq!(f.sync(&f.a).kind, StatusKind::Synced);
    // The same large directory must block when the remote replaces it with a
    // file, without listing its entire contents or returning outputLimit.
    g(&f.b, &["pull", "--ff-only"]);
    commit(&f.a, "more-local.txt", "more local work\n");
    let local = g(&f.a, &["rev-parse", "HEAD"]);
    let remote = commit(&f.b, &cache, "remote file replaces directory\n");
    g(&f.b, &["push"]);
    let error = f.service.sync(f.a.to_str().unwrap()).unwrap_err();
    assert_eq!(error.kind, "changed");
    assert!(error.message.contains("被忽略文件"));
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), local);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
    assert_eq!(fs::read_dir(f.a.join(cache)).unwrap().count(), 45_000);
}

#[test]
fn rebase_ignored_intermediate_replay_path_is_protected_with_literal_names() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    let file = "中 文 [x] & '$.bin";
    commit(&f.a, file, "temporary tracked file\n");
    g(&f.a, &["rm", "--", file]);
    g(&f.a, &["commit", "-m", "delete intermediate file"]);
    // Use a global exclusion file configured only in this temporary repository.
    let exclusions = f.root.join("ignored-patterns");
    fs::write(&exclusions, "*.bin\n").unwrap();
    g(
        &f.a,
        &["config", "core.excludesFile", exclusions.to_str().unwrap()],
    );
    fs::write(f.a.join(file), "precious recreated file\n").unwrap();
    let local = g(&f.a, &["rev-parse", "HEAD"]);
    let remote = commit(&f.b, "remote.txt", "remote work\n");
    g(&f.b, &["push"]);
    let error = f.service.sync(f.a.to_str().unwrap()).unwrap_err();
    assert_eq!(error.kind, "changed");
    assert!(error.message.contains(file));
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), local);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
    assert_eq!(
        fs::read_to_string(f.a.join(file)).unwrap(),
        "precious recreated file\n"
    );
}

#[test]
fn rebase_ignored_file_and_directory_collisions_are_preserved() {
    for directory in [true, false] {
        let f = Fixture::new();
        sync_identity(&f.a);
        g(&f.a, &["config", "pull.rebase", "true"]);
        let local = commit(&f.a, "local.txt", "local work\n");
        fs::write(f.a.join(".git/info/exclude"), "artifact\n").unwrap();
        let saved;
        let remote;
        if directory {
            fs::create_dir(f.a.join("artifact")).unwrap();
            saved = f.a.join("artifact/precious.bin");
            remote = commit(&f.b, "artifact", "remote file\n");
        } else {
            saved = f.a.join("artifact");
            fs::create_dir(f.b.join("artifact")).unwrap();
            remote = commit(&f.b, "artifact/remote.txt", "remote directory\n");
        }
        fs::write(&saved, "precious ignored content\n").unwrap();
        g(&f.b, &["push"]);
        let error = f.service.sync(f.a.to_str().unwrap()).unwrap_err();
        assert_eq!(error.kind, "changed");
        assert_eq!(
            fs::read_to_string(saved).unwrap(),
            "precious ignored content\n"
        );
        assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), local);
        assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote);
    }
}

#[test]
fn rebase_ignored_check_allows_clean_tracked_directory_to_file_replacement() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    fs::create_dir(f.b.join("replace")).unwrap();
    commit(&f.b, "replace/tracked.txt", "tracked contents\n");
    g(&f.b, &["push"]);
    g(&f.a, &["pull", "--ff-only"]);
    commit(&f.a, "local.txt", "local work\n");
    g(&f.b, &["rm", "-r", "replace"]);
    commit(&f.b, "replace", "replacement file\n");
    g(&f.b, &["push"]);
    assert_eq!(f.sync(&f.a).kind, StatusKind::Synced);
    assert_eq!(
        fs::read_to_string(f.a.join("replace")).unwrap(),
        "replacement file\n"
    );
}

#[test]
fn rebase_merges_preserves_local_merge_topology() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "merges"]);
    g(&f.a, &["switch", "-c", "side"]);
    commit(&f.a, "side.txt", "side\n");
    g(&f.a, &["switch", "main"]);
    commit(&f.a, "local.txt", "local\n");
    g(&f.a, &["merge", "--no-ff", "--no-edit", "side"]);
    let remote = commit(&f.b, "remote.txt", "remote\n");
    g(&f.b, &["push"]);
    let result = f.sync(&f.a);
    assert_eq!(result.kind, StatusKind::Synced);
    assert_eq!(
        g(
            &f.a,
            &[
                "rev-list",
                "--merges",
                "--count",
                &format!("{remote}..HEAD")
            ]
        ),
        "1"
    );
    for file in ["side.txt", "local.txt", "remote.txt"] {
        assert!(f.a.join(file).exists());
    }
}

#[test]
fn rebase_does_not_replay_withdrawn_upstream_commits_after_remote_rewrite() {
    let f = Fixture::new();
    sync_identity(&f.a);
    g(&f.a, &["config", "pull.rebase", "true"]);
    let base = g(&f.b, &["rev-parse", "HEAD"]);
    commit(&f.b, "withdrawn.txt", "withdrawn upstream commit\n");
    g(&f.b, &["push"]);
    g(&f.a, &["pull", "--ff-only"]);
    commit(&f.a, "local.txt", "local work\n");
    // Only this test's temporary bare remote is rewritten.
    g(&f.b, &["reset", "--hard", &base]);
    let new_upstream = commit(&f.b, "replacement.txt", "replacement upstream\n");
    g(&f.b, &["push", "--force"]);
    let result = f.sync(&f.a);
    assert_eq!(result.kind, StatusKind::Synced);
    assert_eq!(g(&f.a, &["rev-parse", "HEAD^"]), new_upstream);
    assert!(!f.a.join("withdrawn.txt").exists());
    assert!(f.a.join("local.txt").exists());
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
    assert!(f
        .service
        .working_changes(f.a.to_str().unwrap())
        .unwrap()
        .iter()
        .any(|file| file.path == "shared.txt" && file.status == "UU"));
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
    assert!(f
        .service
        .history(&r.path, 100, false)
        .unwrap()
        .commits
        .is_empty());
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
fn history_scope_excludes_unmerged_local_remote_and_tag_only_commits() {
    let f = Fixture::new();
    let path = f.a.to_str().unwrap();
    let base = g(&f.a, &["rev-parse", "HEAD"]);
    g(&f.a, &["checkout", "-b", "side"]);
    let side = commit(&f.a, "side.txt", "unmerged local branch");
    g(&f.a, &["checkout", "-b", "tag-only"]);
    let tagged = commit(&f.a, "tag.txt", "tag-only history");
    g(&f.a, &["tag", "archived", &tagged]);
    g(&f.a, &["checkout", "main"]);
    g(&f.a, &["branch", "-D", "tag-only"]);
    let main = commit(&f.a, "main.txt", "current branch");
    let remote = commit(&f.b, "remote.txt", "remote-only history");
    g(&f.b, &["push"]);
    g(&f.a, &["fetch"]);

    let current = f.service.history(path, 2, false).unwrap();
    assert_eq!(
        current
            .commits
            .iter()
            .map(|c| c.hash.as_str())
            .collect::<Vec<_>>(),
        vec![main.as_str(), base.as_str()]
    );
    assert!(!current.has_more);
    let all = f.service.history(path, 100, true).unwrap();
    for hash in [&main, &base, &side, &tagged, &remote] {
        assert!(all.commits.iter().any(|c| &c.hash == hash));
    }
    assert!(f.service.history(path, 2, true).unwrap().has_more);
}

#[test]
fn history_scope_handles_detached_and_unborn_head() {
    let f = Fixture::new();
    let path = f.a.to_str().unwrap();
    let base = g(&f.a, &["rev-parse", "HEAD"]);
    let main = commit(&f.a, "main.txt", "main tip");
    g(&f.a, &["checkout", "--detach", &base]);
    let detached = commit(&f.a, "detached.txt", "detached tip");
    let current = f.service.history(path, 100, false).unwrap();
    assert_eq!(current.commits.len(), 2);
    assert_eq!(current.commits[0].hash, detached);
    assert!(current.commits[0].refs.contains(&"HEAD".into()));
    let all = f.service.history(path, 100, true).unwrap();
    for hash in [&main, &detached] {
        assert!(all.commits.iter().any(|c| &c.hash == hash));
    }

    g(&f.a, &["symbolic-ref", "HEAD", "refs/heads/unborn"]);
    let current = f.service.history(path, 100, false).unwrap();
    assert!(current.commits.is_empty());
    assert!(!current.has_more);
    assert!(f
        .service
        .history(path, 100, true)
        .unwrap()
        .commits
        .iter()
        .any(|c| c.hash == main));
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
    let h = f
        .service
        .history(f.a.to_str().unwrap(), 100, false)
        .unwrap();
    assert_eq!(h.commits[0].parents, vec![main, feature]);
    assert!(h.commits[0].refs.contains(&"refs/heads/main".into()));
    assert!(h.commits[0].refs.contains(&"HEAD".into()));
    assert!(h
        .commits
        .iter()
        .any(|c| c.refs.contains(&"refs/remotes/origin/main".into())));
    let page = f.service.history(f.a.to_str().unwrap(), 2, false).unwrap();
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
    assert!(f.service.history(&child.path, 100, false).is_err());
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
    let h = f
        .service
        .history(f.a.to_str().unwrap(), 100, false)
        .unwrap();
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
    assert!(f.service.history(&r.path, 100, false).unwrap().shallow);
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
    let history = f.service.history(&repo.path, 100, false).unwrap();
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
        let history = f.service.history(path, 100, false);
        let diagnostic = f.service.diagnostic(path);
        let branches = f.service.branches(path);
        let working = f.service.working_changes(path);
        let changes = f.service.commit_changes(path, &head);
        let diff = f.service.file_diff(path, &head, "shared.txt");
        let sync = f.service.sync(path);
        fs::set_permissions(&dir, original).unwrap();
        if existing_lock {
            fs::set_permissions(&lock, fs::Permissions::from_mode(0o644)).unwrap();
        }
        assert_eq!(status.unwrap().kind, StatusKind::Synced);
        assert_eq!(history.unwrap().commits[0].hash, head);
        assert!(diagnostic.unwrap().contains(&head));
        assert!(branches.unwrap().iter().any(|b| b.current));
        assert!(working.unwrap().is_empty());
        assert_eq!(changes.unwrap().files[0].path, "shared.txt");
        assert!(diff.unwrap().contains("+base"));
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
        let history = f.service.history(f.a.to_str().unwrap(), 1, false).unwrap();
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
        f.service.history(&added.path, 100, false).unwrap().commits[0].hash,
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
        f.service.history(&added.path, 100, false).unwrap().commits[0].hash,
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

#[test]
fn branch_switching_and_creation_are_local_and_track_remote_explicitly() {
    let f = Fixture::new();
    let path = f.a.to_str().unwrap();
    let remote_head = g(&f.remote, &["rev-parse", "main"]);
    g(&f.a, &["branch", "feature/local"]);
    let branches = f.service.branches(path).unwrap();
    assert!(branches
        .iter()
        .any(|b| b.name == "main" && b.current && !b.remote));
    assert!(branches
        .iter()
        .any(|b| b.reference == "refs/remotes/origin/main" && b.remote));
    assert!(!branches.iter().any(|b| b.name == "origin/HEAD"));
    assert_eq!(
        f.service
            .switch_branch(path, "refs/heads/feature/local", false)
            .unwrap()
            .branch,
        "feature/local"
    );
    assert_eq!(
        f.service
            .switch_branch(path, "feature/new", true)
            .unwrap()
            .branch,
        "feature/new"
    );
    for bad in ["--discard-changes", "bad name", "@{-1}", "HEAD"] {
        assert!(f.service.switch_branch(path, bad, true).is_err());
    }
    assert_eq!(f.status(&f.a).branch, "feature/new");
    g(&f.b, &["switch", "-c", "feature/remote"]);
    let oid = commit(&f.b, "remote-branch.txt", "remote branch tip");
    g(&f.b, &["push", "origin", "feature/remote"]);
    g(&f.a, &["fetch", "origin"]);
    let switched = f
        .service
        .switch_branch(path, "refs/remotes/origin/feature/remote", false)
        .unwrap();
    assert_eq!(switched.branch, "feature/remote");
    assert_eq!(switched.head, oid);
    assert_eq!(switched.upstream.as_deref(), Some("origin/feature/remote"));
    assert!(f
        .service
        .switch_branch(path, "refs/remotes/origin/feature/remote", false)
        .is_err());
    assert_eq!(f.status(&f.a).head, oid);
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote_head);
}

#[test]
fn branch_switch_refuses_dirty_operations_locks_and_occupied_worktrees() {
    let f = Fixture::new();
    let path = f.a.to_str().unwrap();
    g(&f.a, &["branch", "other"]);
    fs::write(f.a.join("shared.txt"), "precious dirty content").unwrap();
    assert_eq!(
        f.service
            .switch_branch(path, "refs/heads/other", false)
            .unwrap_err()
            .kind,
        "changed"
    );
    assert_eq!(
        fs::read_to_string(f.a.join("shared.txt")).unwrap(),
        "precious dirty content"
    );
    g(&f.a, &["restore", "shared.txt"]);
    fs::create_dir(f.a.join(".git/rebase-merge")).unwrap();
    assert_eq!(
        f.service
            .switch_branch(path, "refs/heads/other", false)
            .unwrap_err()
            .kind,
        "changed"
    );
    fs::remove_dir(f.a.join(".git/rebase-merge")).unwrap();
    fs::write(f.a.join(".git/index.lock"), "keep lock").unwrap();
    assert_eq!(
        f.service
            .switch_branch(path, "refs/heads/other", false)
            .unwrap_err()
            .kind,
        "busy"
    );
    assert_eq!(
        fs::read_to_string(f.a.join(".git/index.lock")).unwrap(),
        "keep lock"
    );
    fs::remove_file(f.a.join(".git/index.lock")).unwrap();
    let linked = f.root.join("linked");
    g(
        &f.a,
        &["worktree", "add", linked.to_str().unwrap(), "other"],
    );
    assert!(f
        .service
        .switch_branch(path, "refs/heads/other", false)
        .is_err());
    assert_eq!(f.status(&f.a).branch, "main");
}

#[test]
fn branch_switch_does_not_overwrite_ignored_files() {
    let f = Fixture::new();
    g(&f.a, &["switch", "-c", "other"]);
    commit(&f.a, "ignored.txt", "tracked on other");
    g(&f.a, &["switch", "main"]);
    fs::write(f.a.join(".git/info/exclude"), "ignored.txt\n").unwrap();
    fs::write(f.a.join("ignored.txt"), "precious ignored content").unwrap();
    assert!(f
        .service
        .switch_branch(f.a.to_str().unwrap(), "refs/heads/other", false)
        .is_err());
    assert_eq!(
        fs::read_to_string(f.a.join("ignored.txt")).unwrap(),
        "precious ignored content"
    );
    assert_eq!(f.status(&f.a).branch, "main");
}

#[cfg(unix)]
#[test]
fn failed_post_checkout_hook_reports_error_after_branch_has_changed() {
    use std::os::unix::fs::PermissionsExt;
    let f = Fixture::new();
    let path = f.a.to_str().unwrap();
    g(&f.a, &["switch", "-c", "feature"]);
    let feature_head = commit(&f.a, "feature.txt", "feature content\n");
    g(&f.a, &["switch", "main"]);
    let remote_head = g(&f.remote, &["rev-parse", "main"]);
    let hook = f.a.join(".git/hooks/post-checkout");
    fs::write(
        &hook,
        "#!/bin/sh\necho 'post-checkout failed' >&2\nexit 1\n",
    )
    .unwrap();
    fs::set_permissions(&hook, fs::Permissions::from_mode(0o755)).unwrap();

    let error = f
        .service
        .switch_branch(path, "refs/heads/feature", false)
        .unwrap_err();
    assert!(error.message.contains("post-checkout failed"));
    let actual = f.service.status(path).unwrap();
    assert_eq!(actual.branch, "feature");
    assert_eq!(actual.head, feature_head);
    assert_eq!(actual.changed, 0);
    assert!(f
        .service
        .branches(path)
        .unwrap()
        .iter()
        .any(|b| b.name == "feature" && b.current));
    assert_eq!(
        fs::read_to_string(f.a.join("feature.txt")).unwrap(),
        "feature content\n"
    );
    assert_eq!(g(&f.remote, &["rev-parse", "main"]), remote_head);
}

#[test]
fn commit_files_and_diffs_handle_root_rename_delete_binary_and_literal_paths() {
    let f = Fixture::new();
    let path = f.a.to_str().unwrap();
    let root = g(&f.a, &["rev-parse", "HEAD"]);
    let changes = f.service.commit_changes(path, &root).unwrap();
    assert!(changes.parent.is_none());
    assert_eq!(changes.files.len(), 1);
    assert_eq!(changes.files[0].status, "A");
    assert!(f
        .service
        .file_diff(path, &root, "shared.txt")
        .unwrap()
        .contains("+base"));
    g(&f.a, &["mv", "shared.txt", "renamed 文件.txt"]);
    fs::write(f.a.join("binary.dat"), [0, 255, 0, 1]).unwrap();
    let literal = if cfg!(windows) {
        "[literal].txt"
    } else {
        ":(glob)*.txt"
    };
    fs::write(f.a.join(literal), "literal change\n").unwrap();
    #[cfg(unix)]
    fs::write(f.a.join("line\nbreak.txt"), "newline filename\n").unwrap();
    g(&f.a, &["add", "--all"]);
    g(
        &f.a,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "rename and binary",
        ],
    );
    let hash = g(&f.a, &["rev-parse", "HEAD"]);
    let changes = f.service.commit_changes(path, &hash).unwrap();
    #[cfg(unix)]
    {
        assert!(changes
            .files
            .iter()
            .any(|file| file.path == "line\nbreak.txt"));
        assert!(f
            .service
            .file_diff(path, &hash, "line\nbreak.txt")
            .unwrap()
            .contains("+newline filename"));
    }
    assert_eq!(changes.parent.as_deref(), Some(root.as_str()));
    let renamed = changes
        .files
        .iter()
        .find(|file| file.path == "renamed 文件.txt")
        .unwrap();
    assert!(renamed.status.starts_with('R'));
    assert_eq!(renamed.old_path.as_deref(), Some("shared.txt"));
    assert!(f
        .service
        .file_diff(path, &hash, &renamed.path)
        .unwrap()
        .contains("rename from shared.txt"));
    assert!(f
        .service
        .file_diff(path, &hash, "binary.dat")
        .unwrap()
        .contains("Binary files"));
    let diff = f.service.file_diff(path, &hash, literal).unwrap();
    assert!(diff.contains("+literal change"));
    assert!(!diff.contains("binary.dat"));
    assert!(f.service.file_diff(path, &hash, "not-in-commit").is_err());
    assert!(f.service.commit_changes(path, "--all").is_err());
    let blob = g(&f.a, &["rev-parse", "HEAD:binary.dat"]);
    assert!(f.service.commit_changes(path, &blob).is_err());
    fs::write(f.a.join(literal), "dirty worktree must remain").unwrap();
    assert!(f
        .service
        .file_diff(path, &hash, literal)
        .unwrap()
        .contains("+literal change"));
    assert_eq!(
        fs::read_to_string(f.a.join(literal)).unwrap(),
        "dirty worktree must remain"
    );
    g(&f.a, &["rm", "binary.dat"]);
    g(
        &f.a,
        &[
            "-c",
            "commit.gpgsign=false",
            "commit",
            "-m",
            "delete binary",
        ],
    );
    let deleted = g(&f.a, &["rev-parse", "HEAD"]);
    assert!(f
        .service
        .commit_changes(path, &deleted)
        .unwrap()
        .files
        .iter()
        .any(|file| file.path == "binary.dat" && file.status == "D"));
    assert!(f
        .service
        .file_diff(path, &deleted, "binary.dat")
        .unwrap()
        .contains("deleted file mode"));
}

#[test]
fn merge_changes_compare_first_parent_and_never_run_external_diff() {
    let f = Fixture::new();
    g(&f.a, &["switch", "-c", "side"]);
    commit(&f.a, "side.txt", "side only");
    g(&f.a, &["switch", "main"]);
    let parent = commit(&f.a, "main.txt", "main only");
    g(
        &f.a,
        &[
            "-c",
            "commit.gpgsign=false",
            "merge",
            "--no-ff",
            "side",
            "-m",
            "merge side",
        ],
    );
    let hash = g(&f.a, &["rev-parse", "HEAD"]);
    g(
        &f.a,
        &[
            "config",
            "diff.external",
            "sourceneo-command-that-must-never-run",
        ],
    );
    g(&f.a, &["config", "diff.relative", "true"]);
    fs::write(f.a.join(".git/info/attributes"), "*.txt diff=review\n").unwrap();
    g(
        &f.a,
        &[
            "config",
            "diff.review.textconv",
            "sourceneo-command-that-must-never-run",
        ],
    );
    g(
        &f.a,
        &[
            "config",
            "diff.review.command",
            "sourceneo-command-that-must-never-run",
        ],
    );
    let path = f.a.to_str().unwrap();
    let changes = f.service.commit_changes(path, &hash).unwrap();
    assert_eq!(changes.parent.as_deref(), Some(parent.as_str()));
    assert_eq!(changes.files.len(), 1);
    assert_eq!(changes.files[0].path, "side.txt");
    assert!(f
        .service
        .file_diff(path, &hash, "side.txt")
        .unwrap()
        .contains("+side only"));
}

#[test]
fn shallow_commit_details_do_not_mislabel_missing_parents_as_a_root_commit() {
    let f = Fixture::new();
    commit(&f.a, "second.txt", "second commit");
    g(&f.a, &["push"]);
    let shallow = f.root.join("shallow-details");
    let normalized = f.remote.to_string_lossy().replace('\\', "/");
    let url = format!("file:///{}", normalized.trim_start_matches('/'));
    g(
        &f.root,
        &["clone", "--depth", "1", &url, shallow.to_str().unwrap()],
    );
    let repo = f.service.add(shallow.to_str().unwrap()).unwrap();
    let head = g(&shallow, &["rev-parse", "HEAD"]);
    assert!(f.service.commit_changes(&repo.path, &head).is_err());
    assert_eq!(g(&shallow, &["rev-parse", "HEAD"]), head);
    assert_eq!(g(&shallow, &["status", "--porcelain"]), "");
}

#[test]
fn working_files_preserve_stage_state_renames_and_untracked_paths_without_writes() {
    let f = Fixture::new();
    commit(&f.a, "old.txt", "rename content");
    commit(&f.a, "keep.txt", "keep the worktree copy");
    g(&f.a, &["rm", "--cached", "keep.txt"]);
    g(&f.a, &["mv", "old.txt", "新 文件.txt"]);
    fs::write(f.a.join("shared.txt"), "staged content\n").unwrap();
    g(&f.a, &["add", "shared.txt"]);
    fs::write(f.a.join("shared.txt"), "unstaged content\n").unwrap();
    fs::create_dir(f.a.join("untracked")).unwrap();
    fs::write(f.a.join("untracked/one.txt"), "one").unwrap();
    fs::write(f.a.join("untracked/two.txt"), "two").unwrap();
    #[cfg(unix)]
    fs::write(f.a.join("line\nbreak.txt"), "newline").unwrap();
    let head = g(&f.a, &["rev-parse", "HEAD"]);
    let index = fs::read(f.a.join(".git/index")).unwrap();
    let files = f.service.working_changes(f.a.to_str().unwrap()).unwrap();
    assert!(files
        .iter()
        .any(|file| file.path == "keep.txt" && file.status == "D "));
    assert!(files
        .iter()
        .any(|file| file.path == "keep.txt" && file.status == "??"));
    assert!(files
        .iter()
        .any(|file| file.path == "shared.txt" && file.status == "MM"));
    assert!(files.iter().any(|file| file.path == "新 文件.txt"
        && file.status == "R "
        && file.old_path.as_deref() == Some("old.txt")));
    for path in ["untracked/one.txt", "untracked/two.txt"] {
        assert!(files
            .iter()
            .any(|file| file.path == path && file.status == "??"));
    }
    #[cfg(unix)]
    assert!(files
        .iter()
        .any(|file| file.path == "line\nbreak.txt" && file.status == "??"));
    assert_eq!(fs::read(f.a.join(".git/index")).unwrap(), index);
    assert_eq!(
        fs::read_to_string(f.a.join("shared.txt")).unwrap(),
        "unstaged content\n"
    );
    assert_eq!(g(&f.a, &["rev-parse", "HEAD"]), head);
}
