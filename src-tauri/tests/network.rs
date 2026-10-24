use sourceneo_lib::{git, network::NetworkSettings, service::Service};
use std::{
    fs,
    io::{Read, Write},
    net::TcpListener,
    path::Path,
    process::Command,
    time::{Duration, Instant},
};

#[test]
fn validates_and_persists_proxy_without_changing_repository_settings() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("repositories.json");
    fs::write(
        &config,
        r#"{"repos":[],"groups":[],"tools":{"terminal":"","editor":""}}"#,
    )
    .unwrap();
    let original = fs::read(&config).unwrap();
    let service = Service::new(config.clone()).unwrap();
    assert!(service.network_settings().proxy.is_empty());
    for proxy in [
        "http://127.0.0.1:12345",
        "https://localhost:12345",
        "socks5://[::1]:1080",
        "socks5h://localhost:1080",
    ] {
        service
            .save_network_settings(NetworkSettings {
                proxy: format!(" {proxy} "),
                ..Default::default()
            })
            .unwrap();
        assert!(!Service::new(config.clone())
            .unwrap()
            .network_settings()
            .proxy
            .is_empty());
    }
    let before = service.network_settings().proxy;
    for proxy in [
        "127.0.0.1:12345",
        "file:///tmp/proxy",
        "http://user:secret@localhost:12345",
        "http://localhost:12345/path",
        "http://localhost:0",
        "socks5://localhost",
        "http://localhost:12345?token=secret",
        "http://local\nhost:12345",
    ] {
        assert!(
            service
                .save_network_settings(NetworkSettings {
                    proxy: proxy.into(),
                    ..Default::default()
                })
                .is_err(),
            "{proxy}"
        );
        assert_eq!(service.network_settings().proxy, before);
    }
    assert_eq!(fs::read(&config).unwrap(), original);
    service
        .save_network_settings(NetworkSettings::default())
        .unwrap();
    assert!(Service::new(config)
        .unwrap()
        .network_settings()
        .proxy
        .is_empty());
}

