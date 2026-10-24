use crate::{
    git,
    model::*,
    sync_policy::{self, Rebase},
};
use fs2::FileExt;
use serde::{Deserialize, Serialize};
use std::{
    collections::{HashMap, VecDeque},
    fs::{self, OpenOptions},
    path::{Path, PathBuf},
    sync::{
        atomic::{AtomicBool, AtomicU64, Ordering},
        Arc, Condvar, Mutex,
    },
    time::Instant,
};

#[derive(Default, Serialize, Deserialize)]
struct Saved {
    repos: Vec<Repo>,
    #[serde(default)]
    groups: Vec<RepoGroup>,
    #[serde(default)]
    fetched: HashMap<String, u64>,
    #[serde(default)]
    tools: crate::tools::ToolSettings,
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
    fn enter_cancelable(&self, cancel: &AtomicBool) -> Result<Permit<'_>> {
        let mut n = self.count.lock().unwrap();
        while *n >= 3 && !cancel.load(Ordering::Relaxed) {
            n = self
                .changed
                .wait_timeout(n, std::time::Duration::from_millis(100))
                .unwrap()
                .0;
        }
        if cancel.load(Ordering::Relaxed) {
            return Err(GitError::new("cancelled", "已取消排队中的克隆"));
        }
        *n += 1;
        Ok(Permit(self))
    }
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

struct ActiveOperation {
    operation: String,
    stage: String,
    started: Instant,
    started_at: u64,
}

