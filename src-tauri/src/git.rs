use crate::model::*;
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::{
        atomic::{AtomicBool, Ordering},
        mpsc,
    },
    thread,
    time::{Duration, Instant},
};

pub fn redact(input: &str) -> String {
    use std::sync::OnceLock;
    static URL: OnceLock<regex::Regex> = OnceLock::new();
    static SECRET: OnceLock<regex::Regex> = OnceLock::new();
    static AUTH: OnceLock<regex::Regex> = OnceLock::new();
    let s = URL
        // Invalid, unencoded credentials can contain several @ signs. Redact through
        // the last one in the authority, without consuming @ in paths or queries.
        .get_or_init(|| regex::Regex::new(r"(?i)(https?|ssh)://[^\s/?#]*@").unwrap())
        .replace_all(input, "$1://[redacted]@");
    let s = AUTH
        .get_or_init(|| regex::Regex::new(r"(?i)authorization:[^\r\n]+").unwrap())
        .replace_all(&s, "Authorization: [redacted]");
    SECRET.get_or_init(||regex::Regex::new(r"(?i)([?&](?:access_token|token|key|password|auth|signature|credential)=|(?:password|token|secret)[:=]\s*)[^\s&]+").unwrap()).replace_all(&s,"$1[redacted]").into_owned()
}
fn classify(s: &str) -> &'static str {
    let s = s.to_lowercase();
    if [
        "authentication",
        "permission denied",
        "could not read username",
        "invalid username",
        "access denied",
    ]
    .iter()
    .any(|x| s.contains(x))
    {
        "auth"
    } else if [
        "could not resolve",
        "unable to access",
        "connection",
        "network",
        "timed out",
        "no route",
        "unreachable",
    ]
    .iter()
    .any(|x| s.contains(x))
    {
        "network"
    } else if s.contains(".lock") || s.contains("another git process") {
        "busy"
    } else {
        "git"
    }
}
fn read_capped(mut pipe: impl Read, first_record: bool) -> (Vec<u8>, bool) {
    let mut all = Vec::new();
    let mut truncated = false;
    let mut buf = [0u8; 8192];
    while let Ok(n) = pipe.read(&mut buf) {
        if n == 0 {
            break;
        }
        let end = first_record
            .then(|| buf[..n].iter().position(|b| *b == 0))
            .flatten();
        let count = end.map_or(n, |i| i + 1);
        let room = (8 * 1024 * 1024usize).saturating_sub(all.len());
        truncated |= count > room;
        all.extend_from_slice(&buf[..count.min(room)]);
        if end.is_some() || (first_record && truncated) {
            break;
        }
    }
    (all, truncated)
}
pub fn run(path: &Path, args: &[&str]) -> Result<String> {
    run_timeout(path, args, Duration::from_secs(90))
}
fn check_directory(path: &Path) -> Result<()> {
    let metadata = std::fs::metadata(path).map_err(|e| {
        GitError::new(
            if e.kind() == std::io::ErrorKind::PermissionDenied {
                "permission"
            } else {
                "path"
            },
            format!("无法访问仓库目录 {}：{e}", path.display()),
        )
    })?;
    if !metadata.is_dir() {
        return Err(GitError::new(
            "path",
            format!("仓库路径不是目录：{}", path.display()),
        ));
    }
    Ok(())
}
fn spawn_error(path: &Path, error: std::io::Error) -> GitError {
    // The directory can disappear between validation and process creation.
    if let Err(path_error) = check_directory(path) {
        return path_error;
    }
    match error.kind() {
        std::io::ErrorKind::NotFound => GitError::new(
            "environment",
            format!("找不到 Git，请确认已安装 Git 并加入 PATH：{error}"),
        ),
        std::io::ErrorKind::PermissionDenied => GitError::new(
            "permission",
            format!("没有权限进入仓库目录或执行 Git，请检查目录及程序权限：{error}"),
        ),
        _ => GitError::new("environment", format!("无法启动 Git：{error}")),
    }
}
pub fn run_timeout(path: &Path, args: &[&str], timeout: Duration) -> Result<String> {
    run_output(path, args, timeout, false, false, None, None)
}

pub fn run_network(
    path: &Path,
    args: &[&str],
    network: &crate::network::NetworkSettings,
) -> Result<String> {
    run_output(
        path,
        args,
        Duration::from_secs(90),
        false,
        false,
        None,
        Some(network),
    )
}

