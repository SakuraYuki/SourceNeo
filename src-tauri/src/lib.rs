pub mod acquisition;
pub mod git;
pub mod history;
mod journal;
pub mod model;
pub mod network;
pub mod service;
mod sync_policy;
pub mod tools;

#[cfg(feature = "desktop")]
mod desktop {
    use super::{model::*, service::Service};
    use std::sync::Arc;
    use tauri::{Manager, State};
    type Backend<'a> = State<'a, Arc<Service>>;
    async fn blocking<T: Send + 'static>(
        f: impl FnOnce() -> Result<T> + Send + 'static,
    ) -> Result<T> {
        tauri::async_runtime::spawn_blocking(f)
            .await
            .map_err(|e| GitError::new("internal", e.to_string()))?
    }
    #[tauri::command]
    pub async fn list_repos(state: Backend<'_>) -> Result<Vec<Repo>> {
        let s = state.inner().clone();
        blocking(move || Ok(s.list())).await
    }
    #[tauri::command]
    pub async fn repo_groups(state: Backend<'_>) -> Result<Vec<RepoGroup>> {
        let s = state.inner().clone();
        blocking(move || Ok(s.groups())).await
    }
    #[tauri::command]
    pub async fn edit_repo_groups(state: Backend<'_>, edit: GroupEdit) -> Result<Vec<RepoGroup>> {
        let s = state.inner().clone();
        blocking(move || s.edit_groups(edit)).await
    }
    #[tauri::command]
    pub async fn add_repo(state: Backend<'_>, path: String) -> Result<Repo> {
        let s = state.inner().clone();
        blocking(move || s.add(&path)).await
    }
    #[tauri::command]
    pub async fn remove_repo(state: Backend<'_>, path: String) -> Result<()> {
        let s = state.inner().clone();
        blocking(move || s.remove(&path)).await
    }
    #[tauri::command]
    pub async fn repo_status(state: Backend<'_>, path: String) -> Result<RepoStatus> {
        let s = state.inner().clone();
        blocking(move || s.status(&path)).await
    }
    #[tauri::command]
    pub async fn repo_history(
        state: Backend<'_>,
        path: String,
        limit: usize,
        all_branches: Option<bool>,
    ) -> Result<History> {
        let s = state.inner().clone();
        blocking(move || s.history(&path, limit, all_branches.unwrap_or(false))).await
    }
    #[tauri::command]
    pub async fn sync_repo(state: Backend<'_>, path: String) -> Result<RepoStatus> {
        let s = state.inner().clone();
        blocking(move || s.sync(&path)).await
    }
    #[tauri::command]
    pub async fn repo_branches(state: Backend<'_>, path: String) -> Result<Vec<Branch>> {
        let s = state.inner().clone();
        blocking(move || s.branches(&path)).await
    }
    #[tauri::command]
    pub async fn working_changes(state: Backend<'_>, path: String) -> Result<Vec<ChangedFile>> {
        let s = state.inner().clone();
        blocking(move || s.working_changes(&path)).await
    }
    #[tauri::command]
    pub async fn network_settings(state: Backend<'_>) -> Result<crate::network::NetworkSettings> {
        Ok(state.network_settings())
    }
    #[tauri::command]
    pub async fn save_network_settings(
        state: Backend<'_>,
        settings: crate::network::NetworkSettings,
    ) -> Result<()> {
        let s = state.inner().clone();
        blocking(move || s.save_network_settings(settings)).await
    }
    #[tauri::command]
    pub async fn tool_settings(state: Backend<'_>) -> Result<crate::tools::ToolSettings> {
        Ok(state.tool_settings())
    }
    #[tauri::command]
    pub async fn save_tool_settings(
        state: Backend<'_>,
        settings: crate::tools::ToolSettings,
    ) -> Result<()> {
        let s = state.inner().clone();
        blocking(move || s.save_tool_settings(settings)).await
    }
    #[tauri::command]
    pub async fn open_tool(state: Backend<'_>, path: String, kind: String) -> Result<()> {
        let s = state.inner().clone();
        blocking(move || s.open_tool(&path, &kind)).await
    }
    #[tauri::command]
    pub async fn fetch_remotes(state: Backend<'_>, path: String) -> Result<Vec<String>> {
        let s = state.inner().clone();
        blocking(move || s.fetch_remotes(&path)).await
    }
    #[tauri::command]
    pub async fn fetch_repo(
        state: Backend<'_>,
        path: String,
        remote: Option<String>,
    ) -> Result<RepoStatus> {
        let s = state.inner().clone();
        blocking(move || s.fetch(&path, remote.as_deref())).await
    }
    #[tauri::command]
    pub async fn working_diff(
        state: Backend<'_>,
        path: String,
        file: String,
        side: String,
    ) -> Result<String> {
        let s = state.inner().clone();
        blocking(move || s.working_diff(&path, &file, &side)).await
    }
    #[tauri::command]
    pub async fn switch_branch(
        state: Backend<'_>,
        path: String,
        reference: String,
        create: bool,
    ) -> Result<RepoStatus> {
        let s = state.inner().clone();
        blocking(move || s.switch_branch(&path, &reference, create)).await
    }
    #[tauri::command]
    pub async fn commit_changes(
        state: Backend<'_>,
        path: String,
        hash: String,
    ) -> Result<CommitChanges> {
        let s = state.inner().clone();
        blocking(move || s.commit_changes(&path, &hash)).await
    }
    #[tauri::command]
    pub async fn file_diff(
        state: Backend<'_>,
        path: String,
        hash: String,
        file: String,
    ) -> Result<String> {
        let s = state.inner().clone();
        blocking(move || s.file_diff(&path, &hash, &file)).await
    }
    #[tauri::command]
    pub async fn operation_logs(state: Backend<'_>) -> Result<Vec<LogEntry>> {
        let s = state.inner().clone();
        blocking(move || Ok(s.logs())).await
    }
    #[tauri::command]
    pub async fn copy_diagnostic(state: Backend<'_>, path: String) -> Result<String> {
        let s = state.inner().clone();
        blocking(move || s.diagnostic(&path)).await
    }
    #[tauri::command]
    pub async fn repo_history_page(
        state: Backend<'_>,
        path: String,
        all_branches: bool,
        snapshot: Option<String>,
        offset: usize,
        query: Option<HistoryQuery>,
    ) -> Result<HistoryPage> {
        let s = state.inner().clone();
        blocking(move || {
            s.history_page(
                &path,
                all_branches,
                snapshot.as_deref(),
                offset,
                query.unwrap_or_default(),
            )
        })
        .await
    }
    #[tauri::command]
    pub async fn compare_upstream(state: Backend<'_>, path: String) -> Result<Comparison> {
        let s = state.inner().clone();
        blocking(move || s.compare(&path)).await
    }
    #[tauri::command]
    pub async fn sync_preview(state: Backend<'_>, path: String) -> Result<SyncPreview> {
        let s = state.inner().clone();
        blocking(move || s.preview(&path)).await
    }
    #[tauri::command]
    pub async fn scan_repositories(
        state: Backend<'_>,
        root: String,
        depth: usize,
    ) -> Result<ScanResult> {
        let s = state.inner().clone();
        blocking(move || s.scan(&root, depth)).await
    }
    #[tauri::command]
    pub async fn clone_repository(
        state: Backend<'_>,
        url: String,
        parent: String,
        name: String,
        request_id: String,
    ) -> Result<Repo> {
        let s = state.inner().clone();
        blocking(move || s.clone_with_id(&request_id, &url, &parent, &name)).await
    }
    #[tauri::command]
    pub fn cancel_clone(state: Backend<'_>, request_id: String) -> Result<()> {
        state.cancel_clone(&request_id)
    }
    #[tauri::command]
    pub fn operation_progress(state: Backend<'_>) -> Vec<OperationProgress> {
        state.progress()
    }
    #[tauri::command]
    pub async fn export_logs(
        state: Backend<'_>,
        destination: String,
        path: Option<String>,
    ) -> Result<()> {
        let s = state.inner().clone();
        blocking(move || {
            let logs: Vec<_> = s
                .logs()
                .into_iter()
                .filter(|l| {
                    path.as_ref()
                        .is_none_or(|p| l.path == super::git::redact(p))
                })
                .collect();
            let destination = std::path::Path::new(&destination);
            let parent = destination
                .parent()
                .ok_or_else(|| GitError::new("path", "无效导出路径"))?;
            let mut file = tempfile::NamedTempFile::new_in(parent)
                .map_err(|e| GitError::new("io", e.to_string()))?;
            use std::io::Write;
            file.write_all(
                &serde_json::to_vec_pretty(&logs)
                    .map_err(|e| GitError::new("io", e.to_string()))?,
            )
            .map_err(|e| GitError::new("io", e.to_string()))?;
            file.persist(destination)
                .map_err(|e| GitError::new("io", e.to_string()))?;
            Ok(())
        })
        .await
    }
    #[tauri::command]
    pub async fn compare_diff(
        state: Backend<'_>,
        path: String,
        head: String,
        target: String,
        file: String,
    ) -> Result<String> {
        let s = state.inner().clone();
        blocking(move || s.compare_diff(&path, &head, &target, &file)).await
    }
    pub fn run() {
        tauri::Builder::default()
            .plugin(tauri_plugin_single_instance::init(|app, _, _| {
                if let Some(w) = app.get_webview_window("main") {
                    let _ = w.unminimize();
                    let _ = w.set_focus();
                }
            }))
            .plugin(tauri_plugin_dialog::init())
            .plugin(tauri_plugin_clipboard_manager::init())
            .plugin(
                tauri_plugin_window_state::Builder::default()
                    .with_state_flags(
                        tauri_plugin_window_state::StateFlags::SIZE
                            | tauri_plugin_window_state::StateFlags::POSITION
                            | tauri_plugin_window_state::StateFlags::MAXIMIZED
                            | tauri_plugin_window_state::StateFlags::FULLSCREEN
                            | tauri_plugin_window_state::StateFlags::VISIBLE,
                    )
                    .build(),
            )
            .setup(|app| {
                let config_dir = app.path().app_config_dir()?;
                #[cfg(debug_assertions)]
                let config_dir = std::env::var_os("SOURCENEO_CONFIG_DIR")
                    .map(std::path::PathBuf::from)
                    .unwrap_or(config_dir);
                let config = config_dir.join("repositories.json");
                app.manage(Arc::new(Service::new(config)?));
                Ok(())
            })
            .invoke_handler(tauri::generate_handler![
                repo_history_page,
                compare_upstream,
                compare_diff,
                sync_preview,
                scan_repositories,
                clone_repository,
                cancel_clone,
                operation_progress,
                export_logs,
                list_repos,
                repo_groups,
                edit_repo_groups,
                add_repo,
                remove_repo,
                repo_status,
                repo_history,
                sync_repo,
                repo_branches,
                working_changes,
                working_diff,
                fetch_remotes,
                fetch_repo,
                network_settings,
                save_network_settings,
                tool_settings,
                save_tool_settings,
                open_tool,
                switch_branch,
                commit_changes,
                file_diff,
                operation_logs,
                copy_diagnostic
            ])
            .run(tauri::generate_context!())
            .expect("SourceNeo 启动失败");
    }
}
#[cfg(feature = "desktop")]
pub use desktop::run;