pub struct Service {
    config: PathBuf,
    network: Mutex<crate::network::NetworkSettings>,
    saved: Mutex<Saved>,
    locks: Mutex<HashMap<String, Arc<Mutex<()>>>>,
    gate: Gate,
    journal: crate::journal::Journal,
    snapshots: Mutex<VecDeque<crate::history::Snapshot>>,
    snapshot_id: AtomicU64,
    operations: Mutex<HashMap<String, ActiveOperation>>,
    clones: Mutex<HashMap<String, Arc<AtomicBool>>>,
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
        let journal = crate::journal::Journal::new(config.with_file_name("operations.json"));
        Ok(Self {
            network: Mutex::new(crate::network::NetworkSettings::load(
                &config.with_file_name("network.json"),
            )?),
            config,
            saved: Mutex::new(saved),
            locks: Mutex::new(HashMap::new()),
            gate: Gate {
                count: Mutex::new(0),
                changed: Condvar::new(),
            },
            journal,
            snapshots: Mutex::new(VecDeque::new()),
            snapshot_id: AtomicU64::new(1),
            operations: Mutex::new(HashMap::new()),
            clones: Mutex::new(HashMap::new()),
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
    pub fn groups(&self) -> Vec<RepoGroup> {
        self.saved.lock().unwrap().groups.clone()
    }
    pub fn update_repo_flags(&self, path: &str, patch: RepoFlagsPatch) -> Result<Repo> {
        // Serialize the mark with this repository's operations: once saving succeeds,
        // even a sync that was queued earlier must observe the new prohibition.
        self.with_repo(path, Access::ReadOnly, |_| {
            let mut saved = self.saved.lock().unwrap();
            let index = saved
                .repos
                .iter()
                .position(|r| r.path == path)
                .ok_or_else(|| GitError::new("path", "仓库已从列表移除"))?;
            let old = saved.repos[index].clone();
            if let Some(pinned) = patch.pinned {
                saved.repos[index].pinned = pinned;
            }
            if let Some(disabled) = patch.sync_disabled {
                saved.repos[index].sync_disabled = disabled;
            }
            if let Err(error) = self.persist(&saved) {
                saved.repos[index] = old;
                return Err(error);
            }
            Ok(saved.repos[index].clone())
        })
    }
    fn ensure_sync_allowed(&self, path: &str) -> Result<()> {
        let saved = self.saved.lock().unwrap();
        let repo = saved
            .repos
            .iter()
            .find(|r| r.path == path)
            .ok_or_else(|| GitError::new("path", "仓库已从列表移除"))?;
        if repo.sync_disabled {
            return Err(GitError::new(
                "syncDisabled",
                "此仓库已禁止同步，仅允许抓取；如需同步，请先在仓库菜单中解除禁止",
            ));
        }
        Ok(())
    }
    pub fn edit_groups(&self, edit: GroupEdit) -> Result<Vec<RepoGroup>> {
        let mut saved = self.saved.lock().unwrap();
        let mut groups = saved.groups.clone();
        let valid_name = |name: &str| -> Result<String> {
            let name = name.trim();
            if name.is_empty() || name.chars().count() > 80 || name.chars().any(char::is_control) {
                return Err(GitError::new(
                    "group",
                    "分组名称须为 1–80 个字符，不能包含控制字符",
                ));
            }
            Ok(name.to_owned())
        };
        match edit {
            GroupEdit::Create { name } => {
                let name = valid_name(&name)?;
                if groups.iter().any(|g| g.name == name) {
                    return Err(GitError::new("group", "分组名称已存在"));
                }
                groups.push(RepoGroup {
                    name,
                    paths: Vec::new(),
                });
            }
            GroupEdit::Rename { group, name } => {
                let name = valid_name(&name)?;
                if groups.iter().any(|g| g.name == name && g.name != group) {
                    return Err(GitError::new("group", "分组名称已存在"));
                }
                groups
                    .iter_mut()
                    .find(|g| g.name == group)
                    .ok_or_else(|| GitError::new("group", "分组不存在"))?
                    .name = name;
            }
            GroupEdit::Remove { group } => {
                if !groups.iter().any(|g| g.name == group) {
                    return Err(GitError::new("group", "分组不存在"));
                }
                groups.retain(|g| g.name != group);
            }
            GroupEdit::Move { paths, group } => {
                if group
                    .as_ref()
                    .is_some_and(|name| !groups.iter().any(|g| &g.name == name))
                {
                    return Err(GitError::new("group", "分组不存在"));
                }
                if paths
                    .iter()
                    .any(|p| !saved.repos.iter().any(|r| &r.path == p))
                {
                    return Err(GitError::new("path", "部分仓库已从列表移除，请刷新后重试"));
                }
                for g in &mut groups {
                    g.paths.retain(|p| !paths.contains(p));
                    if Some(&g.name) == group.as_ref() {
                        for path in &paths {
                            if !g.paths.contains(path) {
                                g.paths.push(path.clone());
                            }
                        }
                    }
                }
            }
        }
        let old = std::mem::replace(&mut saved.groups, groups);
        if let Err(e) = self.persist(&saved) {
            saved.groups = old;
            return Err(e);
        }
        Ok(saved.groups.clone())
    }
    pub fn tool_settings(&self) -> crate::tools::ToolSettings {
        self.saved.lock().unwrap().tools.clone()
    }
    pub fn network_settings(&self) -> crate::network::NetworkSettings {
        self.network.lock().unwrap().clone()
    }
    pub fn save_network_settings(&self, settings: crate::network::NetworkSettings) -> Result<()> {
        let settings = settings.validated()?;
        let mut current = self.network.lock().unwrap();
        settings.save(&self.config.with_file_name("network.json"))?;
        *current = settings;
        Ok(())
    }
    pub fn save_tool_settings(&self, settings: crate::tools::ToolSettings) -> Result<()> {
        let mut saved = self.saved.lock().unwrap();
        let old = std::mem::replace(&mut saved.tools, settings);
        if let Err(e) = self.persist(&saved) {
            saved.tools = old;
            return Err(e);
        }
        Ok(())
    }
    pub fn open_tool(&self, path: &str, kind: &str) -> Result<()> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            let spec = crate::tools::plan(&repo.path, kind, &self.tool_settings(), cfg!(windows))?;
            crate::tools::launch(&spec)
        })
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
        let old_groups = saved.groups.clone();
        let fetched = saved.fetched.remove(path);
        saved.repos.retain(|r| r.path != path);
        for group in &mut saved.groups {
            group.paths.retain(|p| p != path);
        }
        if let Err(e) = self.persist(&saved) {
            saved.repos = old;
            saved.groups = old_groups;
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
    pub fn history(&self, path: &str, limit: usize, all_branches: bool) -> Result<History> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            git::history(repo, limit.clamp(1, 5000), all_branches)
        })
    }
    pub fn branches(&self, path: &str) -> Result<Vec<Branch>> {
        self.with_repo(path, Access::ReadOnly, git::branches)
    }
    pub fn working_changes(&self, path: &str) -> Result<Vec<ChangedFile>> {
        self.with_repo(path, Access::ReadOnly, git::working_changes)
    }
    pub fn working_diff(&self, path: &str, file: &str, side: &str) -> Result<String> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            git::working_diff(repo, file, side)
        })
    }
    pub fn fetch_remotes(&self, path: &str) -> Result<Vec<String>> {
        self.with_repo(path, Access::ReadOnly, git::fetch_remotes)
    }
    pub fn fetch(&self, path: &str, remote: Option<&str>) -> Result<RepoStatus> {
        let result = self.with_repo(path, Access::Write, |repo| {
            self.tracked(path, "抓取", || {
                git::check_locks(repo)?;
                self.stage(path, "fetching");
                git::fetch_with_network(repo, remote, &self.network_settings())?;
                self.stage(path, "checking");
                let mut saved = self.saved.lock().unwrap();
                let old = saved.fetched.insert(repo.common_dir.clone(), now());
                if let Err(e) = self.persist(&saved) {
                    if let Some(time) = old {
                        saved.fetched.insert(repo.common_dir.clone(), time);
                    } else {
                        saved.fetched.remove(&repo.common_dir);
                    }
                    return Err(e);
                }
                drop(saved);
                self.errors.lock().unwrap().remove(path);
                self.raw_status(repo)
            })
        });
        match &result {
            Ok(_) => self.log(path, "success", "已仅获取远端；本地分支和文件保持原状"),
            Err(e) => self.log(path, "error", format!("获取失败：{}", e.message)),
        }
        result
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
        let progress = self.progress().into_iter().find(|p| p.path == path);
        self.journal.push(LogEntry {
            time: now(),
            path: git::redact(path),
            level: level.into(),
            message: GitError::new("log", message).message,
            operation: progress.as_ref().map(|p| p.operation.clone()),
            stage: progress.as_ref().map(|p| p.stage.clone()),
            elapsed_ms: progress.map(|p| p.elapsed_ms),
        });
    }
    pub fn logs(&self) -> Vec<LogEntry> {
        self.journal.entries()
    }
    pub fn progress(&self) -> Vec<OperationProgress> {
        self.operations
            .lock()
            .unwrap()
            .iter()
            .map(|(path, p)| OperationProgress {
                path: path.clone(),
                operation: p.operation.clone(),
                stage: p.stage.clone(),
                started_at: p.started_at,
                elapsed_ms: p.started.elapsed().as_millis() as u64,
            })
            .collect()
    }
    fn stage(&self, path: &str, stage: &str) {
        if let Some(p) = self.operations.lock().unwrap().get_mut(path) {
            p.stage = stage.into();
        }
    }
    fn tracked<T>(
        &self,
        path: &str,
        operation: &str,
        work: impl FnOnce() -> Result<T>,
    ) -> Result<T> {
        self.operations.lock().unwrap().insert(
            path.into(),
            ActiveOperation {
                operation: operation.into(),
                stage: "checking".into(),
                started: Instant::now(),
                started_at: now(),
            },
        );
        self.log(path, "info", format!("开始{operation}"));
        let result = work();
        self.log(
            path,
            if result.is_ok() { "info" } else { "error" },
            match &result {
                Ok(_) => format!("{operation}已结束，请查看结果状态"),
                Err(e) => e.message.clone(),
            },
        );
        self.operations.lock().unwrap().remove(path);
        result
    }
    pub fn history_page(
        &self,
        path: &str,
        all: bool,
        snapshot: Option<&str>,
        offset: usize,
        query: HistoryQuery,
    ) -> Result<HistoryPage> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            let state = if let Some(id) = snapshot {
                self.snapshots
                    .lock()
                    .unwrap()
                    .iter()
                    .find(|s| s.id == id && s.path == path && s.created.elapsed().as_secs() < 600)
                    .cloned()
                    .ok_or_else(|| GitError::new("snapshot", "历史快照已过期，请刷新历史后重试"))?
            } else {
                if offset != 0 {
                    return Err(GitError::new("snapshot", "请从第一页开始读取"));
                }
                let id = self.snapshot_id.fetch_add(1, Ordering::Relaxed).to_string();
                let state = crate::history::Snapshot::new(repo, id, all, query)?;
                let mut cache = self.snapshots.lock().unwrap();
                cache.push_front(state.clone());
                cache.truncate(16);
                state
            };
            let page = state.page(offset, 100)?;
            Ok(HistoryPage {
                snapshot: state.id,
                commits: page.commits,
                has_more: page.has_more,
                shallow: page.shallow,
            })
        })
    }
    pub fn compare(&self, path: &str) -> Result<Comparison> {
        self.with_repo(path, Access::ReadOnly, crate::history::compare)
    }
    pub fn compare_diff(&self, path: &str, head: &str, target: &str, file: &str) -> Result<String> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            for oid in [head, target] {
                if !matches!(oid.len(), 40 | 64) || !oid.bytes().all(|b| b.is_ascii_hexdigit()) {
                    return Err(GitError::new("revision", "无效提交 SHA"));
                }
            }
            let files = git::parse_changes(&git::tree_diff(
                repo,
                head,
                Some(target),
                &["--name-status", "-z"],
                &[],
            )?)?;
            let changed = files
                .iter()
                .find(|f| f.path == file)
                .ok_or_else(|| GitError::new("path", "文件不在比较结果中"))?;
            let mut paths = vec![changed.path.as_str()];
            if let Some(old) = &changed.old_path {
                paths.push(old);
            }
            git::tree_diff(
                repo,
                head,
                Some(target),
                &["--patch", "--src-prefix=a/", "--dst-prefix=b/"],
                &paths,
            )
        })
    }
    pub fn scan(&self, root: &str, depth: usize) -> Result<ScanResult> {
        let _permit = self.gate.enter();
        crate::acquisition::scan(root, depth)
    }
    pub fn clone_repo(&self, url: &str, parent: &str, name: &str) -> Result<Repo> {
        let id = format!(
            "internal-{}",
            self.snapshot_id.fetch_add(1, Ordering::Relaxed)
        );
        self.clone_with_id(&id, url, parent, name)
    }
    pub fn cancel_clone(&self, id: &str) -> Result<()> {
        let clones = self.clones.lock().unwrap();
        let cancel = clones
            .get(id)
            .ok_or_else(|| GitError::new("clone", "克隆已结束或尚未开始，请查看当前结果"))?;
        cancel.store(true, Ordering::Relaxed);
        Ok(())
    }
    pub fn clone_with_id(&self, id: &str, url: &str, parent: &str, name: &str) -> Result<Repo> {
        if id.is_empty() || id.len() > 100 {
            return Err(GitError::new("clone", "无效克隆任务标识"));
        }
        let cancel = Arc::new(AtomicBool::new(false));
        {
            let mut clones = self.clones.lock().unwrap();
            if clones.contains_key(id) {
                return Err(GitError::new("busy", "克隆任务已存在"));
            }
            clones.insert(id.into(), cancel.clone());
        }
        let result = (|| {
            let target = crate::acquisition::clone_target(url, parent, name)?;
            let path = target.to_string_lossy().into_owned();
            let repo = {
                let _permit = self.gate.enter_cancelable(&cancel)?;
                if cancel.load(Ordering::Relaxed) {
                    return Err(GitError::new("cancelled", "已取消排队中的克隆"));
                }
                self.tracked(&path, "克隆", || {
                    self.stage(&path, "cloning");
                    crate::acquisition::clone_with_network(
                        url,
                        &target,
                        &cancel,
                        &self.network_settings(),
                    )
                })?
            };
            self.add(&repo.path).map_err(|e| {
                GitError::new(
                    &e.kind,
                    format!(
                        "克隆已成功，但加入列表失败：{}。仓库保留在 {}，可手动添加",
                        e.message, repo.path
                    ),
                )
            })
        })();
        self.clones.lock().unwrap().remove(id);
        result
    }
    pub fn preview(&self, path: &str) -> Result<SyncPreview> {
        self.with_repo(path, Access::ReadOnly, |repo| {
            let status = self.raw_status(repo)?;
            let mut preview = SyncPreview {
                status,
                remote: None,
                policy: String::new(),
                steps: vec!["获取远端并重新校验工作区、分支及同步策略".into()],
                blockers: vec![],
            };
            if let Err(error) = self.ensure_sync_allowed(path) {
                preview.blockers.push(error.message);
            }
            let s = &preview.status;
            if let Err(e) = git::check_locks(repo) {
                preview.blockers.push(e.message);
            }
            if s.changed > 0 {
                preview
                    .blockers
                    .push("存在未提交改动，整合和推送将暂停".into());
            }
            if s.operation.is_some()
                || !s.conflicts.is_empty()
                || s.upstream.is_none()
                || s.head == "(initial)"
                || s.branch == "(detached)"
                || s.reason.as_deref() == Some("upstreamMissing")
            {
                preview.blockers.push(s.message.clone());
            }
            if s.upstream.is_some() && s.branch != "(detached)" {
                let p = Path::new(path);
                match git::run(
                    p,
                    &["config", "--get", &format!("branch.{}.remote", s.branch)],
                ) {
                    Ok(remote) => {
                        let remote = remote.trim().to_owned();
                        if remote == "." {
                            preview.blockers.push("上游不是远程分支".into());
                        }
                        preview.remote = Some(git::redact(&remote));
                    }
                    Err(e) => preview.blockers.push(e.message),
                }
                match git::run(
                    p,
                    &[
                        "config",
                        "--null",
                        "--get-all",
                        &format!("branch.{}.merge", s.branch),
                    ],
                ) {
                    Ok(merges) => {
                        let values: Vec<_> = merges.split_terminator('\0').collect();
                        if values.len() != 1 || !values[0].starts_with("refs/heads/") {
                            preview.blockers.push("上游合并目标无效或不唯一".into());
                        }
                    }
                    Err(e) => preview.blockers.push(e.message),
                }
                match sync_policy::read(p, &s.branch) {
                    Ok(policy) => {
                        let ff_only = policy.ff.as_deref() == Some("--ff-only")
                            || (policy.rebase == Rebase::Unset && policy.ff.is_none());
                        preview.policy = format!(
                            "{}；{}",
                            match policy.rebase {
                                Rebase::Unset => "未配置 rebase",
                                Rebase::Merge => "merge",
                                Rebase::Linear => "rebase（继承 rebase.rebaseMerges）",
                                Rebase::Merges => "rebase（保留合并）",
                                Rebase::Interactive => "交互式 rebase",
                            },
                            policy
                                .ff
                                .as_deref()
                                .unwrap_or("继承 Git merge.ff；无同步策略时仅允许快进")
                        );
                        if s.behind > 0 {
                            if s.ahead > 0 && ff_only {
                                preview.blockers.push("当前分叉，策略只允许快进".into());
                            } else if s.ahead > 0 && policy.rebase == Rebase::Interactive {
                                preview.blockers.push("需要在终端完成交互式 rebase".into());
                            } else {
                                preview.steps.push(
                                    if s.ahead > 0
                                        && matches!(policy.rebase, Rebase::Linear | Rebase::Merges)
                                        && !ff_only
                                    {
                                        "按 Git 配置重放本地提交（rebase）"
                                    } else if ff_only
                                        || (s.ahead == 0
                                            && matches!(
                                                policy.rebase,
                                                Rebase::Linear
                                                    | Rebase::Merges
                                                    | Rebase::Interactive
                                            ))
                                    {
                                        "快进到已获取的上游提交"
                                    } else {
                                        "按 merge / ff 配置整合，可能创建合并提交"
                                    }
                                    .into(),
                                );
                            }
                        }
                        if s.ahead > 0
                            || (s.behind > 0
                                && !ff_only
                                && !matches!(
                                    policy.rebase,
                                    Rebase::Linear | Rebase::Merges | Rebase::Interactive
                                ))
                        {
                            preview.steps.push("必要时普通推送当前分支到上游".into());
                        } else {
                            preview
                                .steps
                                .push("若远端引用和策略保持不变，可能无需推送".into());
                        }
                    }
                    Err(e) => preview.blockers.push(e.message),
                }
            }
            Ok(preview)
        })
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
        let network = self.network_settings();
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
        let policy = sync_policy::read(path, &before.branch)?;
        // Resolve the old upstream's fork point before fetch changes its reflog,
        // as pull --rebase does. This avoids replaying withdrawn upstream commits.
        let fork_point = git::run(
            path,
            &["merge-base", "--fork-point", "@{upstream}", &before.head],
        )
        .ok()
        .map(|oid| oid.trim().to_owned());
        self.stage(&repo.path, "fetching");
        self.log(&repo.path, "info", format!("获取远程更新 · {remote}"));
        git::fetch_with_network(repo, Some(remote), &network)?;
        {
            let mut saved = self.saved.lock().unwrap();
            saved.fetched.insert(repo.common_dir.clone(), now());
            self.persist(&saved)?;
        }
        self.stage(&repo.path, "checking");
        let mut s = self.raw_status(repo)?;
        if before.branch != s.branch || before.head != s.head || before.upstream != s.upstream {
            return Err(GitError::new(
                "changed",
                "fetch 期间仓库被外部操作修改，已暂停同步",
            ));
        }
        if s.operation.is_some() || !s.conflicts.is_empty() || s.kind == StatusKind::Attention {
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
        let rebase_requested = matches!(
            policy.rebase,
            Rebase::Linear | Rebase::Merges | Rebase::Interactive
        );
        // Pull uses the old fork point only when it was removed by an upstream
        // rewrite. Otherwise exclude the entire fetched upstream, including any
        // local commits a colleague has already adopted. A pure remote rewind
        // can need rebase even though its ahead/behind count has behind == 0.
        let replay_base = match fork_point.as_deref() {
            Some(point)
                if rebase_requested
                    && git::run(path, &["merge-base", "--is-ancestor", point, upstream_oid])
                        .is_err() =>
            {
                point
            }
            _ => upstream_oid,
        };
        let rewritten = replay_base != upstream_oid && policy.ff.as_deref() != Some("--ff-only");
        if s.behind > 0 || rewritten {
            let ff_only = policy.ff.as_deref() == Some("--ff-only")
                || (policy.rebase == Rebase::Unset && policy.ff.is_none());
            if ff_only && s.ahead > 0 {
                s.message = if policy.ff.is_some() {
                    "本地与远程已分叉；pull.ff=only 仅允许快进，同步已暂停"
                } else {
                    "本地与远程已分叉；尚未配置同步策略，请设置 pull.rebase 或 pull.ff 后重试"
                }
                .into();
                self.log(&repo.path, "warning", &s.message);
                return Ok(s);
            }
            if policy.rebase == Rebase::Interactive && !ff_only && s.ahead > 0 {
                s.message =
                    "配置要求 interactive rebase，请在终端完成交互式 rebase 后再同步".into();
                s.kind = StatusKind::Attention;
                self.log(&repo.path, "warning", &s.message);
                return Ok(s);
            }
            let rebase =
                matches!(policy.rebase, Rebase::Linear | Rebase::Merges) && !ff_only && s.ahead > 0;
            // Pin the fetched commit; do not fetch again through pull, autostash,
            // or rewrite other local branches through rebase.updateRefs.
            let mut expected_merge_head = None;
            self.stage(&repo.path, "integrating");
            let result = if rebase {
                sync_policy::protect_ignored(path, upstream_oid, replay_base)?;
                self.assert_unchanged(repo, &s)?;
                self.log(&repo.path, "info", "按 Git 配置执行 rebase");
                let mut args = vec![
                    "-c",
                    "core.editor=false",
                    "-c",
                    "sequence.editor=false",
                    "rebase",
                    "--no-autostash",
                    "--no-update-refs",
                    "--no-fork-point",
                ];
                // With pull.rebase=true, leave rebase.rebaseMerges in control.
                // pull.rebase=merges explicitly overrides it, just as Git does.
                if policy.rebase == Rebase::Merges {
                    args.push("--rebase-merges");
                }
                args.extend(["--onto", upstream_oid, replay_base]);
                git::run(path, &args)
            } else {
                self.log(&repo.path, "info", "按 Git 配置整合远端提交");
                let ff = if ff_only
                    || matches!(
                        policy.rebase,
                        Rebase::Linear | Rebase::Merges | Rebase::Interactive
                    ) {
                    Some("--ff-only")
                } else {
                    policy.ff.as_deref()
                };
                let mut args = vec![
                    "-c",
                    "merge.autoStash=false",
                    "merge",
                    "--no-autostash",
                    "--no-squash",
                    "--no-edit",
                    "--no-overwrite-ignore",
                ];
                if let Some(ff) = ff {
                    args.push(ff);
                    if ff == "--ff-only" || (ff == "--ff" && s.ahead == 0) {
                        expected_merge_head = Some(upstream_oid);
                    }
                }
                args.push(upstream_oid);
                git::run(path, &args)
            };
            if let Err(error) = result {
                let paused = self.raw_status(repo)?;
                if paused.operation.is_some() || !paused.conflicts.is_empty() {
                    self.log(
                        &repo.path,
                        "warning",
                        format!("{} · {}", paused.message, error.message),
                    );
                    return Ok(paused);
                }
                return Err(error);
            }
            let after = self.raw_status(repo)?;
            if after.branch != s.branch
                || expected_merge_head.is_some_and(|head| after.head != head)
                || after.upstream != s.upstream
                || after.behind > 0
                || after.operation.is_some()
                || !after.conflicts.is_empty()
                || git::run(
                    path,
                    &["merge-base", "--is-ancestor", upstream_oid, &after.head],
                )
                .is_err()
            {
                return Err(GitError::new(
                    "changed",
                    "整合后检测到外部修改或整合尚未完成，同步已暂停；请刷新后检查",
                ));
            }
            s = after;
        }
        self.assert_unchanged(repo, &s)?;
        if s.ahead > 0 {
            // Push the inspected commit, to the tracked branch, with ordinary fast-forward checks.
            // Ignore push.default, push refspecs, and followTags so unrelated branches are never pushed.
            self.stage(&repo.path, "pushing");
            self.log(&repo.path, "info", format!("正常推送 · {} 个提交", s.ahead));
            git::run_network(
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
                &network,
            )?;
        }
        let s = self.raw_status(repo)?;
        self.log(&repo.path, "success", &s.message);
        Ok(s)
    }
    pub fn sync(&self, path: &str) -> Result<RepoStatus> {
        self.log(path, "info", "开始同步");
        self.errors.lock().unwrap().remove(path);
        let result = self.with_repo(path, Access::Write, |repo| {
            self.ensure_sync_allowed(path)?;
            self.tracked(path, "同步", || self.sync_inner(repo))
        });
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
        Ok(git::redact(&format!("SourceNeo 诊断\n仓库：{}\n分支：{}\nHEAD：{}\nUpstream：{}\n状态：{:?}\n说明：{}\n待推送：{}，待拉取：{}，未提交改动：{}\n进行中操作：{}\n冲突文件：\n{}\n远程状态更新时间（Unix 秒）：{}\n最近错误：\n{}\n\n请先检查状态并解释处理方案；SourceNeo 同步遵循 Git 的 rebase / merge / 仅快进配置；冲突时暂停，不会自动解决冲突或丢弃修改。",s.path,s.branch,s.head,s.upstream.as_deref().unwrap_or("未配置"),s.kind,s.message,s.ahead,s.behind,s.changed,s.operation.as_deref().unwrap_or("无"),if s.conflicts.is_empty(){"无".into()}else{s.conflicts.iter().map(|p|format!("- {p}")).collect::<Vec<_>>().join("\n")},s.last_fetched.map(|v|v.to_string()).unwrap_or_else(||"从未获取".into()),if errors.is_empty(){"无"}else{&errors})))
    }
}
