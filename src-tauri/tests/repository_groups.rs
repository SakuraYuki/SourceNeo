use sourceneo_lib::{model::GroupEdit, service::Service};
use std::{fs, path::Path, process::Command};

fn init(path: &Path) -> String {
    let output = Command::new("git")
        .args(["init", "--quiet"])
        .arg(path)
        .output()
        .unwrap();
    assert!(output.status.success());
    path.to_str().unwrap().into()
}

#[test]
fn groups_persist_and_moves_are_atomic_and_exclusive() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("repositories.json");
    let service = Service::new(config.clone()).unwrap();
    let a = service.add(&init(&temp.path().join("a"))).unwrap();
    let b = service.add(&init(&temp.path().join("b"))).unwrap();
    service
        .edit_groups(GroupEdit::Create {
            name: " 工作 ".into(),
        })
        .unwrap();
    service
        .edit_groups(GroupEdit::Create {
            name: "个人".into(),
        })
        .unwrap();
    service
        .edit_groups(GroupEdit::Move {
            paths: vec![a.path.clone(), b.path.clone(), a.path.clone()],
            group: Some("工作".into()),
        })
        .unwrap();
    assert_eq!(
        service.groups()[0].paths,
        vec![a.path.clone(), b.path.clone()]
    );
    let before = fs::read(&config).unwrap();
    assert!(service
        .edit_groups(GroupEdit::Move {
            paths: vec![a.path.clone(), "/missing".into()],
            group: Some("个人".into())
        })
        .is_err());
    assert!(service
        .edit_groups(GroupEdit::Move {
            paths: vec![a.path.clone()],
            group: Some("missing".into())
        })
        .is_err());
    assert_eq!(fs::read(&config).unwrap(), before);
    service
        .edit_groups(GroupEdit::Move {
            paths: vec![a.path.clone()],
            group: Some("个人".into()),
        })
        .unwrap();
    assert_eq!(service.groups()[0].paths, vec![b.path.clone()]);
    assert_eq!(service.groups()[1].paths, vec![a.path.clone()]);
    service
        .edit_groups(GroupEdit::Rename {
            group: "个人".into(),
            name: "开源".into(),
        })
        .unwrap();
    let reloaded = Service::new(config).unwrap();
    assert_eq!(reloaded.groups(), service.groups());
    reloaded
        .edit_groups(GroupEdit::Remove {
            group: "开源".into(),
        })
        .unwrap();
    assert_eq!(reloaded.list().len(), 2);
    assert!(Path::new(&a.path).exists());
    reloaded
        .edit_groups(GroupEdit::Move {
            paths: vec![b.path.clone()],
            group: None,
        })
        .unwrap();
    assert!(reloaded.groups()[0].paths.is_empty());
    reloaded
        .edit_groups(GroupEdit::Move {
            paths: vec![b.path.clone()],
            group: Some("工作".into()),
        })
        .unwrap();
    reloaded.remove(&b.path).unwrap();
    assert!(reloaded.groups()[0].paths.is_empty());
    assert!(Path::new(&b.path).exists());
}

#[test]
fn old_config_migrates_and_invalid_edits_preserve_saved_groups() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("repositories.json");
    fs::write(&config, r#"{"repos":[]}"#).unwrap();
    let service = Service::new(config.clone()).unwrap();
    assert!(service.groups().is_empty());
    service
        .edit_groups(GroupEdit::Create {
            name: "工作".into(),
        })
        .unwrap();
    service
        .edit_groups(GroupEdit::Create {
            name: "个人".into(),
        })
        .unwrap();
    let before = fs::read(&config).unwrap();
    for name in ["", " \n ", "工作", "bad\nname", &"a".repeat(81)] {
        assert!(service
            .edit_groups(GroupEdit::Create { name: name.into() })
            .is_err());
    }
    assert!(service
        .edit_groups(GroupEdit::Rename {
            group: "工作".into(),
            name: "个人".into()
        })
        .is_err());
    assert!(service
        .edit_groups(GroupEdit::Rename {
            group: "missing".into(),
            name: "项目".into()
        })
        .is_err());
    assert!(service
        .edit_groups(GroupEdit::Remove {
            group: "missing".into()
        })
        .is_err());
    assert_eq!(fs::read(&config).unwrap(), before);
    // Force atomic replacement to fail without relying on OS permission behavior.
    fs::remove_file(&config).unwrap();
    fs::create_dir(&config).unwrap();
    let groups = service.groups();
    assert!(service
        .edit_groups(GroupEdit::Remove {
            group: "工作".into()
        })
        .is_err());
    assert_eq!(service.groups(), groups);
}
