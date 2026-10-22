import type { Repo } from "./types";

export interface BatchResult {
  repo: Repo;
  outcome: "success" | "attention" | "error" | "skipped";
  message: string;
}
export interface BatchProgress {
  label: string;
  total: number;
  running: boolean;
  results: BatchResult[];
}
const outcomes = {
  success: "成功",
  attention: "需处理",
  error: "失败",
  skipped: "跳过",
};
export function BatchReport({
  batch,
  onClose,
}: {
  batch: BatchProgress;
  onClose: () => void;
}) {
  return (
    <div className="batch-report">
      <div className="batch-summary">
        <span role="status">
          {batch.label}
          {batch.running ? "进行中" : "完成"} · {batch.results.length}/
          {batch.total}
          {Object.entries(outcomes).map(([key, label]) => {
            const count = batch.results.filter((r) => r.outcome === key).length;
            return count ? ` · ${count} ${label}` : "";
          })}
        </span>
        {!batch.running && (
          <button
            className="text-btn"
            aria-label="关闭批量操作结果"
            onClick={onClose}
          >
            关闭
          </button>
        )}
      </div>
      <progress
        aria-label="批量操作进度"
        max={batch.total}
        value={batch.results.length}
      />
      {batch.results.length > 0 && (
        <details>
          <summary>查看各仓库结果</summary>
          <ul>
            {batch.results.map((r) => (
              <li
                key={r.repo.path}
                className={r.outcome === "error" ? "error-text" : ""}
              >
                <strong title={r.repo.path}>{r.repo.name}</strong> ·{" "}
                {outcomes[r.outcome]}：{r.message}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}