// Internal read-only query: only the first ignored path is needed to refuse a
// write. Never invoke ls-files without explicit, literal, scoped pathspecs.
pub(crate) fn first_ignored_path(path: &Path, probes: &[String]) -> Result<Option<String>> {
    if probes.is_empty() {
        return Ok(None);
    }
    // Literal pathspecs are otherwise case-sensitive even on Windows. Match
    // the filesystem / Git case mode so Config.json cannot overwrite an
    // ignored config.json (including a differently cased parent component).
    let ignore_case = cfg!(windows)
        || run(
            path,
            &[
                "config",
                "--type=bool",
                "--default",
                "false",
                "--get",
                "core.ignoreCase",
            ],
        )?
        .trim()
            == "true";
    let magic = if ignore_case {
        ":(top,literal,icase)"
    } else {
        ":(top,literal)"
    };
    let specs: Vec<_> = probes.iter().map(|p| format!("{magic}{p}")).collect();
    let mut args = vec![
        "-c",
        "core.fsmonitor=false",
        "ls-files",
        "--others",
        "--ignored",
        "--exclude-standard",
        "--directory",
        "--no-empty-directory",
        "-z",
        "--",
    ];
    args.extend(specs.iter().map(String::as_str));
    let output = run_output(
        path,
        &args,
        Duration::from_secs(90),
        true,
        false,
        None,
        None,
    )?;
    if output.is_empty() {
        return Ok(None);
    }
    output
        .strip_suffix('\0')
        .map(|p| Some(p.to_owned()))
        .ok_or_else(|| GitError::new("git", "忽略路径记录不完整，同步已暂停"))
}

fn run_output(
    path: &Path,
    args: &[&str],
    timeout: Duration,
    first_record: bool,
    online_only: bool,
    cancel: Option<&AtomicBool>,
    network: Option<&crate::network::NetworkSettings>,
) -> Result<String> {
    check_directory(path)?;
    let mut command = Command::new("git");
    if let Some(network) = network {
        network.apply(&mut command);
    }
    command
        .arg("--no-pager")
        .arg("-c")
        .arg("color.ui=false")
        .arg("-c")
        .arg("core.quotepath=false")
        .args(args)
        .current_dir(path)
        .env("GIT_TERMINAL_PROMPT", "0")
        .env("GCM_INTERACTIVE", "Never")
        .env("GIT_OPTIONAL_LOCKS", "0")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Do not inherit repository redirection from the process that launched the GUI.
    for key in [
        "GIT_DIR",
        "GIT_WORK_TREE",
        "GIT_INDEX_FILE",
        "GIT_COMMON_DIR",
        "GIT_OBJECT_DIRECTORY",
        "GIT_ALTERNATE_OBJECT_DIRECTORIES",
    ] {
        command.env_remove(key);
    }
    #[cfg(unix)]
    {
        use std::os::unix::process::CommandExt;
        unsafe {
            command.pre_exec(|| {
                if libc::setpgid(0, 0) == -1 {
                    return Err(std::io::Error::last_os_error());
                }
                Ok(())
            });
        }
    }
    #[cfg(windows)]
    {
        use std::os::windows::process::CommandExt;
        command.creation_flags(0x08000000 | 0x00000200);
    }
    if online_only {
        command.env("GIT_ALLOW_PROTOCOL", "https:ssh");
    }
    let mut child = command.spawn().map_err(|e| spawn_error(path, e))?;
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = out_tx.send(read_capped(out, first_record));
    });
    thread::spawn(move || {
        let _ = err_tx.send(read_capped(err, false));
    });
    let start = Instant::now();
    let mut status = None;
    let mut stdout = None;
    let mut stderr = None;
    loop {
        if status.is_none() {
            status = child
                .try_wait()
                .map_err(|e| GitError::new("io", e.to_string()))?;
        }
        if stdout.is_none() {
            stdout = out_rx.try_recv().ok();
        }
        if stderr.is_none() {
            stderr = err_rx.try_recv().ok();
        }
        if first_record {
            if let Some((bytes, false)) = &stdout {
                if bytes.last() == Some(&0) {
                    // This mode is restricted to the read-only ls-files query.
                    // Stop as soon as a blocker is found; do not drain millions
                    // of paths or treat the resulting broken pipe as an error.
                    let _ = child.kill();
                    let _ = child.wait();
                    return Ok(String::from_utf8_lossy(bytes).into_owned());
                }
            }
        }
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            break;
        }
        let cancelled = cancel.is_some_and(|flag| flag.load(Ordering::Relaxed));
        if cancelled || start.elapsed() > timeout {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                use std::os::windows::process::CommandExt;
                let _ = Command::new("taskkill")
                    .creation_flags(0x08000000)
                    .args(["/PID", &child.id().to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(GitError::new(
                if cancelled { "cancelled" } else { "timeout" },
                if cancelled {
                    "已取消克隆，已尝试终止本次进程树"
                } else {
                    "Git 操作超时，已停止等待并尝试终止本次进程树；请检查网络和顶部“网络设置”中的代理，以及 SSH、凭据助手或 hooks"
                },
            ));
        }
        thread::sleep(Duration::from_millis(20));
    }
    let status = status.unwrap();
    let (stdout, out_truncated) = stdout.unwrap();
    let (stderr, err_truncated) = stderr.unwrap();
    if out_truncated || err_truncated {
        return Err(GitError::new(
            "outputLimit",
            "Git 输出超过 8 MiB 上限，已停止解析；请减少历史加载量或检查仓库状态",
        ));
    }
    if !status.success() {
        let e = String::from_utf8_lossy(&stderr);
        return Err(GitError::new(
            classify(&e),
            if e.trim().is_empty() {
                format!("Git 退出：{status}")
            } else {
                e.trim().to_string()
            },
        ));
    }
    Ok(String::from_utf8_lossy(&stdout).into_owned())
}
pub(crate) fn clone_online(
    parent: &Path,
    url: &str,
    target: &Path,
    cancel: &AtomicBool,
    network: &crate::network::NetworkSettings,
) -> Result<String> {
    run_output(
        parent,
        &[
            "clone",
            "--no-recurse-submodules",
            "--",
            url,
            &target.to_string_lossy(),
        ],
        Duration::from_secs(900),
        false,
        true,
        Some(cancel),
        Some(network),
    )
}

