pub mod git;
pub mod model;
pub mod service;
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
    pub async fn repo_history(state: Backend<'_>, path: String, limit: usize) -> Result<History> {
        let s = state.inner().clone();
        blocking(move || s.history(&path, limit)).await
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
                list_repos,
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
