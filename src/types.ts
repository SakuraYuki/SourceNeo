export interface Repo {
  path: string;
  name: string;
  commonDir: string;
}
export interface RepoGroup {
  name: string;
  paths: string[];
}
export type GroupEdit =
  | { action: "create"; name: string }
  | { action: "rename"; group: string; name: string }
  | { action: "remove"; group: string }
  | { action: "move"; paths: string[]; group: string | null };
export type Kind =
  | "synced"
  | "ahead"
  | "behind"
  | "diverged"
  | "conflict"
  | "attention"
  | "error";
export interface RepoStatus {
  path: string;
  branch: string;
  head: string;
  upstream: string | null;
  ahead: number;
  behind: number;
  changed: number;
  conflicts: string[];
  operation: string | null;
  kind: Kind;
  message: string;
  errorKind: string | null;
  reason?: string | null;
  lastFetched: number | null;
  checkedAt: number;
  shallow: boolean;
}
export interface Commit {
  hash: string;
  parents: string[];
  subject: string;
  author: string;
  date: string;
  refs: string[];
}
export interface History {
  commits: Commit[];
  hasMore: boolean;
  shallow: boolean;
}
export interface LogEntry {
  operation?: string | null;
  stage?: string | null;
  elapsedMs?: number | null;
  time: number;
  path: string;
  level: string;
  message: string;
}
export interface Branch {
  reference: string;
  name: string;
  remote: boolean;
  current: boolean;
}
export interface ChangedFile {
  path: string;
  oldPath: string | null;
  status: string;
}
export interface CommitChanges {
  hash: string;
  parent: string | null;
  files: ChangedFile[];
}
export const labels: Record<Kind, string> = {
  synced: "已同步",
  ahead: "待推送",
  behind: "待拉取",
  diverged: "已分叉",
  conflict: "已有冲突",
  attention: "需要处理",
  error: "同步失败",
};
export function errorText(e: unknown): string {
  return typeof e === "object" && e !== null && "message" in e
    ? String(e.message)
    : String(e);
}

export interface HistoryQuery {
  field: "message" | "author" | "sha";
  text: string;
}
export interface HistoryPage extends History {
  snapshot: string;
}
export interface Comparison {
  head: string;
  upstream: string;
  target: string;
  local: History;
  remote: History;
  files: ChangedFile[];
}
export interface SyncPreview {
  status: RepoStatus;
  remote: string | null;
  policy: string;
  steps: string[];
  blockers: string[];
}
export interface OperationProgress {
  path: string;
  operation: string;
  stage: string;
  startedAt: number;
  elapsedMs: number;
}
export interface ScanResult {
  repos: Repo[];
  warnings: string[];
  truncated: boolean;
}