fn fixture_git(path: &Path, args: &[&str]) -> String {
    let out = Command::new("git")
        .args([
            "-c",
            "user.name=Network Test",
            "-c",
            "user.email=network@example.test",
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
        "{}",
        String::from_utf8_lossy(&out.stderr)
    );
    String::from_utf8(out.stdout).unwrap().trim().into()
}

// A real local HTTP proxy receives CONNECT. The invalid target domain can never
// succeed through direct DNS, so this exercises the actual Git child process.
fn through_proxy<T>(action: impl FnOnce(NetworkSettings) -> T) -> T {
    let listener = TcpListener::bind("127.0.0.1:0").unwrap();
    listener.set_nonblocking(true).unwrap();
    let settings = NetworkSettings {
        proxy: format!("http://{}", listener.local_addr().unwrap()),
        bypass: ".devops.aliyuncs.com".into(),
    }
    .validated()
    .unwrap();
    let server = std::thread::spawn(move || {
        let deadline = Instant::now() + Duration::from_secs(6);
        while Instant::now() < deadline {
            if let Ok((mut stream, _)) = listener.accept() {
                stream.set_nonblocking(false).unwrap();
                stream
                    .set_read_timeout(Some(Duration::from_secs(2)))
                    .unwrap();
                let mut bytes = [0; 8192];
                let n = stream.read(&mut bytes).unwrap();
                stream.write_all(b"HTTP/1.1 502 Bad Gateway\r\nContent-Length: 0\r\nConnection: close\r\n\r\n").unwrap();
                return String::from_utf8_lossy(&bytes[..n]).into_owned();
            }
            std::thread::sleep(Duration::from_millis(10));
        }
        panic!("Git did not connect to the configured proxy");
    });
    let result = action(settings);
    assert!(server
        .join()
        .unwrap()
        .starts_with("CONNECT sourceneo-proxy-fixture.invalid:443 "));
    result
}

#[test]
fn bypass_domains_migrate_validate_and_survive_restart() {
    let temp = tempfile::tempdir().unwrap();
    let config = temp.path().join("repositories.json");
    fs::write(
        temp.path().join("network.json"),
        r#"{"proxy":"http://127.0.0.1:12345"}"#,
    )
    .unwrap();
    let service = Service::new(config.clone()).unwrap();
    assert!(service.network_settings().bypass.is_empty());
    service
        .save_network_settings(NetworkSettings {
            proxy: "http://127.0.0.1:12345".into(),
            bypass: " .DEVOPS.aliyuncs.com， localhost,localhost ".into(),
        })
        .unwrap();
    assert_eq!(
        Service::new(config).unwrap().network_settings().bypass,
        ".devops.aliyuncs.com,localhost"
    );
    for invalid in ["https://example.com", "example.com/path", "example.com:443"] {
        assert!(service
            .save_network_settings(NetworkSettings {
                proxy: String::new(),
                bypass: invalid.into()
            })
            .is_err());
    }
}

#[test]
fn real_git_bypasses_proxy_only_for_matching_hosts() {
    fn endpoint(code: u16) -> (String, std::thread::JoinHandle<String>) {
        let listener = TcpListener::bind("127.0.0.1:0").unwrap();
        let address = listener.local_addr().unwrap().to_string();
        listener.set_nonblocking(true).unwrap();
        let handle = std::thread::spawn(move || {
            let deadline = Instant::now() + Duration::from_secs(6);
            while Instant::now() < deadline {
                if let Ok((mut stream, _)) = listener.accept() {
                    stream.set_nonblocking(false).unwrap();
                    stream
                        .set_read_timeout(Some(Duration::from_secs(2)))
                        .unwrap();
                    let mut bytes = [0; 8192];
                    let n = stream.read(&mut bytes).unwrap();
                    write!(
                        stream,
                        "HTTP/1.1 {code} Test\r\nContent-Length: 0\r\nConnection: close\r\n\r\n"
                    )
                    .unwrap();
                    return String::from_utf8_lossy(&bytes[..n]).into_owned();
                }
                std::thread::sleep(Duration::from_millis(10));
            }
            panic!("Git did not reach endpoint {code}");
        });
        (address, handle)
    }
    let temp = tempfile::tempdir().unwrap();
    let (origin, origin_server) = endpoint(503);
    let (proxy, proxy_server) = endpoint(502);
    let url = format!("http://{origin}/repo.git");
    let mut settings = NetworkSettings {
        proxy: format!("http://{proxy}"),
        bypass: ".other.invalid".into(),
    };
    let error = git::run_network(temp.path(), &["ls-remote", &url], &settings).unwrap_err();
    assert!(error.message.contains("502"), "{}", error.message);
    assert!(proxy_server
        .join()
        .unwrap()
        .starts_with(&format!("GET http://{origin}/repo.git")));
    settings.bypass = "127.0.0.1".into();
    let error = git::run_network(temp.path(), &["ls-remote", &url], &settings).unwrap_err();
    assert!(error.message.contains("503"), "{}", error.message);
    assert!(origin_server.join().unwrap().starts_with("GET /repo.git"));
}

#[test]
fn fetch_sync_clone_and_push_use_the_saved_proxy_in_real_git_processes() {
    let temp = tempfile::tempdir().unwrap();
    let root = temp.path().join("repo");
    fs::create_dir(&root).unwrap();
    // Match the stored repository path on Windows, without the extended-length prefix.
    let root = dunce::canonicalize(root).unwrap();
    fixture_git(&root, &["init", "-b", "main"]);
    fixture_git(&root, &["commit", "--allow-empty", "-m", "initial"]);
    fixture_git(
        &root,
        &[
            "remote",
            "add",
            "origin",
            "https://sourceneo-proxy-fixture.invalid/team/repo.git",
        ],
    );
    fixture_git(&root, &["update-ref", "refs/remotes/origin/main", "HEAD"]);
    fixture_git(&root, &["branch", "--set-upstream-to=origin/main", "main"]);
    let config = temp.path().join("config/repositories.json");
    let service = Service::new(config.clone()).unwrap();
    service.add(root.to_str().unwrap()).unwrap();
    let head = fixture_git(&root, &["rev-parse", "HEAD"]);
    through_proxy(|settings| {
        service.save_network_settings(settings).unwrap();
        let restarted = Service::new(config.clone()).unwrap();
        let e = restarted.fetch(root.to_str().unwrap(), None).unwrap_err();
        assert!(e.message.contains("502"), "{}", e.message);
    });
    through_proxy(|settings| {
        service.save_network_settings(settings).unwrap();
        let e = service.sync(root.to_str().unwrap()).unwrap_err();
        assert!(e.message.contains("502"), "{}", e.message);
    });
    through_proxy(|settings| {
        service.save_network_settings(settings).unwrap();
        let e = service
            .clone_repo(
                "https://sourceneo-proxy-fixture.invalid/team/repo.git",
                temp.path().to_str().unwrap(),
                "cloned",
            )
            .unwrap_err();
        assert!(e.message.contains("502"), "{}", e.message);
    });
    through_proxy(|settings| {
        let e = git::run_network(&root, &["push", "--dry-run", "origin", "main"], &settings)
            .unwrap_err();
        assert!(e.message.contains("502"), "{}", e.message);
    });
    assert_eq!(fixture_git(&root, &["rev-parse", "HEAD"]), head);
    assert!(fixture_git(&root, &["status", "--porcelain"]).is_empty());
}
