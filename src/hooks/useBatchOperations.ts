import { useRef, useState, type RefObject } from "react";
import { pooled } from "../api";
import type { Repo } from "../types";
import type { BatchProgress, BatchResult } from "../BatchReport";

export function useBatchOperations(
  repos: RefObject<Repo[]>,
  busy: RefObject<Set<string>>,
  mounted: RefObject<boolean>,
  setBusy: (value: Set<string>) => void,
  run: (
    repo: Repo,
    fetchOnly?: boolean,
    remote?: string,
    reserved?: boolean,
  ) => Promise<BatchResult>,
) {
  const [batch, setBatch] = useState<BatchProgress | null>(null);
  const guard = useRef(false),
    stopped = useRef(false);
  async function runBatch(targets: Repo[], fetchOnly = false) {
    targets = [...new Map(targets.map((r) => [r.path, r])).values()];
    if (guard.current || !targets.length) return;
    guard.current = true;
    stopped.current = false;
    const queued: Repo[] = [],
      results: BatchResult[] = [];
    for (const target of targets) {
      const current = repos.current.find((r) => r.path === target.path);
      if (!fetchOnly && current?.syncDisabled)
        results.push({
          repo: current,
          outcome: "skipped",
          message: "已禁止同步，仍可抓取",
        });
      else if (busy.current.has(target.path))
        results.push({
          repo: target,
          outcome: "skipped",
          message: "仓库正在操作",
        });
      else {
        busy.current.add(target.path);
        queued.push(target);
      }
    }
    setBusy(new Set(busy.current));
    setBatch({
      label: fetchOnly ? "批量抓取" : "批量同步",
      total: targets.length,
      running: true,
      results,
      targets,
      fetchOnly,
      startedAt: Date.now(),
      stopped: false,
    });
    try {
      await pooled(queued, async (target) => {
        const start = Date.now();
        let result: BatchResult;
        if (
          stopped.current ||
          !repos.current.some((r) => r.path === target.path)
        ) {
          busy.current.delete(target.path);
          setBusy(new Set(busy.current));
          result = {
            repo: target,
            outcome: "skipped",
            message: stopped.current ? "已停止排队" : "仓库已移除",
          };
        } else {
          result = await run(target, fetchOnly, undefined, true);
          result.elapsedMs = Date.now() - start;
        }
        if (mounted.current)
          setBatch((old) =>
            old ? { ...old, results: [...old.results, result] } : old,
          );
      });
    } finally {
      guard.current = false;
      if (mounted.current)
        setBatch((old) => (old ? { ...old, running: false } : old));
    }
  }
  function stopBatch() {
    stopped.current = true;
    setBatch((old) => (old ? { ...old, stopped: true } : old));
  }
  function retryFailed() {
    if (!batch || batch.running) return;
    const failed = new Set(
      batch.results
        .filter((r) => r.outcome === "error")
        .map((r) => r.repo.path),
    );
    void runBatch(
      repos.current.filter((r) => failed.has(r.path)),
      batch.fetchOnly,
    );
  }
  return { batch, setBatch, runBatch, stopBatch, retryFailed };
}
