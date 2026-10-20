export interface Repo {
  path: string;
  name: string;
  commonDir: string;
}
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
  time: number;
  path: string;
  level: string;
  message: string;
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