fn absolute(base: &Path, value: &str) -> Result<PathBuf> {
    // Remove only Git's record terminator, preserving whitespace in real path names.
    let value = value.strip_suffix('\n').unwrap_or(value);
    #[cfg(windows)]
    let value = value.strip_suffix('\r').unwrap_or(value);
    let p = PathBuf::from(value);
    let p = if p.is_absolute() { p } else { base.join(p) };
    dunce::canonicalize(p).map_err(|e| GitError::new("path", e.to_string()))
}
pub fn discover(path: &str) -> Result<Repo> {
    let original = Path::new(path);
    // Existing paths (and indeterminate access errors) must never select a trimmed sibling.
    let base = if matches!(original.try_exists(), Ok(false)) {
        Path::new(path.trim())
    } else {
        original
    };
    let root = run(base, &["rev-parse", "--show-toplevel"])?;
    let root = absolute(base, &root)?;
    let common = run(&root, &["rev-parse", "--git-common-dir"])?;
    let common = absolute(&root, &common)?;
    Ok(Repo {
        path: root.to_string_lossy().into_owned(),
        name: root
            .file_name()
            .unwrap_or_default()
            .to_string_lossy()
            .into_owned(),
        common_dir: common.to_string_lossy().into_owned(),
        pinned: false,
        sync_disabled: false,
    })
}
pub fn git_dir(repo: &Repo) -> Result<PathBuf> {
    absolute(
        Path::new(&repo.path),
        &run(Path::new(&repo.path), &["rev-parse", "--git-dir"])?,
    )
}
pub fn check_locks(repo: &Repo) -> Result<()> {
    let dir = git_dir(repo)?;
    let common = Path::new(&repo.common_dir);
    for p in [
        dir.join("index.lock"),
        dir.join("HEAD.lock"),
        common.join("packed-refs.lock"),
        common.join("shallow.lock"),
        common.join("config.lock"),
    ] {
        if p.exists() {
            return Err(GitError::new(
                "busy",
                "检测到 Git 锁，可能有外部 AI/Git 操作正在运行；请稍后重试，不要手动删除锁",
            ));
        }
    }
    Ok(())
}
pub fn inspect(repo: &Repo, last_fetched: Option<u64>) -> Result<RepoStatus> {
    let path = Path::new(&repo.path);
    let raw = run(
        path,
        &[
            "status",
            "--porcelain=v2",
            "--branch",
            "-z",
            "--untracked-files=all",
        ],
    )?;
    let mut s = RepoStatus {
        path: repo.path.clone(),
        branch: String::new(),
        head: String::new(),
        upstream: None,
        ahead: 0,
        behind: 0,
        changed: 0,
        conflicts: vec![],
        operation: None,
        kind: StatusKind::Synced,
        message: String::new(),
        error_kind: None,
        reason: None,
        last_fetched,
        checked_at: now(),
        shallow: false,
    };
    let mut skip = false;
    for line in raw.split('\0') {
        if skip {
            skip = false;
            continue;
        }
        if let Some(v) = line.strip_prefix("# branch.oid ") {
            s.head = v.into()
        } else if let Some(v) = line.strip_prefix("# branch.head ") {
            s.branch = v.into()
        } else if let Some(v) = line.strip_prefix("# branch.upstream ") {
            s.upstream = Some(v.into())
        } else if let Some(v) = line.strip_prefix("# branch.ab ") {
            let mut it = v.split_whitespace();
            s.ahead = it
                .next()
                .unwrap_or("+0")
                .trim_start_matches('+')
                .parse()
                .unwrap_or(0);
            s.behind = it
                .next()
                .unwrap_or("-0")
                .trim_start_matches('-')
                .parse()
                .unwrap_or(0);
        } else if line.starts_with("u ") {
            s.changed += 1;
            if let Some(p) = line.splitn(11, ' ').nth(10) {
                s.conflicts.push(p.into());
            }
        } else if line.starts_with("1 ") || line.starts_with("? ") {
            s.changed += 1
        } else if line.starts_with("2 ") {
            s.changed += 1;
            skip = true
        }
    }
    let dir = git_dir(repo)?;
    for (file, label) in [
        ("rebase-merge", "rebase"),
        ("rebase-apply", "rebase / am"),
        ("MERGE_HEAD", "merge"),
        ("CHERRY_PICK_HEAD", "cherry-pick"),
        ("REVERT_HEAD", "revert"),
        ("sequencer", "sequencer"),
        ("BISECT_START", "bisect"),
    ] {
        if dir.join(file).exists() {
            s.operation = Some(label.into());
            break;
        }
    }
    s.shallow = run(path, &["rev-parse", "--is-shallow-repository"])?.trim() == "true";
    let (kind, msg) = if !s.conflicts.is_empty() {
        (StatusKind::Conflict, "存在文件冲突，请交给 AI 处理")
    } else if s.operation.is_some() {
        (StatusKind::Attention, "Git 操作尚未完成，同步已暂停")
    } else if s.head == "(initial)" {
        (StatusKind::Attention, "空仓库，尚无提交")
    } else if s.branch == "(detached)" {
        (StatusKind::Attention, "游离 HEAD，请先切换到本地分支")
    } else if s.upstream.is_none() {
        (StatusKind::Attention, "尚未设置 upstream，请交给 AI 配置")
    } else if run(path, &["rev-parse", "--verify", "@{upstream}"]).is_err() {
        s.reason = Some("upstreamMissing".into());
        (StatusKind::Attention, "upstream 已失效，请检查远程分支")
    } else if s.ahead > 0 && s.behind > 0 {
        (
            StatusKind::Diverged,
            "本地与远程各有新增提交；同步将按 Git 配置整合，分叉不代表文件冲突",
        )
    } else if s.behind > 0 {
        (StatusKind::Behind, "有远程提交可快进拉取")
    } else if s.ahead > 0 {
        (StatusKind::Ahead, "有本地提交待推送")
    } else if s.changed > 0 {
        (
            StatusKind::Attention,
            "存在未提交改动，交给 AI 提交后再同步",
        )
    } else {
        (StatusKind::Synced, "本地与已知远程一致")
    };
    s.kind = kind;
    s.message = msg.into();
    if let Some(operation) = &s.operation {
        s.message = if s.conflicts.is_empty() {
            format!("{operation} 尚未完成，同步已暂停；请在外部 Git 工具完成或中止后再同步，不会自动推送")
        } else {
            format!("{operation} 发生冲突（{} 个文件），同步已暂停且未推送；请解决冲突后继续或中止 {operation}", s.conflicts.len())
        };
    }
    if s.changed > 0 && matches!(s.kind, StatusKind::Ahead | StatusKind::Behind) {
        s.message.push_str("；存在未提交改动，同步写操作将暂停");
    }
    Ok(s)
}
pub fn history(repo: &Repo, limit: usize, all_branches: bool) -> Result<History> {
    let path = Path::new(&repo.path);
    let shallow = run(path, &["rev-parse", "--is-shallow-repository"])?.trim() == "true";
    let has_head = run(path, &["rev-parse", "--verify", "HEAD"]).is_ok();
    // An unborn branch has no current history, even if other refs have commits.
    if !has_head && !all_branches {
        return Ok(History {
            commits: Vec::new(),
            has_more: false,
            shallow,
        });
    }
    let mut args = vec![
        "log",
        "--encoding=UTF-8",
        "--no-show-signature",
        "--no-notes",
        "--no-decorate",
        "--no-patch",
        "--topo-order",
        "-z",
        "--format=%H%x00%P%x00%s%x00%an%x00%aI",
    ];
    if all_branches {
        args.push("--all");
    }
    if has_head {
        args.push("HEAD")
    };
    let count = format!("-{}", limit.clamp(1, 5000) + 1);
    args.push(&count);
    args.push("--");
    let raw = run(path, &args)?;
    let mut refs: HashMap<String, Vec<String>> = HashMap::new();
    for line in run(
        path,
        &[
            "for-each-ref",
            "--format=%(objectname)%00%(*objectname)%00%(refname)",
            "refs/heads",
            "refs/remotes",
            "refs/tags",
        ],
    )?
    .lines()
    {
        let p: Vec<_> = line.split('\0').collect();
        if p.len() == 3 {
            let oid = if p[1].is_empty() { p[0] } else { p[1] };
            refs.entry(oid.into()).or_default().push(p[2].into());
        }
    }
    if has_head {
        let head = run(path, &["rev-parse", "HEAD"])?;
        refs.entry(head.trim().into())
            .or_default()
            .push("HEAD".into());
    }
    let fields: Vec<_> = raw.split_terminator('\0').collect();
    let mut commits = Vec::new();
    for p in fields.as_chunks::<5>().0 {
        commits.push(Commit {
            hash: p[0].into(),
            parents: p[1].split_whitespace().map(String::from).collect(),
            subject: redact(p[2]),
            author: redact(p[3]),
            date: p[4].into(),
            refs: refs.remove(p[0]).unwrap_or_default(),
        });
    }
    let has_more = commits.len() > limit;
    commits.truncate(limit);
    Ok(History {
        commits,
        has_more,
        shallow,
    })
}

