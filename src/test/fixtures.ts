import { vi } from "vitest";
import type { History, Repo, RepoStatus } from "../types";

export const repo: Repo = {
  path: "/test/repo",
  name: "test-repo",
  commonDir: "/test/repo/.git",
};
export const status: RepoStatus = {
  path: repo.path,
  branch: "main",
  head: "commit-0",
  upstream: "origin/main",
  ahead: 0,
  behind: 0,
  changed: 0,
  conflicts: [],
  operation: null,
  kind: "synced",
  message: "本地与已知远程一致",
  errorKind: null,
  lastFetched: null,
  checkedAt: 1,
  shallow: false,
};
export function makeHistory(count: number): History {
  return {
    shallow: false,
    hasMore: true,
    commits: Array.from({ length: count }, (_, i) => ({
      hash: `commit-${i}`,
      parents: [`commit-${i + 1}`],
      subject: `Commit ${i}`,
      author: "Test",
      date: "2026-10-20",
      refs: [],
    })),
  };
}

// JSDOM has no layout engine or native modal top layer.
export class ResizeObserverMock {
  static instances: ResizeObserverMock[] = [];
  observe = vi.fn();
  disconnect = vi.fn();
  constructor(private callback: ResizeObserverCallback) {
    ResizeObserverMock.instances.push(this);
  }
  resize(height: number) {
    this.callback(
      [{ contentRect: { height } } as ResizeObserverEntry],
      this as unknown as ResizeObserver,
    );
  }
}
export function installDomMocks() {
  ResizeObserverMock.instances = [];
  vi.stubGlobal("ResizeObserver", ResizeObserverMock);
  HTMLDialogElement.prototype.showModal = function () {
    this.open = true;
  };
  HTMLDialogElement.prototype.close = function () {
    this.open = false;
  };
}
