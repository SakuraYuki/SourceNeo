use sourceneo_lib::{model::RepoFlagsPatch, service::Service};
use std::{fs, path::Path, process::Command};

fn git(path: &Path, args: &[&str]) -> String {
    let output = Command::new("git")
        .args([
            "-c",
            "user.name=SourceNeo Test",
            "-c",
            "user.email=test@example.invalid",
        ])
        .args(args)
        .current_dir(path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .output()
        .unwrap();
    assert!(
        output.status.success(),
        "{}",
        String::from_utf8_lossy(&output.stderr)
    );
    String::from_utf8_lossy(&output.stdout).trim().to_string()
}

#[test]
fn flags_persist_independently_migrate_and_roll_back_on_save_failure() {
    let temp = tempfile::tempdir().unwrap();
    git(temp.path(), &["init", "--quiet", "repo"]);
    let config = temp.path().join("repositories.json");
    let service = Service::new(config.clone()).unwrap();
    let repo = service
        .add(temp.path().join("repo").to_str().unwrap())
        .unwrap();
    assert!(!repo.pinned && !repo.sync_disabled);
    service
        .update_repo_flags(
            &repo.path,
            RepoFlagsPatch {
                pinned: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    service
        .update_repo_flags(
            &repo.path,
            RepoFlagsPatch {
                sync_disabled: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    let reloaded = Service::new(config.clone()).unwrap();
    assert!(reloaded.list()[0].pinned && reloaded.list()[0].sync_disabled);
    let saved = fs::read(&config).unwrap();
    fs::remove_file(&config).unwrap();
    fs::create_dir(&config).unwrap();
    assert!(reloaded
        .update_repo_flags(
            &repo.path,
            RepoFlagsPatch {
                pinned: Some(false),
                sync_disabled: Some(false)
            }
        )
        .is_err());
    assert!(reloaded.list()[0].pinned && reloaded.list()[0].sync_disabled);
    fs::remove_dir(&config).unwrap();
    let mut legacy: serde_json::Value = serde_json::from_slice(&saved).unwrap();
    legacy["repos"][0].as_object_mut().unwrap().remove("pinned");
    legacy["repos"][0]
        .as_object_mut()
        .unwrap()
        .remove("syncDisabled");
    fs::write(&config, serde_json::to_vec(&legacy).unwrap()).unwrap();
    let migrated = Service::new(config).unwrap();
    assert!(!migrated.list()[0].pinned && !migrated.list()[0].sync_disabled);
    assert!(migrated
        .update_repo_flags("/missing", RepoFlagsPatch::default())
        .is_err());
}

#[test]
fn disabled_sync_blocks_integrating_and_pushing_but_fetch_still_works() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path();
    git(
        root,
        &[
            "init",
            "--quiet",
            "--bare",
            "--initial-branch=main",
            "remote",
        ],
    );
    git(root, &["clone", "--quiet", "remote", "local"]);
    let local = root.join("local");
    fs::write(local.join("file"), "initial\n").unwrap();
    git(&local, &["add", "."]);
    git(&local, &["commit", "--quiet", "-m", "initial"]);
    git(&local, &["push", "--quiet", "-u", "origin", "main"]);
    git(root, &["clone", "--quiet", "remote", "peer"]);
    let peer = root.join("peer");
    fs::write(peer.join("file"), "remote update\n").unwrap();
    git(&peer, &["commit", "--quiet", "-am", "remote update"]);
    git(&peer, &["push", "--quiet"]);
    let config = root.join("config/repositories.json");
    let service = Service::new(config.clone()).unwrap();
    let repo = service.add(local.to_str().unwrap()).unwrap();
    service
        .update_repo_flags(
            &repo.path,
            RepoFlagsPatch {
                pinned: Some(true),
                sync_disabled: Some(true),
            },
        )
        .unwrap();
    // A restarted backend must enforce the prohibition too, independently of the UI.
    let service = Service::new(config).unwrap();
    let head = git(&local, &["rev-parse", "HEAD"]);
    let index = fs::read(local.join(".git/index")).unwrap();
    assert_eq!(service.sync(&repo.path).unwrap_err().kind, "syncDisabled");
    assert_eq!(git(&local, &["rev-parse", "origin/main"]), head);
    assert_eq!(git(&local, &["rev-parse", "HEAD"]), head);
    assert_eq!(fs::read(local.join(".git/index")).unwrap(), index);
    assert_eq!(fs::read_to_string(local.join("file")).unwrap(), "initial\n");
    assert!(service
        .preview(&repo.path)
        .unwrap()
        .blockers
        .iter()
        .any(|b| b.contains("已禁止同步")));
    let fetched = service.fetch(&repo.path, None).unwrap();
    assert_eq!(fetched.behind, 1);
    assert_eq!(git(&local, &["rev-parse", "HEAD"]), head);
    service
        .update_repo_flags(
            &repo.path,
            RepoFlagsPatch {
                sync_disabled: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    assert!(service.list()[0].pinned);
    service.sync(&repo.path).unwrap();
    assert_eq!(
        git(&local, &["rev-parse", "HEAD"]),
        git(&peer, &["rev-parse", "HEAD"])
    );
    fs::write(local.join("file"), "local update\n").unwrap();
    git(&local, &["commit", "--quiet", "-am", "local update"]);
    let remote_head = git(&root.join("remote"), &["rev-parse", "main"]);
    service
        .update_repo_flags(
            &repo.path,
            RepoFlagsPatch {
                sync_disabled: Some(true),
                ..Default::default()
            },
        )
        .unwrap();
    assert_eq!(service.sync(&repo.path).unwrap_err().kind, "syncDisabled");
    assert_eq!(
        git(&root.join("remote"), &["rev-parse", "main"]),
        remote_head
    );
    service
        .update_repo_flags(
            &repo.path,
            RepoFlagsPatch {
                sync_disabled: Some(false),
                ..Default::default()
            },
        )
        .unwrap();
    service.sync(&repo.path).unwrap();
    assert_eq!(
        git(&root.join("remote"), &["rev-parse", "main"]),
        git(&local, &["rev-parse", "HEAD"])
    );
}
