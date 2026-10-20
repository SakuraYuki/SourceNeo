use crate::{git, model::*};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::{Arc, Condvar, Mutex},
};

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    repos: Vec<Repo>,
    #[serde(default)]
    fetched: HashMap<String, u64>,
}
struct Gate {
    count: Mutex<usize>,
    changed: Condvar,
}
struct Permit<'a>(&'a Gate);
impl Drop for Permit<'_> {
    fn drop(&mut self) {
        *self.0.count.lock().unwrap() -= 1;
        self.0.changed.notify_one();
    }
}
impl Gate {
    fn enter(&self) -> Permit<'_> {
        let mut n = self.count.lock().unwrap();
        while *n >= 3 {
            n = self.changed.wait(n).unwrap()
        }
        *n += 1;
        Permit(self)
    }
}

enum Access {
    ReadOnly,
    Write,
}

pub struct Service {
    config: PathBuf,
    saved: Mutex<Saved>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    gate: Gate,
    logs: Mutex<VecDeque<LogEntry>>,
    errors: Mutex<HashMap<String, GitError>>,
}
impl Service {
    pub fn new(config: PathBuf) -> Result<Self> {
        let saved = if config.exists() {
            serde_json::from_slice(
                &fs::read(&config).map_err(|e| GitError::new("config", e.to_string()))?,
            )
            .map_err(|e| GitError::new("config", format!("仓库配置无法读取，原文件已保留：{e}")))?
        } else {
            Saved::default()
        };
        Ok(Self {
            config,
            saved: Mutex::new(saved),
            locks: Mutex::new(HashMap::new()),
            gate: Gate {
                count: Mutex::new(0),
                changed: Condvar::new(),
            },
            logs: Mutex::new(VecDeque::new()),
            errors: Mutex::new(HashMap::new()),
        })
    }
    fn persist(&self, saved: &Saved) -> Result<()> {
        let parent = self
            .config
            .parent()
            .ok_or_else(|| GitError::new("config", "无效配置路径"))?;
        fs::create_dir_all(parent).map_err(|e| GitError::new("config", e.to_string()))?;
        let mut temp = tempfile::NamedTempFile::new_in(parent)
            .map_err(|e| GitError::new("config", e.to_string()))?;
        use std::io::Write;
        temp.write_all(&serde_json::to_vec_pretty(saved).unwrap())
            .and_then(|_| temp.as_file().sync_all())
            .map_err(|e| GitError::new("config", e.to_string()))?;
        temp.persist(&self.config)
            .map_err(|e| GitError::new("config", e.to_string()))?;
        Ok(())
    }
    pub fn list(&self) -> Vec<Repo> {
        self.saved.lock().unwrap().repos.clone()
    }
    pub fn add(&self, path: &str) -> Result<Repo> {
        let repo = {
            let _permit = self.gate.enter();
            git::discover(path)?
        };
        let mut saved = self.saved.lock().unwrap();
        if let Some(existing) = saved.repos.iter().find(|r| r.path == repo.path) {
            return Ok(existing.clone());
        }
        saved.repos.push(repo.clone());
        if let Err(e) = self.persist(&saved) {
            saved.repos.pop();
            return Err(e);
        }
        drop(saved);
        self.log(&repo.path, "info", "已添加仓库");
        Ok(repo)
    }
    pub fn remove(&self, path: &str) -> Result<()> {
        let mut saved = self.saved.lock().unwrap();
        let old = saved.repos.clone();
        let fetched = saved.fetched.remove(path);
        saved.repos.retain(|r| r.path != path);
        if let Err(e) = self.persist(&saved) {
            saved.repos = old;
            if let Some(t) = fetched {
                saved.fetched.insert(path.into(), t);
            }
            return Err(e);
        }
        self.errors.lock().unwrap().remove(path);
        self.log(path, "info", "已从列表移除，磁盘文件保留");
        Ok(())
    }
    fn with_repo<T>(
        &self,
        path: &str,
        access: Access,
        f: impl FnOnce(&Repo) -> Result<T>,
    ) -> Result<T> {
        let stored = self
            .list()
            .into_iter()
            .find(|r| r.path == path)
            .ok_or_else(|| GitError::new("path", "仓库未添加到列表"))?;
        let repo = {
            let _permit = self.gate.enter();
            git::discover(&stored.path)?
        };
        if repo.path != stored.path || repo.common_dir != stored.common_dir {
            return Err(GitError::new(
                "path",
                "仓库根目录或 Git 元数据位置已变化，已停止操作；请确认仓库后重新添加",
            ));
        }
        let lock = self
            .locks
            .lock()
            .unwrap()
            .entry(repo.common_dir.clone())
            .or_insert_with(|| Arc::new(Mutex::new(())))
            .clone();
        // Wait for a shared-worktree lock before consuming a global worker slot.
        let _guard = lock.lock().unwrap();
        let _permit = self.gate.enter();
        // Reads stay serialized inside this process but never require writing Git metadata.
        // Sync and branch switching mutate Git state and need the cross-process application lock.
        let _file = if matches!(access, Access::Write) {
            let file = OpenOptions::new()
                .create(true)
                .truncate(false)
                .read(true)
                .write(true)
                .open(Path::new(&repo.common_dir).join("sourceneo.lock"))
                .map_err(|e| {
                    GitError::new(
                        if e.kind() == std::io::ErrorKind::PermissionDenied {
                            "permission"
                        } else {
                            "io"
                        },
                        format!("无法打开仓库操作锁：{e}"),
                    )
                })?;
            file.try_lock_exclusive().map_err(|_| {
                GitError::new("busy", "另一个 SourceNeo 实例正在操作此仓库，请稍后重试")
            })?;
            Some(file)
        } else {
            None
        };
        f(&repo)
    }
    fn fetched(&self, repo: &Repo) -> Option<u64> {
        let saved = self.saved.lock().unwrap();
        saved
            .fetched
            .get(&repo.common_dir)
            .or_else(|| saved.fetched.get(&repo.path))
            .copied()
    }
    fn raw_status(&self, repo: &Repo) -> Result<RepoStatus> {
        git::inspect(repo, self.fetched(repo))
    }
    pub fn status(&self, path: &str) -> Result<RepoStatus> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            let mut s = self.raw_status(repo)?;
            if let Some(e) = self.errors.lock().unwrap().get(path) {
                s.error_kind = Some(e.kind.clone());
                s.message = format!("{} · 最近同步失败：{}", s.message, e.message);
                if !matches!(s.kind, StatusKind::Conflict | StatusKind::Diverged) {
                    s.kind = StatusKind::Error;
                }
            }
            Ok(s)
        })
    }
    pub fn history(&self, path: &str, limit: usize) -> Result<History> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            git::history(repo, limit.clamp(1, 5000))
        })
    }
    pub fn branches(&self, path: &str) -> Result<Vec<Branch>> {
        self.with_repo(path, Access::ReadOnly, git::branches)
    }
    pub fn working_changes(&self, path: &str) -> Result<Vec<ChangedFile>> {
        self.with_repo(path, Access::ReadOnly, git::working_changes)
    }
    pub fn commit_changes(&self, path: &str, hash: &str) -> Result<CommitChanges> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            git::commit_changes(repo, hash)
        })
    }
    pub fn file_diff(&self, path: &str, hash: &str, file: &str) -> Result<String> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            git::file_diff(repo, hash, file)
        })
    }
    pub fn switch_branch(&self, path: &str, reference: &str, create: bool) -> Result<RepoStatus> {
        self.with_repo(path, Access::Write, |repo| {
            git::check_locks(repo)?;
            let before = self.raw_status(repo)?;
            if before.changed > 0 || before.operation.is_some() || !before.conflicts.is_empty() {
                return Err(GitError::new("changed", "存在未提交改动、冲突或未完成的 Git 操作，请处理后再切换分支；不会自动 stash 或丢弃改动"));
            }
            let cwd = Path::new(&repo.path);
            let mut args = vec!["switch", "--no-guess", "--no-recurse-submodules", "--no-overwrite-ignore"];
            let local;
            if create {
                if reference.is_empty() || reference.starts_with('-') || reference == "HEAD" {
                    return Err(GitError::new("branch", "请输入有效的新分支名称"));
                }
                git::run(cwd, &["check-ref-format", &format!("refs/heads/{reference}")])?;
                args.extend(["-c", reference, "--", &before.head]);
            } else {
                let target = git::branches(repo)?.into_iter().find(|b| b.reference == reference)
                    .ok_or_else(|| GitError::new("branch", "分支已不存在，请重新加载分支列表"))?;
                if target.current { return Ok(before); }
                if target.remote {
                    let remotes = git::run(cwd, &["remote"])?;
                    let name = reference.strip_prefix("refs/remotes/").unwrap();
                    let remote = remotes.lines().filter(|r| name.starts_with(&format!("{r}/"))).max_by_key(|r| r.len())
                        .ok_or_else(|| GitError::new("branch", "找不到此远程分支对应的 remote"))?;
                    local = name[remote.len() + 1..].to_string();
                    args.extend(["--track", "-c", &local, "--", reference]);
                } else {
                    local = target.name;
                    args.extend(["--", &local]);
                }
            }
            self.assert_unchanged(repo, &before)?;
            git::run(cwd, &args)?;
            self.errors.lock().unwrap().remove(path);
            let after = self.raw_status(repo)?;
            self.log(path, "success", format!("已切换分支 · {}", after.branch));
            Ok(after)
        })
    }
    fn log(&self, path: &str, level: &str, message: impl AsRef<str>) {
        let mut logs = self.logs.lock().unwrap();
        logs.push_front(LogEntry {
            time: now(),
            path: git::redact(path),
            level: level.into(),
            message: git::redact(message.as_ref()),
        });
        logs.truncate(500);
    }
    pub fn logs(&self) -> Vec<LogEntry> {
        self.logs.lock().unwrap().iter().cloned().collect()
    }
    fn assert_unchanged(&self, repo: &Repo, expected: &RepoStatus) -> Result<RepoStatus> {
        git::check_locks(repo)?;
        let actual = self.raw_status(repo)?;
        if actual.head != expected.head
            || actual.branch != expected.branch
            || actual.upstream != expected.upstream
            || actual.changed > 0
            || actual.operation.is_some()
            || !actual.conflicts.is_empty()
        {
            return Err(GitError::new(
                "changed",
                "仓库被外部操作修改或有未提交改动，本次同步已暂停；请刷新后重试",
            ));
        }
        Ok(actual)
    }
    fn sync_inner(&self, repo: &Repo) -> Result<RepoStatus> {
        git::check_locks(repo)?;
        let before = self.raw_status(repo)?;
        if !before.conflicts.is_empty() || before.operation.is_some() {
            self.log(&repo.path, "warning", &before.message);
            return Ok(before);
        }
        if before.head == "(initial)" || before.branch == "(detached)" || before.upstream.is_none()
        {
            self.log(&repo.path, "warning", &before.message);
            return Ok(before);
        }
        let path = Path::new(&repo.path);
        let remote = git::run(
            path,
            &[
                "config",
                "--get",
                &format!("branch.{}.remote", before.branch),
            ],
        )?;
        let remote = remote.trim();
        // Git resolves the first merge value as upstream; config --get returns the last.
        // Refuse ambiguous configuration before fetch or any branch write.
        let merge_config = git::run(
            path,
            &[
                "config",
                "--null",
                "--get-all",
                &format!("branch.{}.merge", before.branch),
            ],
        )?;
        let mut merge_values = merge_config.split_terminator('\0');
        let merge = merge_values.next().unwrap_or("");
        if merge_values.next().is_some() {
            return Err(GitError::new(
                "configuration",
                "upstream 合并目标存在多个配置值，同步已暂停；请交给 AI 检查配置后重试",
            ));
        }
        if remote == "." || !merge.starts_with("refs/heads/") {
            return Err(GitError::new(
                "configuration",
                "当前 upstream 不是远程分支，请交给 AI 检查配置",
            ));
        }
        self.log(&repo.path, "info", format!("获取远程更新 · {remote}"));
        git::run(
            path,
            &[
                "fetch",
                "--no-recurse-submodules",
                "--no-auto-maintenance",
                "--",
                remote,
            ],
        )?;
        {
            let mut saved = self.saved.lock().unwrap();
            saved.fetched.insert(repo.common_dir.clone(), now());
            self.persist(&saved)?;
        }
        let mut s = self.raw_status(repo)?;
        if before.branch != s.branch || before.head != s.head || before.upstream != s.upstream {
            return Err(GitError::new(
                "changed",
                "fetch 期间仓库被外部操作修改，已暂停同步",
            ));
        }
        if s.kind == StatusKind::Diverged
            || s.operation.is_some()
            || !s.conflicts.is_empty()
            || s.kind == StatusKind::Attention
        {
            self.log(&repo.path, "warning", &s.message);
            return Ok(s);
        }
        if s.changed > 0 {
            s.message = "已获取远程状态；存在未提交改动，请交给 AI 处理后再同步".into();
            s.kind = StatusKind::Attention;
            self.log(&repo.path, "warning", &s.message);
            return Ok(s);
        }
        self.assert_unchanged(repo, &s)?;
        let upstream_oid = git::run(path, &["rev-parse", "--verify", "@{upstream}"])?;
        let upstream_oid = upstream_oid.trim();
        if s.behind > 0 {
            self.log(
                &repo.path,
                "info",
                format!("快进拉取 · {} 个提交", s.behind),
            );
            // Pin the fetched commit. Never create a merge commit or run a rebase.
            git::run(
                path,
                &[
                    "-c",
                    "merge.autoStash=false",
                    "merge",
                    "--ff-only",
                    "--no-autostash",
                    "--no-squash",
                    "--no-edit",
                    "--no-overwrite-ignore",
                    upstream_oid,
                ],
            )?;
            let after = self.raw_status(repo)?;
            if after.branch != s.branch
                || after.head != upstream_oid
                || after.upstream != s.upstream
            {
                return Err(GitError::new(
                    "changed",
                    "快进后检测到外部修改，请刷新后重试",
                ));
            }
            s = after;
        }
        self.assert_unchanged(repo, &s)?;
        if s.ahead > 0 {
            // Push the inspected commit, to the tracked branch, with ordinary fast-forward checks.
            // Ignore push.default, push refspecs, and followTags so unrelated branches are never pushed.
            self.log(&repo.path, "info", format!("正常推送 · {} 个提交", s.ahead));
            git::run(
                path,
                &[
                    "-c",
                    "push.followTags=false",
                    "push",
                    "--recurse-submodules=no",
                    "--",
                    remote,
                    &format!("{}:{merge}", s.head),
                ],
            )?;
        }
        let s = self.raw_status(repo)?;
        self.log(&repo.path, "success", &s.message);
        Ok(s)
    }
    pub fn sync(&self, path: &str) -> Result<RepoStatus> {
        self.log(path, "info", "开始同步");
        self.errors.lock().unwrap().remove(path);
        let result = self.with_repo(path, Access::Write, |repo| self.sync_inner(repo));
        if let Err(e) = &result {
            self.errors.lock().unwrap().insert(path.into(), e.clone());
            self.log(path, "error", &e.message);
        }
        result
    }
    pub fn diagnostic(&self, path: &str) -> Result<String> {
        let s = self.status(path)?;
        let errors = self
            .logs()
            .into_iter()
            .filter(|l| l.path == git::redact(path) && l.level == "error")
            .take(5)
            .map(|l| format!("- {}", l.message))
            .collect::<Vec<_>>()
            .join("\n");
        Ok(git::redact(&format!("SourceNeo 诊断\n仓库：{}\n分支：{}\nHEAD：{}\nUpstream：{}\n状态：{:?}\n说明：{}\n待推送：{}，待拉取：{}，未提交改动：{}\n进行中操作：{}\n冲突文件：\n{}\n远程状态更新时间（Unix 秒）：{}\n最近错误：\n{}\n\n请先检查状态并解释处理方案；SourceNeo 不会自动合并、rebase 或丢弃修改。",s.path,s.branch,s.head,s.upstream.as_deref().unwrap_or("未配置"),s.kind,s.message,s.ahead,s.behind,s.changed,s.operation.as_deref().unwrap_or("无"),if s.conflicts.is_empty(){"无".into()}else{s.conflicts.iter().map(|p|format!("- {p}")).collect::<Vec<_>>().join("\n")},s.last_fetched.map(|v|v.to_string()).unwrap_or_else(||"从未获取".into()),if errors.is_empty(){"无"}else{&errors})))
    }
}
