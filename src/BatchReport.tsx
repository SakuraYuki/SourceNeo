import { useEffect, useId, useRef, useState } from "react";
import {
  Activity,
  AlertCircle,
  Check,
  CheckCircle2,
  ChevronDown,
  ChevronUp,
  Clock3,
  LoaderCircle,
  RotateCw,
  SkipForward,
  CircleStop,
  XCircle,
} from "lucide-react";
import { stageLabels } from "./hooks/useOperationProgress";
import type { OperationProgress, Repo } from "./types";
import { IconButton } from "./IconButton";
import { PopConfirm } from "./PopConfirm";

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
const outcomeIcons = {
  success: Check,
  attention: AlertCircle,
  error: XCircle,
  skipped: SkipForward,
};
export function BatchReport({
  batch,
  onStop,
  onRetry,
  progress,
}: {
  batch: BatchProgress | null;
  onStop: () => void;
  onRetry: () => void;
  progress: OperationProgress[];
}) {
  const [expanded, setExpanded] = useState(false);
  const [stopOrigin, setStopOrigin] = useState<HTMLElement | null>(null);
  const root = useRef<HTMLDivElement>(null);
  const trigger = useRef<HTMLButtonElement>(null);
  const timer = useRef<ReturnType<typeof setTimeout> | undefined>(undefined);
  const panelId = useId();
  const startedAt = batch?.startedAt;
  const running = batch?.running ?? false;
  const succeeded = Boolean(
    batch &&
      !batch.running &&
      batch.results.length === batch.total &&
      batch.results.every((r) => r.outcome === "success"),
  );
  useEffect(() => {
    setExpanded(startedAt !== undefined);
    setStopOrigin(null);
  }, [startedAt]);
  useEffect(() => {
    if (succeeded) timer.current = setTimeout(() => setExpanded(false), 5000);
    return () => clearTimeout(timer.current);
  }, [startedAt, running, succeeded]);
  useEffect(() => {
    if (!expanded) {
      setStopOrigin(null);
      return;
    }
    const outside = (event: PointerEvent) => {
      if (!root.current?.contains(event.target as Node)) setExpanded(false);
    };
    const escape = (event: KeyboardEvent) => {
      if (event.key !== "Escape" || event.defaultPrevented) return;
      // Foreground dialogs and menus own Escape; opening a batch does not move focus.
      if (document.querySelector('dialog[open], [role="menu"]')) return;
      event.preventDefault();
      event.stopPropagation();
      clearTimeout(timer.current);
      setExpanded(false);
      trigger.current?.focus();
    };
    document.addEventListener("pointerdown", outside);
    window.addEventListener("keydown", escape);
    return () => {
      document.removeEventListener("pointerdown", outside);
      window.removeEventListener("keydown", escape);
    };
  }, [expanded]);
  function toggle() {
    clearTimeout(timer.current);
    setExpanded((old) => !old);
  }
  const results = new Map(
    batch?.results.map((result) => [result.repo.path, result]),
  );
  const active = new Map(progress.map((item) => [item.path, item]));
  const counts = Object.entries(outcomes).map(([key, label]) => ({
    key,
    label,
    count:
      batch?.results.filter((result) => result.outcome === key).length ?? 0,
  }));
  const percentage = batch?.total
    ? Math.round((batch.results.length / batch.total) * 100)
    : 0;
  return (
    <div className="batch-status" ref={root}>
      <button
        ref={trigger}
        className={`batch-status-trigger ${batch && !running && !succeeded ? "has-issues" : ""}`}
        disabled={!batch}
        aria-expanded={expanded}
        aria-controls={panelId}
        aria-label={
          batch
            ? `${batch.label}${running ? "进行中" : "结果"}，${batch.results.length}/${batch.total}，${expanded ? "收起" : "展开"}`
            : "任务状态"
        }
        onClick={toggle}
      >
        {running ? (
          <LoaderCircle size={13} className="spin" />
        ) : succeeded ? (
          <CheckCircle2 size={13} />
        ) : batch ? (
          <AlertCircle size={13} />
        ) : (
          <Activity size={13} />
        )}
        <span>
          {batch
            ? `${running ? "进行中" : succeeded ? "已完成" : "需关注"} ${batch.results.length}/${batch.total}`
            : "任务状态"}
        </span>
        {expanded ? <ChevronUp size={12} /> : <ChevronDown size={12} />}
      </button>
      {batch && expanded && (
        <section
          className={`batch-report ${succeeded ? "is-success" : !running ? "has-issues" : "is-running"}`}
          id={panelId}
          aria-label="批量任务状态"
        >
          <div className="batch-report-header">
            <div className="batch-heading">
              <span className="batch-heading-icon" aria-hidden="true">
                {running ? (
                  <LoaderCircle size={22} className="spin" />
                ) : succeeded ? (
                  <CheckCircle2 size={22} />
                ) : (
                  <AlertCircle size={22} />
                )}
              </span>
              <div>
                <h2>{batch.label}</h2>
                <p>
                  {running
                    ? "正在处理仓库，请稍候"
                    : succeeded
                      ? "所有仓库均已处理完成"
                      : "任务已结束，部分仓库需要关注"}
                </p>
              </div>
            </div>
            <IconButton
              className="batch-collapse"
              label="收起批量任务状态"
              icon={ChevronUp}
              onClick={toggle}
            />
            <span role="status" className="batch-announcement">
              {batch.label}
              {batch.running ? "进行中" : "完成"} · {batch.results.length}/
              {batch.total}
              {counts.map(({ count, label }) => {
                return count ? ` · ${count} ${label}` : "";
              })}
            </span>
          </div>
          <div className="batch-overview">
            <div className="batch-progress-label">
              <span>
                已完成 <strong>{batch.results.length}</strong> / {batch.total}{" "}
                个仓库
              </span>
              <span>{percentage}%</span>
            </div>
            <progress
              aria-label="批量操作进度"
              max={batch.total}
              value={batch.results.length}
            />
            <div className="batch-counts">
              {counts
                .filter(({ count }) => count > 0)
                .map(({ key, label, count }) => (
                  <span key={key} className={`batch-count ${key}`}>
                    <i />
                    {label} <strong>{count}</strong>
                  </span>
                ))}
              {running && (
                <span className="batch-count pending">
                  <i />
                  待完成 <strong>{batch.total - batch.results.length}</strong>
                </span>
              )}
            </div>
          </div>
          <div className="batch-list-heading">
            <span>仓库任务</span>
            <span>状态 / 耗时</span>
          </div>
          <ul className="batch-task-list" aria-label="各仓库任务">
            {batch.targets.map((repo) => {
              const result = results.get(repo.path);
              const operation = active.get(repo.path);
              const state =
                result?.outcome ?? (operation ? "running" : "pending");
              const Icon = result
                ? outcomeIcons[result.outcome]
                : operation
                  ? LoaderCircle
                  : Clock3;
              const duration = result?.elapsedMs ?? operation?.elapsedMs;
              const label = result
                ? outcomes[result.outcome]
                : operation
                  ? (stageLabels[operation.stage] ?? operation.stage)
                  : "排队中";
              return (
                <li key={repo.path} className={`batch-task ${state}`}>
                  <span className="batch-task-icon" aria-hidden="true">
                    <Icon
                      size={16}
                      className={state === "running" ? "spin" : undefined}
                    />
                  </span>
                  <div className="batch-task-content">
                    <div className="batch-task-title">
                      <strong title={repo.path}>{repo.name}</strong>
                      <span className="batch-task-state">{label}</span>
                      <time>
                        {duration != null
                          ? `${(duration / 1000).toFixed(1)} 秒`
                          : "—"}
                      </time>
                    </div>
                    <p>
                      {result
                        ? `${outcomes[result.outcome]}：${result.message}`
                        : operation
                          ? `正在${label}，请稍候…`
                          : "等待空闲任务或仓库锁"}
                    </p>
                  </div>
                </li>
              );
            })}
          </ul>
          <div className="batch-footer">
            <p className="batch-hint">
              <span
                className={`dot ${running ? "" : succeeded ? "synced" : "attention"}`}
              />
              {running
                ? "收起后，任务会继续执行"
                : succeeded
                  ? "成功后自动收起，可从顶部再次查看"
                  : "结果将保留，处理后可重试失败项"}
            </p>
            {running && (
              <button
                className="warning"
                disabled={batch.stopped}
                aria-haspopup="dialog"
                aria-expanded={Boolean(stopOrigin)}
                onClick={(event) =>
                  setStopOrigin(stopOrigin ? null : event.currentTarget)
                }
              >
                <CircleStop size={13} />
                {batch.stopped ? "等待进行中的任务完成" : "停止排队"}
              </button>
            )}
            {!running &&
              results.size > 0 &&
              counts.some((item) => item.key === "error" && item.count > 0) && (
                <button className="batch-retry" onClick={onRetry}>
                  <RotateCw size={14} />
                  仅重试失败项
                </button>
              )}
          </div>
          {running && !batch.stopped && stopOrigin && (
            <PopConfirm
              origin={stopOrigin}
              label="停止排队确认"
              title="停止后续排队任务？"
              description="尚未开始的仓库将跳过，正在进行的 Git 操作会继续完成。"
              confirmText="停止排队"
              tone="warning"
              onClose={() => setStopOrigin(null)}
              onConfirm={() => {
                setStopOrigin(null);
                onStop();
              }}
            />
          )}
        </section>
      )}
    </div>
  );
}
