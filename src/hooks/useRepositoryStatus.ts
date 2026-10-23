import { useCallback, useRef, useState, type RefObject } from "react";
import { api, pooled } from "../api";
import { errorText, type Repo, type RepoStatus } from "../types";

export function statusSignature(status: RepoStatus) {
  const { checkedAt: _checkedAt, ...content } = status;
  return JSON.stringify(content);
}
export function nextInterval(unchanged: number) {
  return Math.min(120_000, 15_000 * 2 ** Math.min(unchanged, 3));
}
export function useRepositoryStatus(
  repos: RefObject<Repo[]>,
  busy: RefObject<Set<string>>,
  selected: RefObject<string>,
  mounted: RefObject<boolean>,
) {
  const [statuses, setStatuses] = useState<Record<string, RepoStatus>>({});
  const [failures, setFailures] = useState<Record<string, string>>({});
  const [refreshing, setRefreshing] = useState(false);
  const schedule = useRef(
    new Map<
      string,
      { signature: string; unchanged: number; checked: number }
    >(),
  );
  const guard = useRef(false);
  const lastRefresh = useRef(0);
  const readStatus = useCallback(
    async (repo: Repo) => {
      try {
        const s = await api.status(repo.path);
        if (
          !mounted.current ||
          !repos.current.some((r) => r.path === repo.path)
        )
          return;
        const old = schedule.current.get(repo.path);
        const signature = statusSignature(s);
        schedule.current.set(repo.path, {
          signature,
          unchanged: old?.signature === signature ? old.unchanged + 1 : 0,
          checked: Date.now(),
        });
        setStatuses((old) => ({ ...old, [repo.path]: s }));
        setFailures((old) => {
          const next = { ...old };
          delete next[repo.path];
          return next;
        });
      } catch (e) {
        if (
          mounted.current &&
          repos.current.some((r) => r.path === repo.path)
        ) {
          setFailures((old) => ({ ...old, [repo.path]: errorText(e) }));
          schedule.current.delete(repo.path);
        }
      }
    },
    [mounted, repos],
  );
  const refresh = useCallback(
    async (force = false) => {
      const now = Date.now();
      if (guard.current || now - lastRefresh.current < (force ? 1000 : 10000))
        return;
      guard.current = true;
      lastRefresh.current = now;
      setRefreshing(true);
      const paths = new Set(repos.current.map((r) => r.path));
      for (const path of schedule.current.keys())
        if (!paths.has(path)) schedule.current.delete(path);
      try {
        const due = repos.current.filter((r) => {
          if (busy.current.has(r.path)) return false;
          const item = schedule.current.get(r.path);
          return (
            force ||
            !item ||
            now - item.checked >=
              (r.path === selected.current
                ? 10_000
                : nextInterval(item.unchanged))
          );
        });
        due.sort(
          (a, b) =>
            Number(b.path === selected.current) -
            Number(a.path === selected.current),
        );
        await pooled(due, readStatus);
      } finally {
        guard.current = false;
        if (mounted.current) setRefreshing(false);
      }
    },
    [repos, busy, selected, mounted, readStatus],
  );
  return {
    statuses,
    setStatuses,
    failures,
    setFailures,
    refreshing,
    readStatus,
    refresh,
  };
}