pub fn branches(repo: &Repo) -> Result<Vec<Branch>> {
    let raw = run(
        Path::new(&repo.path),
        &[
            "for-each-ref",
            "--sort=refname",
            "--format=%(refname)%00%(symref)%00%(HEAD)",
            "refs/heads/",
            "refs/remotes/",
        ],
    )?;
    Ok(raw
        .lines()
        .filter_map(|line| {
            let fields: Vec<_> = line.split('\0').collect();
            if fields.len() != 3 || !fields[1].is_empty() {
                return None;
            }
            let remote = fields[0].starts_with("refs/remotes/");
            Some(Branch {
                reference: fields[0].into(),
                name: fields[0]
                    .strip_prefix(if remote {
                        "refs/remotes/"
                    } else {
                        "refs/heads/"
                    })?
                    .into(),
                remote,
                current: fields[2] == "*",
            })
        })
        .collect())
}

pub fn working_changes(repo: &Repo) -> Result<Vec<ChangedFile>> {
    let raw = run(
        Path::new(&repo.path),
        &["status", "--porcelain=v1", "-z", "--untracked-files=all"],
    )?;
    let mut records = raw.split_terminator('\0');
    let mut files = Vec::new();
    while let Some(record) = records.next() {
        let status = record
            .get(..2)
            .ok_or_else(|| GitError::new("parse", "工作区状态格式无效"))?;
        let path = record
            .get(3..)
            .ok_or_else(|| GitError::new("parse", "工作区路径格式无效"))?;
        let old_path = if status.contains(['R', 'C']) {
            Some(
                records
                    .next()
                    .ok_or_else(|| GitError::new("parse", "工作区重命名格式无效"))?
                    .into(),
            )
        } else {
            None
        };
        files.push(ChangedFile {
            path: path.into(),
            old_path,
            status: status.into(),
        });
    }
    Ok(files)
}

