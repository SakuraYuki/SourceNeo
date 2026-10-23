import { invoke, isTauri } from "@tauri-apps/api/core";
import { open, save } from "@tauri-apps/plugin-dialog";
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
  RepoGroup,
  GroupEdit,
  HistoryPage,
  HistoryQuery,
  Comparison,
  SyncPreview,
  ScanResult,
  OperationProgress,
} from "./types";
export const desktop = isTauri();
export const api = {
  theme: (theme: "light" | "dark") =>
    desktop ? getCurrentWindow().setTheme(theme) : Promise.resolve(),
  list: () => (desktop ? invoke<Repo[]>("list_repos") : Promise.resolve([])),
  groups: () =>
    desktop ? invoke<RepoGroup[]>("repo_groups") : Promise.resolve([]),
  editGroups: (edit: GroupEdit) =>
    invoke<RepoGroup[]>("edit_repo_groups", { edit }),
  add: (path: string) => invoke<Repo>("add_repo", { path }),
  remove: (path: string) => invoke<void>("remove_repo", { path }),
  status: (path: string) => invoke<RepoStatus>("repo_status", { path }),
  history: readHistory,
  historyPage: (
    path: string,
    allBranches: boolean,
    query: HistoryQuery,
    snapshot: string | null = null,
    offset = 0,
  ) =>
    invoke<HistoryPage>("repo_history_page", {
      path,
      allBranches,
      query,
      snapshot,
      offset,
    }),
  pickFolder: () =>
    open({ directory: true, multiple: false, title: "选择父目录" }),
  compareDiff: (path: string, head: string, target: string, file: string) =>
    invoke<string>("compare_diff", { path, head, target, file }),
  compare: (path: string) => invoke<Comparison>("compare_upstream", { path }),
  preview: (path: string) => invoke<SyncPreview>("sync_preview", { path }),
  scan: (root: string, depth: number) =>
    invoke<ScanResult>("scan_repositories", { root, depth }),
  clone: (url: string, parent: string, name: string, requestId: string) =>
    invoke<Repo>("clone_repository", { url, parent, name, requestId }),
  cancelClone: (requestId: string) =>
    invoke<void>("cancel_clone", { requestId }),
  progress: () =>
    desktop
      ? invoke<OperationProgress[]>("operation_progress")
      : Promise.resolve([]),
  exportLogs: async (path: string | null) => {
    const destination = await save({
      defaultPath: "SourceNeo-operations.json",
      filters: [{ name: "JSON", extensions: ["json"] }],
    });
    if (destination) await invoke<void>("export_logs", { destination, path });
    return Boolean(destination);
  },
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

// The public accumulated-history shape keeps the view simple; IPC sends only new pages.
const pages = new Map<string, HistoryPage>();
const generations = new Map<string, symbol>();
async function readHistory(
  path: string,
  limit: number,
  allBranches = false,
): Promise<History> {
  const key = `${path}:${allBranches}`;
  const generation = Symbol(key);
  generations.delete(key);
  generations.set(key, generation);
  if (generations.size > 16)
    generations.delete(generations.keys().next().value!);
  const previous = pages.get(key);
  let state =
    previous &&
    limit > 100 &&
    limit > previous.commits.length &&
    previous.hasMore
      ? previous
      : undefined;
  const count = Math.min(5000, Math.max(100, limit));
  do {
    let page: HistoryPage;
    try {
      page = await invoke("repo_history_page", {
        path,
        allBranches,
        snapshot: state?.snapshot ?? null,
        offset: state?.commits.length ?? 0,
        query: null,
      });
    } catch (e) {
      if (
        state &&
        typeof e === "object" &&
        e !== null &&
        "kind" in e &&
        e.kind === "snapshot"
      ) {
        state = undefined;
        page = await invoke("repo_history_page", {
          path,
          allBranches,
          snapshot: null,
          offset: 0,
          query: null,
        });
      } else throw e;
    }
    state = { ...page, commits: [...(state?.commits ?? []), ...page.commits] };
  } while (state.hasMore && state.commits.length < count);
  if (generations.get(key) === generation) {
    pages.delete(key);
    pages.set(key, state);
    while (pages.size > 8) {
      const oldest = pages.keys().next().value!;
      pages.delete(oldest);
    }
  }
  return state;
}
