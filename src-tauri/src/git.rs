use crate::model::*;
use std::{
    collections::HashMap,
    io::Read,
    path::{Path, PathBuf},
    process::{Command, Stdio},
    sync::mpsc,
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
fn read_capped(mut pipe: impl Read) -> (Vec<u8>, bool) {
    let mut all = Vec::new();
    let mut truncated = false;
    let mut buf = [0u8; 8192];
    while let Ok(n) = pipe.read(&mut buf) {
        if n == 0 {
            break;
        }
        let room = (8 * 1024 * 1024usize).saturating_sub(all.len());
        truncated |= n > room;
        all.extend_from_slice(&buf[..n.min(room)]);
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
    check_directory(path)?;
    let mut command = Command::new("git");
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
    let mut child = command.spawn().map_err(|e| spawn_error(path, e))?;
    let out = child.stdout.take().unwrap();
    let err = child.stderr.take().unwrap();
    let (out_tx, out_rx) = mpsc::channel();
    let (err_tx, err_rx) = mpsc::channel();
    thread::spawn(move || {
        let _ = out_tx.send(read_capped(out));
    });
    thread::spawn(move || {
        let _ = err_tx.send(read_capped(err));
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
        if status.is_some() && stdout.is_some() && stderr.is_some() {
            break;
        }
        if start.elapsed() > timeout {
            #[cfg(unix)]
            unsafe {
                libc::kill(-(child.id() as i32), libc::SIGKILL);
            }
            #[cfg(windows)]
            {
                let _ = Command::new("taskkill")
                    .args(["/PID", &child.id().to_string(), "/T", "/F"])
                    .stdout(Stdio::null())
                    .stderr(Stdio::null())
                    .status();
            }
            let _ = child.kill();
            let _ = child.wait();
            return Err(GitError::new(
                "timeout",
                "Git 操作超时，已停止等待并尝试终止本次进程树；请检查网络、SSH、凭据助手或 hooks",
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
        (StatusKind::Attention, "upstream 已失效，请检查远程分支")
    } else if s.ahead > 0 && s.behind > 0 {
        (
            StatusKind::Diverged,
            "本地与远程已分叉，需要合并；尚不代表文件冲突",
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
    if s.changed > 0 && matches!(s.kind, StatusKind::Ahead | StatusKind::Behind) {
        s.message.push_str("；存在未提交改动，同步写操作将暂停");
    }
    Ok(s)
}
pub fn history(repo: &Repo, limit: usize) -> Result<History> {
    let path = Path::new(&repo.path);
    let shallow = run(path, &["rev-parse", "--is-shallow-repository"])?.trim() == "true";
    let has_head = run(path, &["rev-parse", "--verify", "HEAD"]).is_ok();
    let mut args = vec![
        "log",
        "--encoding=UTF-8",
        "--no-show-signature",
        "--no-notes",
        "--no-decorate",
        "--no-patch",
        "--all",
        "--topo-order",
        "-z",
        "--format=%H%x00%P%x00%s%x00%an%x00%aI",
    ];
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

fn tree_diff(
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
    Ok(CommitChanges {
        hash,
        parent,
        files,
    })
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