fn commit_pair(repo: &Repo, hash: &str) -> Result<(String, Option<String>)> {
    if !matches!(hash.len(), 40 | 64) || !hash.bytes().all(|b| b.is_ascii_hexdigit()) {
        return Err(GitError::new("revision", "请选择有效的完整提交 SHA"));
    }
    let oid = run(
        Path::new(&repo.path),
        &["rev-parse", "--verify", &format!("{hash}^{{commit}}")],
    )?;
    let oid = oid.trim().to_string();
    // Read real parent headers: rev-list hides parents at a shallow boundary.
    let raw = run(Path::new(&repo.path), &["cat-file", "commit", &oid])?;
    let parent = raw
        .lines()
        .take_while(|line| !line.is_empty())
        .find_map(|line| line.strip_prefix("parent ").map(String::from));
    if parent.as_ref().is_some_and(|parent| {
        parent.len() != oid.len() || !parent.bytes().all(|byte| byte.is_ascii_hexdigit())
    }) {
        return Err(GitError::new("revision", "提交的父对象 SHA 格式无效"));
    }
    Ok((oid, parent))
}

pub(crate) fn tree_diff(
    repo: &Repo,
    hash: &str,
    parent: Option<&str>,
    options: &[&str],
    paths: &[&str],
) -> Result<String> {
    let mut args = vec![
        "--literal-pathspecs",
        "diff-tree",
        "--root",
        "-r",
        "--no-commit-id",
        "--find-renames",
        "--no-ext-diff",
        "--no-textconv",
        "--no-color",
        "--no-relative",
    ];
    args.extend_from_slice(options);
    if let Some(parent) = parent {
        args.push(parent);
    }
    args.push(hash);
    args.push("--");
    args.extend_from_slice(paths);
    run(Path::new(&repo.path), &args)
}

