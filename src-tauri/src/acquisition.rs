use crate::{git, model::*};
use std::{
    fs,
    path::{Component, Path, PathBuf},
    time::Instant,
};

fn link_like(meta: &fs::Metadata) -> bool {
    #[cfg(windows)]
    {
        use std::os::windows::fs::MetadataExt;
        meta.file_attributes() & 0x400 != 0
    }
    #[cfg(not(windows))]
    {
        meta.is_symlink()
    }
}
pub fn scan(root: &str, depth: usize) -> Result<ScanResult> {
    if depth > 6 {
        return Err(GitError::new("scan", "扫描深度最多为 6"));
    }
    let root = fs::canonicalize(root).map_err(|e| GitError::new("path", e.to_string()))?;
    if !root.is_dir() {
        return Err(GitError::new("path", "请选择目录"));
    }
    let mut result = ScanResult {
        repos: vec![],
        warnings: vec![],
        truncated: false,
    };
    let mut pending = vec![(root, 0)];
    let mut visited = 0;
    let mut discovered = 0;
    let started = Instant::now();
    while let Some((path, level)) = pending.pop() {
        visited += 1;
        if visited > 10_000 || result.repos.len() >= 500 || started.elapsed().as_secs() >= 30 {
            result.truncated = true;
            break;
        }
        if path.join(".git").symlink_metadata().is_ok() {
            match git::discover(&path.to_string_lossy()) {
                Ok(repo) if !result.repos.iter().any(|r| r.path == repo.path) => {
                    result.repos.push(repo)
                }
                Err(e) if result.warnings.len() < 30 => {
                    result
                        .warnings
                        .push(format!("{}：{}", path.display(), e.message))
                }
                _ => {}
            }
            continue;
        }
        if level >= depth {
            continue;
        }
        match fs::read_dir(&path) {
            Ok(entries) => {
                for entry in entries {
                    discovered += 1;
                    if discovered > 10_000 {
                        result.truncated = true;
                        break;
                    }
                    match entry {
                        Ok(entry) => {
                            let name = entry.file_name();
                            if [
                                "node_modules",
                                "target",
                                "dist",
                                "build",
                                "vendor",
                                ".git",
                                ".venv",
                                "venv",
                                ".cache",
                                "Library",
                            ]
                            .contains(&name.to_string_lossy().as_ref())
                            {
                                continue;
                            }
                            if let Ok(meta) = fs::symlink_metadata(entry.path()) {
                                if meta.is_dir() && !link_like(&meta) {
                                    pending.push((entry.path(), level + 1));
                                }
                            }
                        }
                        Err(e) if result.warnings.len() < 30 => {
                            result.warnings.push(git::redact(&e.to_string()))
                        }
                        _ => {}
                    }
                }
            }
            Err(e) if result.warnings.len() < 30 => result
                .warnings
                .push(git::redact(&format!("{}：{e}", path.display()))),
            _ => {}
        }
        if result.truncated {
            break;
        }
    }
    result.repos.sort_by(|a, b| a.path.cmp(&b.path));
    Ok(result)
}

pub fn clone_target(url: &str, parent: &str, name: &str) -> Result<PathBuf> {
    // Accept online Git transports only; credentials belong in the user's helper/SSH agent.
    if url.len() > 4096 || url.chars().any(char::is_whitespace) || url.chars().any(char::is_control)
    {
        return Err(GitError::new("clone", "远程地址含无效字符"));
    }
    let https = url.strip_prefix("https://").is_some_and(|rest| {
        let authority = rest.split('/').next().unwrap_or("");
        !authority.is_empty() && !authority.contains('@') && !rest.contains(['?', '#'])
    });
    let ssh = url.strip_prefix("ssh://").is_some_and(|rest| {
        let authority = rest.split('/').next().unwrap_or("");
        !authority.is_empty()
            && authority.rsplit_once('@').is_none_or(|(user, host)| {
                !user.is_empty() && !user.contains(':') && !host.is_empty()
            })
            && !rest.contains(['?', '#'])
    });
    let scp = !url.contains("://")
        && url.split_once(':').is_some_and(|(host, path)| {
            let host = host.rsplit('@').next().unwrap_or("");
            !host.is_empty()
                && !host.starts_with('-')
                && host
                    .chars()
                    .all(|c| c.is_ascii_alphanumeric() || c == '.' || c == '-')
                && !path.is_empty()
        });
    if !(https || ssh || scp) {
        return Err(GitError::new(
            "clone",
            "请输入 HTTPS、SSH 或 git@host:path 地址；HTTPS 地址不要包含账号、令牌或查询参数",
        ));
    }
    if name.is_empty()
        || name.len() > 120
        || name.ends_with(['.', ' '])
        || name
            .chars()
            .any(|c| c.is_control() || "<>:\"/\\|?*".contains(c))
        || !matches!(
            Path::new(name).components().collect::<Vec<_>>().as_slice(),
            [Component::Normal(_)]
        )
    {
        return Err(GitError::new(
            "path",
            "请输入有效的新文件夹名称，不要包含路径分隔符",
        ));
    }
    // Git for Windows cannot reliably clone into a verbatim (\\?\) destination.
    let parent = dunce::canonicalize(parent).map_err(|e| GitError::new("path", e.to_string()))?;
    if !parent.is_dir() {
        return Err(GitError::new("path", "目标父路径不是目录"));
    }
    let target = parent.join(name);
    if target.symlink_metadata().is_ok() {
        return Err(GitError::new(
            "path",
            "目标已存在，请选择新的文件夹名称；不会覆盖已有目录",
        ));
    }
    Ok(target)
}
pub fn clone_into(
    url: &str,
    target: &Path,
    cancel: &std::sync::atomic::AtomicBool,
) -> Result<Repo> {
    clone_with_network(
        url,
        target,
        cancel,
        &crate::network::NetworkSettings::default(),
    )
}

pub fn clone_with_network(
    url: &str,
    target: &Path,
    cancel: &std::sync::atomic::AtomicBool,
    network: &crate::network::NetworkSettings,
) -> Result<Repo> {
    // Atomic reservation prevents two clone requests from sharing a destination.
    fs::create_dir(target).map_err(|e| GitError::new("path", format!("无法创建目标目录：{e}")))?;
    let parent = target
        .parent()
        .ok_or_else(|| GitError::new("path", "目标目录无效"))?;
    git::clone_online(parent, url, target, cancel, network).map_err(|e| {
        GitError::new(
            &e.kind,
            format!(
                "克隆未完成：{}\n请检查目标目录后重试（不自动删除）：{}",
                e.message,
                target.display()
            ),
        )
    })?;
    git::discover(&target.to_string_lossy())
}
