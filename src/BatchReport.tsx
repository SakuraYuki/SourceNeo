import { stageLabels } from "./hooks/useOperationProgress";
import type { OperationProgress, Repo } from "./types";

export interface BatchResult {
  repo: Repo;
  outcome: "success" | "attention" | "error" | "skipped";
  message: string;
  elapsedMs?: number;
}
export interface BatchProgress {
  label: string;
  total: number;
  running: boolean;
  results: BatchResult[];
  targets: Repo[];
  fetchOnly: boolean;
  startedAt: number;
  stopped: boolean;
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
  onStop,
  onRetry,
  progress,
}: {
  batch: BatchProgress;
  onClose: () => void;
  onStop: () => void;
  onRetry: () => void;
  progress: OperationProgress[];
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
        {batch.running && (
          <button disabled={batch.stopped} onClick={onStop}>
            {batch.stopped ? "等待进行中的任务完成" : "停止排队"}
          </button>
        )}
        {!batch.running && batch.results.some((r) => r.outcome === "error") && (
          <button onClick={onRetry}>仅重试失败项</button>
        )}
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
      {batch.running && (
        <div className="batch-active">
          {batch.targets
            .filter(
              (r) =>
                !batch.results.some((result) => result.repo.path === r.path),
            )
            .map((r) => {
              const active = progress.find((p) => p.path === r.path);
              return (
                <span key={r.path}>
                  {r.name} ·{" "}
                  {active
                    ? `${stageLabels[active.stage] ?? active.stage} · ${(active.elapsedMs / 1000).toFixed(0)} 秒`
                    : "排队 / 等待仓库锁"}
                </span>
              );
            })}
        </div>
      )}
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
                {r.elapsedMs != null &&
                  ` · ${(r.elapsedMs / 1000).toFixed(1)} 秒`}
              </li>
            ))}
          </ul>
        </details>
      )}
    </div>
  );
}