pub fn commit_changes(repo: &Repo, hash: &str) -> Result<CommitChanges> {
    let (hash, parent) = commit_pair(repo, hash)?;
    let raw = tree_diff(
        repo,
        &hash,
        parent.as_deref(),
        &["--name-status", "-z"],
        &[],
    )?;
    Ok(CommitChanges {
        hash,
        parent,
        files: parse_changes(&raw)?,
    })
}

pub(crate) fn parse_changes(raw: &str) -> Result<Vec<ChangedFile>> {
    let mut fields = raw.split_terminator('\0');
    let mut files = Vec::new();
    while let Some(status) = fields.next() {
        let first = fields
            .next()
            .ok_or_else(|| GitError::new("parse", "文件列表格式无效"))?;
        let renamed = status.starts_with('R') || status.starts_with('C');
        let (old_path, path) = if renamed {
            (
                Some(first.into()),
                fields
                    .next()
                    .ok_or_else(|| GitError::new("parse", "重命名文件格式无效"))?,
            )
        } else {
            (None, first)
        };
        files.push(ChangedFile {
            path: path.into(),
            old_path,
            status: status.into(),
        });
    }
    Ok(files)
}

pub fn file_diff(repo: &Repo, hash: &str, path: &str) -> Result<String> {
    let changes = commit_changes(repo, hash)?;
    let file = changes
        .files
        .iter()
        .find(|file| file.path == path)
        .ok_or_else(|| GitError::new("path", "该文件不在此提交的变更列表中"))?;
    let mut paths = vec![file.path.as_str()];
    if let Some(old) = &file.old_path {
        paths.push(old);
    }
    tree_diff(
        repo,
        &changes.hash,
        changes.parent.as_deref(),
        &["--patch", "--src-prefix=a/", "--dst-prefix=b/"],
        &paths,
    )
}

pub fn working_diff(repo: &Repo, path: &str, side: &str) -> Result<String> {
    if !matches!(side, "staged" | "unstaged" | "untracked") {
        return Err(GitError::new("path", "无效的差异类型"));
    }
    let file = working_changes(repo)?
        .into_iter()
        .find(|f| {
            f.path == path
                && (f.status.contains('U')
                    || matches!(f.status.as_str(), "AA" | "DD")
                    || match side {
                        "untracked" => f.status == "??",
                        "staged" => f.status != "??" && f.status.as_bytes()[0] != b' ',
                        "unstaged" => f.status != "??" && f.status.as_bytes()[1] != b' ',
                        _ => false,
                    })
        })
        .ok_or_else(|| GitError::new("changed", "文件状态已变化，请刷新文件列表"))?;
    if file.status.contains('U') || matches!(file.status.as_str(), "AA" | "DD") {
        return Ok("冲突尚未解决；请在外部工具检查 index 的冲突阶段和工作区内容。".into());
    }
    let root = Path::new(&repo.path);
    let mut current = root.to_path_buf();
    for component in Path::new(path).components() {
        if !matches!(component, std::path::Component::Normal(_)) {
            return Err(GitError::new("path", "无效的仓库相对路径"));
        }
        current.push(component);
        if let Ok(meta) = std::fs::symlink_metadata(&current) {
            if side != "staged" && meta.file_type().is_symlink() {
                return Ok("符号链接：不读取链接目标；请使用外部工具检查。".into());
            }
        }
    }
    let patch = if file.status == "??" {
        if side != "untracked" {
            return Err(GitError::new("changed", "请选择未跟踪内容"));
        }
        let handle = open_preview(root, path)?;
        if !handle
            .metadata()
            .map_err(|e| GitError::new("io", e.to_string()))?
            .is_file()
        {
            return Err(GitError::new("path", "仅预览普通文件"));
        }
        if !dunce::canonicalize(&current)
            .map_err(|e| GitError::new("path", e.to_string()))?
            .starts_with(root)
        {
            return Err(GitError::new("path", "文件已移到仓库之外"));
        }
        let mut bytes = Vec::new();
        handle
            .take(256 * 1024 + 1)
            .read_to_end(&mut bytes)
            .map_err(|e| GitError::new("io", e.to_string()))?;
        if bytes.contains(&0) {
            "二进制文件：不提供文本预览".into()
        } else if bytes.is_empty() {
            "空文件（未跟踪）".into()
        } else {
            String::from_utf8_lossy(&bytes).into_owned()
        }
    } else {
        let index = match side {
            "staged" => 0,
            "unstaged" => 1,
            _ => return Err(GitError::new("path", "无效的差异类型")),
        };
        if file.status.as_bytes()[index] == b' ' {
            return Err(GitError::new("changed", "此状态已无差异，请刷新"));
        }
        let mut args = vec![
            "--literal-pathspecs",
            "diff",
            "--no-ext-diff",
            "--no-textconv",
            "--no-color",
            "--no-relative",
            "--find-renames",
            "--patch",
            "--src-prefix=a/",
            "--dst-prefix=b/",
        ];
        if side == "staged" {
            args.push("--cached");
        }
        args.extend(["--", path]);
        if let Some(old) = &file.old_path {
            args.push(old);
        }
        run(root, &args)?
    };
    let mut end = patch.len().min(256 * 1024);
    while !patch.is_char_boundary(end) {
        end -= 1;
    }
    let lines: Vec<_> = patch[..end].lines().take(2000).collect();
    let mut result = lines.join("\n");
    if end < patch.len() || patch.lines().count() > 2000 {
        result.push_str("\n[预览已截断：最多 256 KiB / 2,000 行]");
    }
    Ok(if result.is_empty() {
        "无文本差异（空文件、模式变化或状态已变化；请刷新）".into()
    } else {
        result
    })
}

