use crate::{git, model::*};
use std::{
    collections::BTreeSet,
    fs,
    path::{Component, Path},
};

#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Rebase {
    Unset,
    Merge,
    Linear,
    Merges,
    Interactive,
}

#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Policy {
    pub rebase: Rebase,
    pub ff: Option<String>,
}

pub(crate) fn read(path: &Path, branch: &str) -> Result<Policy> {
    // Let Git resolve system/global/local/worktree/includeIf precedence. The
    // last value wins; NUL records distinguish absent from empty booleans.
    let values = git::run(path, &["config", "--null", "--list"])?;
    let config = |key: &str| {
        values
            .split_terminator('\0')
            .filter_map(|entry| {
                let (name, value) = entry.split_once('\n').unwrap_or((entry, ""));
                (name == key).then_some(value.to_owned())
            })
            .next_back()
    };
    let branch_key = format!("branch.{branch}.rebase");
    let (key, value) = if let Some(value) = config(&branch_key) {
        (branch_key.as_str(), Some(value))
    } else {
        ("pull.rebase", config("pull.rebase"))
    };
    let rebase = match value.as_deref() {
        None => Rebase::Unset,
        Some("merges" | "m") => Rebase::Merges,
        Some("interactive" | "i") => Rebase::Interactive,
        Some(_) => {
            let boolean = git::run(path, &["config", "--type=bool", "--get", key])?;
            if boolean.trim() == "true" {
                Rebase::Linear
            } else {
                Rebase::Merge
            }
        }
    };
    let ff = match config("pull.ff") {
        None => None,
        Some(v) if v == "only" => Some("--ff-only".into()),
        Some(_) => {
            let boolean = git::run(path, &["config", "--type=bool", "--get", "pull.ff"])?;
            Some(
                if boolean.trim() == "true" {
                    "--ff"
                } else {
                    "--no-ff"
                }
                .into(),
            )
        }
    };
    Ok(Policy { rebase, ff })
}

// --raw -z --no-renames has alternating metadata/path records. Parse paths
// separately so literal newlines, brackets and leading colons are preserved.
fn changed_paths(output: &str, paths: &mut BTreeSet<String>) -> Result<()> {
    let mut records = output.split_terminator('\0');
    while let Some(header) = records.next() {
        let header = header.trim_start_matches(['\n', '\r']);
        if header.is_empty() {
            continue;
        }
        if !header.starts_with(':') || header.starts_with("::") {
            return Err(GitError::new(
                "git",
                "无法解析同步涉及的路径，已暂停保护检查",
            ));
        }
        let file = records
            .next()
            .filter(|p| !p.is_empty())
            .ok_or_else(|| GitError::new("git", "同步路径记录不完整，已暂停保护检查"))?;
        let relative = Path::new(file);
        if !relative
            .components()
            .all(|c| matches!(c, Component::Normal(_)))
        {
            return Err(GitError::new("path", "同步路径不是有效的仓库相对路径"));
        }
        paths.insert(file.into());
    }
    Ok(())
}

// Rebase may overwrite ignored files. Only inspect paths that can change when
// checking out upstream or replaying local commits, never the whole worktree.
pub(crate) fn protect_ignored(path: &Path, upstream: &str, replay_base: &str) -> Result<()> {
    let mut paths = BTreeSet::new();
    changed_paths(
        &git::run(
            path,
            &[
                "diff",
                "--raw",
                "--no-renames",
                "--no-ext-diff",
                "--no-textconv",
                "-z",
                "HEAD",
                upstream,
                "--",
            ],
        )?,
        &mut paths,
    )?;
    changed_paths(
        &git::run(
            path,
            &[
                "-c",
                "log.showSignature=false",
                "log",
                "--format=",
                "--raw",
                "--diff-merges=separate",
                "--no-renames",
                "--no-ext-diff",
                "--no-textconv",
                "-z",
                &format!("{replay_base}..HEAD"),
                "--",
            ],
        )?,
        &mut paths,
    )?;

    let mut probes = BTreeSet::new();
    for file in paths {
        let mut relative = std::path::PathBuf::new();
        let components: Vec<_> = Path::new(&file).components().collect();
        for (index, component) in components.iter().enumerate() {
            relative.push(component.as_os_str());
            match fs::symlink_metadata(path.join(&relative)) {
                Ok(meta) => {
                    // A file/symlink in a parent position can block a new
                    // directory; don't follow it outside the worktree.
                    #[cfg(windows)]
                    let link_like = {
                        use std::os::windows::fs::MetadataExt;
                        meta.file_attributes() & 0x400 != 0
                    };
                    #[cfg(not(windows))]
                    let link_like = meta.is_symlink();
                    if index + 1 == components.len() || !meta.is_dir() || link_like {
                        probes.insert(
                            relative
                                .iter()
                                .map(|p| p.to_string_lossy())
                                .collect::<Vec<_>>()
                                .join("/"),
                        );
                        break;
                    }
                }
                Err(e) if e.kind() == std::io::ErrorKind::NotFound => break,
                Err(e) => return Err(GitError::new("io", format!("无法检查同步路径 {file}：{e}"))),
            }
        }
    }
    // Bound argv size on Windows. Every query has explicit literal pathspecs;
    // an empty probe list must never turn into an unscoped ls-files invocation.
    let mut batch = Vec::new();
    let mut length = 0;
    for probe in probes {
        let size = probe.encode_utf16().count() + 32;
        if size > 20_000 {
            return Err(GitError::new("path", "同步路径过长，已暂停保护检查"));
        }
        if length + size > 20_000 && !batch.is_empty() {
            check_probes(path, &batch)?;
            batch.clear();
            length = 0;
        }
        batch.push(probe);
        length += size;
    }
    check_probes(path, &batch)
}

fn check_probes(path: &Path, probes: &[String]) -> Result<()> {
    if let Some(file) = git::first_ignored_path(path, probes)? {
        return Err(GitError::new(
            "changed",
            format!(
                "rebase 可能覆盖被忽略文件 {file}，同步已暂停；请先将该文件移到仓库外保存后重试"
            ),
        ));
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn raw_paths_preserve_literal_names_and_reject_incomplete_records() {
        let mut paths = BTreeSet::new();
        changed_paths(
            ":100644 100644 a b M\0\n:[literal].txt\0\n:100644 000000 a b D\0文件 & '$.txt\0",
            &mut paths,
        )
        .unwrap();
        assert!(paths.contains("\n:[literal].txt"));
        assert!(paths.contains("文件 & '$.txt"));
        assert!(changed_paths(":100644 100644 a b M\0", &mut paths).is_err());
        assert!(changed_paths("unexpected\0file\0", &mut paths).is_err());
    }
}
