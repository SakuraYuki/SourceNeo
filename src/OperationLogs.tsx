import { useState } from "react";
import { api } from "./api";
import { errorText, type LogEntry, type Repo } from "./types";
import { stageLabels } from "./hooks/useOperationProgress";
export function OperationLogs({
  logs,
  repos,
  selected,
}: {
  logs: LogEntry[];
  repos: Repo[];
  selected: string;
}) {
  const [scope, setScope] = useState("current"),
    [query, setQuery] = useState(""),
    [error, setError] = useState("");
  const path = scope === "current" ? selected : scope === "all" ? "" : scope;
  const visible = logs.filter(
    (l) =>
      (!path || l.path === path || !l.path) &&
      `${l.message} ${l.operation ?? ""} ${l.path}`
        .toLowerCase()
        .includes(query.toLowerCase()),
  );
  return (
    <div className="operation-logs">
      <div className="log-tools">
        <select
          aria-label="日志仓库筛选"
          value={scope}
          onChange={(e) => setScope(e.target.value)}
        >
          <option value="current">当前仓库</option>
          <option value="all">全部仓库（含已移除）</option>
          {repos.map((r) => (
            <option key={r.path} value={r.path}>
              {r.name}
            </option>
          ))}
        </select>
        <input
          aria-label="搜索日志"
          placeholder="搜索操作或错误"
          value={query}
          onChange={(e) => setQuery(e.target.value)}
        />
        <button
          onClick={() => {
            setError("");
            void api
              .exportLogs(path || null)
              .catch((e) => setError(errorText(e)));
          }}
          title="导出所选仓库的全部保留日志，不限搜索词"
        >
          导出 JSON
        </button>
      </div>
      {error && (
        <p role="alert" className="error-text">
          {error}
        </p>
      )}
      <div className="log-list" role="log" aria-label="Git 操作日志">
        {visible.map((l, i) => (
          <div key={`${l.time}:${i}`} className={`log-entry ${l.level}`}>
            <time>{new Date(l.time * 1000).toLocaleString("zh-CN")}</time>
            <span className="log-level">
              {l.level === "error"
                ? "失败"
                : l.level === "warning"
                  ? "暂停"
                  : l.level === "success"
                    ? "完成"
                    : "信息"}
            </span>
            <span title={l.path}>
              {!path && l.path && (
                <strong>
                  {repos.find((r) => r.path === l.path)?.name ?? l.path} ·{" "}
                </strong>
              )}
              {l.message}
              {l.stage && (
                <small>
                  {" "}
                  · {stageLabels[l.stage] ?? l.stage}
                  {l.elapsedMs != null
                    ? ` · ${(l.elapsedMs / 1000).toFixed(1)} 秒`
                    : ""}
                </small>
              )}
            </span>
          </div>
        ))}
        {!visible.length && <p className="history-empty">没有匹配的操作日志</p>}
      </div>
      <p className="log-retention">
        保留最近 500 条脱敏日志，重启后可查看；导出包含本地仓库路径。
      </p>
    </div>
  );
}