// Open without following links. Validate the opened Windows handle (rather than
// a second path lookup), and walk Unix directory descriptors without symlinks.
fn open_preview(root: &Path, relative: &str) -> Result<std::fs::File> {
    #[cfg(windows)]
    {
        use std::os::windows::{fs::OpenOptionsExt, io::AsRawHandle};
        unsafe extern "system" {
            fn GetFinalPathNameByHandleW(
                handle: *mut std::ffi::c_void,
                buffer: *mut u16,
                length: u32,
                flags: u32,
            ) -> u32;
        }
        let file = std::fs::OpenOptions::new()
            .read(true)
            .custom_flags(0x00200000)
            .open(root.join(relative))
            .map_err(|e| GitError::new("io", e.to_string()))?;
        let mut buffer = vec![0u16; 32768];
        let n = unsafe {
            GetFinalPathNameByHandleW(
                file.as_raw_handle(),
                buffer.as_mut_ptr(),
                buffer.len() as u32,
                0,
            )
        } as usize;
        if n == 0
            || n >= buffer.len()
            || file
                .metadata()
                .map_err(|e| GitError::new("io", e.to_string()))?
                .file_type()
                .is_symlink()
        {
            return Err(GitError::new("path", "无法安全读取符号链接或文件路径"));
        }
        let actual = PathBuf::from(String::from_utf16_lossy(&buffer[..n]));
        if !dunce::simplified(&actual).starts_with(dunce::simplified(root)) {
            return Err(GitError::new("path", "文件已移到仓库之外"));
        }
        Ok(file)
    }
    #[cfg(unix)]
    {
        use std::os::fd::{AsRawFd, FromRawFd};
        let mut dir = std::fs::File::open(root).map_err(|e| GitError::new("io", e.to_string()))?;
        let components: Vec<_> = Path::new(relative).components().collect();
        for (i, component) in components.iter().enumerate() {
            use std::os::unix::ffi::OsStrExt;
            let name = std::ffi::CString::new(component.as_os_str().as_bytes())
                .map_err(|_| GitError::new("path", "无效路径"))?;
            let flags = libc::O_RDONLY
                | libc::O_NOFOLLOW
                | libc::O_NONBLOCK
                | if i + 1 < components.len() {
                    libc::O_DIRECTORY
                } else {
                    0
                };
            let fd = unsafe { libc::openat(dir.as_raw_fd(), name.as_ptr(), flags) };
            if fd < 0 {
                return Err(GitError::new(
                    "path",
                    std::io::Error::last_os_error().to_string(),
                ));
            }
            dir = unsafe { std::fs::File::from_raw_fd(fd) };
        }
        Ok(dir)
    }
}

