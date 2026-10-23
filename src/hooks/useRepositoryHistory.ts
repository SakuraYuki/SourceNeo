import { useCallback, useEffect, useRef, useState } from "react";
import { api } from "../api";
import { errorText, type History } from "../types";

export function useRepositoryHistory(
  selected: string,
  allBranches: boolean,
  fingerprint: string,
) {
  const [history, setHistory] = useState<History | null>(null);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState("");
  const [limit, setLimit] = useState(100);
  const request = useRef(0);
  const current = useRef({ selected, allBranches });
  current.current = { selected, allBranches };
  const cache = useRef(
    new Map<
      string,
      { history: History; limit: number; fingerprint: string; at: number }
    >(),
  );
  const signature = useRef(fingerprint);
  signature.current = fingerprint;
  const loadHistory = useCallback(async (path: string, count: number) => {
    const id = ++request.current;
    if (!path) {
      setHistory(null);
      setHistoryError("");
      setHistoryLoading(false);
      return;
    }
    const scope = current.current.allBranches;
    setHistoryLoading(true);
    setHistoryError("");
    try {
      const result = await api.history(path, count, scope);
      if (id !== request.current || current.current.selected !== path) return;
      const key = `${path}:${scope}`;
      cache.current.delete(key);
      cache.current.set(key, {
        history: result,
        limit: count,
        fingerprint: signature.current,
        at: Date.now(),
      });
      if (cache.current.size > 8)
        cache.current.delete(cache.current.keys().next().value!);
      setHistory(result);
    } catch (e) {
      if (id === request.current) setHistoryError(errorText(e));
    } finally {
      if (id === request.current) setHistoryLoading(false);
    }
  }, []);
  const previous = useRef({
    selected: "",
    scope: allBranches,
    fingerprint: "",
  });
  useEffect(() => {
    const old = previous.current;
    previous.current = { selected, scope: allBranches, fingerprint };
    const moved = old.selected !== selected || old.scope !== allBranches;
    if (moved) {
      ++request.current;
      const cached = cache.current.get(`${selected}:${allBranches}`);
      // Scope toggles deliberately start at the first page; repository navigation restores a recent view.
      if (
        old.selected !== selected &&
        cached &&
        Date.now() - cached.at < 60_000 &&
        (!fingerprint || cached.fingerprint === fingerprint)
      ) {
        setHistory(cached.history);
        setLimit(cached.limit);
        setHistoryError("");
        setHistoryLoading(false);
      } else {
        setHistory(null);
        setLimit(100);
        void loadHistory(selected, 100);
      }
    } else if (fingerprint && !old.fingerprint) {
      const cached = cache.current.get(`${selected}:${allBranches}`);
      if (cached) cached.fingerprint = fingerprint;
    } else if (
      !selected ||
      (fingerprint && old.fingerprint && fingerprint !== old.fingerprint)
    ) {
      setLimit(100);
      void loadHistory(selected, 100);
    }
  }, [selected, allBranches, fingerprint, loadHistory]);
  useEffect(
    () => () => {
      request.current++;
    },
    [],
  );
  return {
    history,
    historyLoading,
    historyError,
    limit,
    setLimit,
    loadHistory,
  };
}
