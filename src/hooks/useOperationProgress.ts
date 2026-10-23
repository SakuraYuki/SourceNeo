import { useEffect, useState } from "react";
import { api } from "../api";
import type { OperationProgress } from "../types";
export const stageLabels: Record<string, string> = {
  checking: "检查",
  fetching: "获取远端",
  integrating: "整合提交",
  pushing: "推送",
  cloning: "克隆",
};
export function useOperationProgress(enabled: boolean) {
  const [progress, setProgress] = useState<OperationProgress[]>([]);
  useEffect(() => {
    if (!enabled) {
      setProgress([]);
      return;
    }
    let active = true,
      pending = false;
    const poll = async () => {
      if (pending) return;
      pending = true;
      try {
        const result = await api.progress();
        if (active) setProgress(result);
      } catch {
        /* A progress read must never alter an in-flight Git operation. */
      } finally {
        pending = false;
      }
    };
    void poll();
    const timer = setInterval(() => void poll(), 1000);
    return () => {
      active = false;
      clearInterval(timer);
    };
  }, [enabled]);
  return progress;
}