pub fn fetch_remotes(repo: &Repo) -> Result<Vec<String>> {
    let path = Path::new(&repo.path);
    let mut remotes: Vec<String> = run(path, &["remote"])?.lines().map(String::from).collect();
    let branch = run(path, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
    if !branch.is_empty() {
        let configured = run(
            path,
            &[
                "config",
                "--get",
                &format!("branch.{}.remote", branch.trim()),
            ],
        )
        .unwrap_or_default();
        if let Some(i) = remotes.iter().position(|r| r == configured.trim()) {
            remotes.swap(0, i);
        }
    }
    Ok(remotes)
}

#[cfg(all(test, windows))]
mod preview_safety_tests {
    use super::*;
    #[test]
    fn opened_handle_cannot_escape_repository_through_a_junction() {
        use std::os::windows::process::CommandExt;
        let temp = tempfile::tempdir().unwrap();
        let root = temp.path().join("repo");
        let outside = temp.path().join("outside");
        std::fs::create_dir(&root).unwrap();
        std::fs::create_dir(&outside).unwrap();
        std::fs::write(outside.join("secret.txt"), "must not be read").unwrap();
        let system = PathBuf::from(std::env::var_os("SystemRoot").unwrap())
            .join("System32/WindowsPowerShell/v1.0/powershell.exe");
        let output = Command::new(system)
            .args(["-NoProfile", "-Command", "New-Item -ItemType Junction -Path $env:SRC_LINK -Target $env:SRC_TARGET | Out-Null"])
            .env("SRC_LINK", root.join("linked")).env("SRC_TARGET", &outside)
            .creation_flags(0x08000000).output().unwrap();
        assert!(
            output.status.success(),
            "{}",
            String::from_utf8_lossy(&output.stderr)
        );
        assert!(open_preview(&root, "linked/secret.txt").is_err());
        std::fs::remove_dir(root.join("linked")).unwrap();
    }
}

pub fn fetch_only(repo: &Repo, selected: Option<&str>) -> Result<()> {
    fetch_with_network(repo, selected, &crate::network::NetworkSettings::default())
}

pub fn fetch_with_network(
    repo: &Repo,
    selected: Option<&str>,
    network: &crate::network::NetworkSettings,
) -> Result<()> {
    let path = Path::new(&repo.path);
    let remotes = fetch_remotes(repo)?;
    let branch = run(path, &["symbolic-ref", "--quiet", "--short", "HEAD"]).unwrap_or_default();
    let configured = if branch.is_empty() {
        String::new()
    } else {
        run(
            path,
            &[
                "config",
                "--get",
                &format!("branch.{}.remote", branch.trim()),
            ],
        )
        .unwrap_or_default()
    };
    let remote = if let Some(name) = selected {
        remotes.iter().find(|r| r.as_str() == name)
    } else if remotes.iter().any(|r| r == configured.trim()) {
        remotes.iter().find(|r| r.as_str() == configured.trim())
    } else if remotes.len() == 1 {
        remotes.first()
    } else {
        None
    }
    .ok_or_else(|| {
        GitError::new(
            "remote",
            if remotes.is_empty() {
                "没有命名远端；请在外部工具添加 remote".into()
            } else {
                format!(
                    "请选择获取远端（{}）；可在仓库菜单中选择“选择获取远端…”后重试",
                    remotes.join("、")
                )
            },
        )
    })?;
    run(
        path,
        &["check-ref-format", &format!("refs/remotes/{remote}/probe")],
    )?;
    // Never let configured mappings target local branches/tags. Preserve safe
    // custom tracking maps so @{upstream} cannot silently stay stale after fetch.
    let mut mappings = vec![format!("+refs/heads/*:refs/remotes/{remote}/*")];
    let configured_maps = run(
        path,
        &[
            "config",
            "--null",
            "--get-all",
            &format!("remote.{remote}.fetch"),
        ],
    )
    .unwrap_or_default();
    for mapping in configured_maps.split_terminator('\0') {
        if mappings.iter().any(|existing| existing == mapping) {
            continue;
        }
        if let Some(source) = mapping.strip_prefix('^') {
            if source.starts_with("refs/") {
                run(path, &["check-ref-format", "--refspec-pattern", source])?;
                mappings.push(mapping.into());
            }
        } else if let Some((source, destination)) = mapping.trim_start_matches('+').split_once(':')
        {
            if source.starts_with("refs/") && destination.starts_with("refs/remotes/") {
                run(path, &["check-ref-format", "--refspec-pattern", source])?;
                run(
                    path,
                    &["check-ref-format", "--refspec-pattern", destination],
                )?;
                mappings.push(mapping.into());
            }
        }
    }
    let mut args = vec![
        "-c",
        "fetch.prune=false",
        "-c",
        "fetch.pruneTags=false",
        "fetch",
        "--no-prune",
        "--no-prune-tags",
        "--no-tags",
        "--no-recurse-submodules",
        "--no-auto-maintenance",
        "--refmap=",
        "--",
        remote,
    ];
    args.extend(mappings.iter().map(String::as_str));
    run_network(path, &args, network)?;
    Ok(())
}
