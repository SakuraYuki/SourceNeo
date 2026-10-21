import { invoke, isTauri } from "@tauri-apps/api/core";
import { open } from "@tauri-apps/plugin-dialog";
import { writeText } from "@tauri-apps/plugin-clipboard-manager";
import { getCurrentWindow } from "@tauri-apps/api/window";
import type {
  Repo,
  RepoStatus,
  History,
  LogEntry,
  Branch,
  CommitChanges,
  ChangedFile,
} from "./types";
export const desktop = isTauri();
export const api = {
  theme: (theme: "light" | "dark") =>
    desktop ? getCurrentWindow().setTheme(theme) : Promise.resolve(),
  list: () => (desktop ? invoke<Repo[]>("list_repos") : Promise.resolve([])),
  add: (path: string) => invoke<Repo>("add_repo", { path }),
  remove: (path: string) => invoke<void>("remove_repo", { path }),
  status: (path: string) => invoke<RepoStatus>("repo_status", { path }),
  history: (path: string, limit: number) =>
    invoke<History>("repo_history", { path, limit }),
  sync: (path: string) => invoke<RepoStatus>("sync_repo", { path }),
  fetch: (path: string, remote?: string) =>
    invoke<RepoStatus>("fetch_repo", { path, remote }),
  remotes: (path: string) => invoke<string[]>("fetch_remotes", { path }),
  toolSettings: () =>
    invoke<{ terminal: string; editor: string }>("tool_settings"),
  saveToolSettings: (settings: { terminal: string; editor: string }) =>
    invoke<void>("save_tool_settings", { settings }),
  openTool: (path: string, kind: string) =>
    invoke<void>("open_tool", { path, kind }),
  branches: (path: string) => invoke<Branch[]>("repo_branches", { path }),
  workingChanges: (path: string) =>
    invoke<ChangedFile[]>("working_changes", { path }),
  workingDiff: (path: string, file: string, side: string) =>
    invoke<string>("working_diff", { path, file, side }),
  switchBranch: (path: string, reference: string, create = false) =>
    invoke<RepoStatus>("switch_branch", { path, reference, create }),
  changes: (path: string, hash: string) =>
    invoke<CommitChanges>("commit_changes", { path, hash }),
  diff: (path: string, hash: string, file: string) =>
    invoke<string>("file_diff", { path, hash, file }),
  logs: () =>
    desktop ? invoke<LogEntry[]>("operation_logs") : Promise.resolve([]),
  diagnostic: (path: string) => invoke<string>("copy_diagnostic", { path }),
  pick: () =>
    open({
      directory: true,
      multiple: false,
      title: "添加本地 Git 仓库或 worktree",
    }),
  copy: (text: string) =>
    desktop ? writeText(text) : navigator.clipboard.writeText(text),
};
/** Keep work bounded even before requests reach the Rust worker gate. */
export async function pooled<T>(
  items: T[],
  work: (item: T) => Promise<void>,
  concurrency = 3,
) {
  let cursor = 0;
  await Promise.all(
    Array.from({ length: Math.min(concurrency, items.length) }, async () => {
      while (cursor < items.length) {
        const item = items[cursor++];
        await work(item);
      }
    }),
  );
}
