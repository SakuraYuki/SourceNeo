// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  renderHook,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { AcquireDialog } from "./AcquireDialog";
import { SyncPreviewDialog } from "./SyncPreviewDialog";
import { HistorySearchDialog } from "./HistorySearchDialog";
import { useBatchOperations } from "./hooks/useBatchOperations";
import { useRepositoryHistory } from "./hooks/useRepositoryHistory";
import { useRepositoryStatus } from "./hooks/useRepositoryStatus";
import { installDomMocks, makeHistory, repo, status } from "./test/fixtures";
import type { BatchResult } from "./BatchReport";
const api = vi.hoisted(() => ({
  clone: vi.fn(),
  cancelClone: vi.fn(),
  history: vi.fn(),
  add: vi.fn(),
  scan: vi.fn(),
  pickFolder: vi.fn(),
  preview: vi.fn(),
  historyPage: vi.fn(),
  status: vi.fn(),
  copy: vi.fn(),
}));
vi.mock("./api", async (original) => ({
  ...(await original<typeof import("./api")>()),
  api,
}));
beforeEach(() => {
  installDomMocks();
  vi.resetAllMocks();
});
afterEach(() => {
  cleanup();
  vi.useRealTimers();
  vi.unstubAllGlobals();
});
function deferred<T>() {
  let resolve!: (value: T) => void;
  const promise = new Promise<T>((r) => {
    resolve = r;
  });
  return { promise, resolve };
}

it("clones once, waits for registration, and keeps errors in the dialog for correction", async () => {
  api.clone
    .mockRejectedValueOnce({ message: "目标已存在" })
    .mockResolvedValueOnce(repo);
  const added = vi.fn().mockResolvedValue(undefined),
    closed = vi.fn();
  render(<AcquireDialog mode="clone" onClose={closed} onAdded={added} />);
  fireEvent.change(screen.getByLabelText("远程仓库地址"), {
    target: { value: "https://host/team/repo.git" },
  });
  fireEvent.change(screen.getByLabelText("目标父目录"), {
    target: { value: "/tmp" },
  });
  fireEvent.click(screen.getByRole("button", { name: "克隆并添加" }));
  await screen.findByText("目标已存在");
  expect(closed).not.toHaveBeenCalled();
  expect(added).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("新文件夹名称"), {
    target: { value: "another" },
  });
  fireEvent.click(screen.getByRole("button", { name: "克隆并添加" }));
  await waitFor(() => expect(added).toHaveBeenCalledWith([repo]));
  expect(api.clone).toHaveBeenLastCalledWith(
    "https://host/team/repo.git",
    "/tmp",
    "another",
    expect.any(String),
  );
  expect(closed).toHaveBeenCalledOnce();
});

it("scans before importing and retains only failed selections for retry", async () => {
  const other = { ...repo, path: "/test/other", name: "other" };
  api.scan.mockResolvedValue({
    repos: [repo, other],
    warnings: [],
    truncated: false,
  });
  api.add.mockImplementation(async (path) => {
    if (path === other.path) throw new Error("权限不足");
    return repo;
  });
  const added = vi.fn().mockResolvedValue(undefined);
  render(<AcquireDialog mode="scan" onClose={vi.fn()} onAdded={added} />);
  fireEvent.change(screen.getByLabelText("目标父目录"), {
    target: { value: "/test" },
  });
  fireEvent.click(screen.getByRole("button", { name: "扫描目录" }));
  await screen.findByText("发现 2 个仓库");
  expect(api.add).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "导入所选（2）" }));
  await screen.findByText("other：权限不足");
  await waitFor(() => expect(added).toHaveBeenCalledWith([repo]));
  expect(screen.getByRole("button", { name: "导入所选（1）" })).toBeTruthy();
});

it("prevents executing a blocked preview", async () => {
  api.preview.mockResolvedValue({
    status,
    remote: "origin",
    policy: "rebase",
    steps: ["Fetch"],
    blockers: ["工作区不干净"],
  });
  const sync = vi.fn();
  render(
    <SyncPreviewDialog
      repo={repo}
      busy={false}
      onClose={vi.fn()}
      onSync={sync}
    />,
  );
  await screen.findByText("工作区不干净");
  fireEvent.click(screen.getByRole("button", { name: "执行同步" }));
  expect(sync).not.toHaveBeenCalled();
});

it("searches backend history rather than the visible commit page and continues its snapshot", async () => {
  api.historyPage
    .mockResolvedValueOnce({
      snapshot: "fixed",
      commits: [
        {
          hash: "abc",
          parents: [],
          subject: "old match",
          author: "tester",
          date: "2026-01-01",
          refs: [],
        },
      ],
      hasMore: true,
      shallow: false,
    })
    .mockResolvedValueOnce({
      snapshot: "fixed",
      commits: [],
      hasMore: false,
      shallow: false,
    });
  render(<HistorySearchDialog repo={repo} onClose={vi.fn()} />);
  fireEvent.change(screen.getByLabelText("搜索提交内容"), {
    target: { value: "old" },
  });
  fireEvent.click(screen.getByRole("button", { name: "搜索" }));
  await screen.findByText("old match");
  fireEvent.click(screen.getByRole("button", { name: "加载更多搜索结果" }));
  await waitFor(() =>
    expect(api.historyPage).toHaveBeenLastCalledWith(
      repo.path,
      true,
      { field: "message", text: "old" },
      "fixed",
      1,
    ),
  );
});

it("stops only queued batch work and retries failures with the original Fetch mode", async () => {
  const repos = [0, 1, 2, 3].map((i) => ({ ...repo, path: `/r${i}` }));
  const busy = { current: new Set<string>() },
    mounted = { current: true };
  const tasks = repos.map(() => deferred<BatchResult>());
  const run = vi.fn((r: typeof repo) => tasks[repos.indexOf(r)].promise);
  const { result } = renderHook(() =>
    useBatchOperations({ current: repos }, busy, mounted, vi.fn(), run),
  );
  let completion!: Promise<void>;
  act(() => {
    completion = result.current.runBatch(repos, true);
  });
  expect(run).toHaveBeenCalledTimes(3);
  act(() => result.current.stopBatch());
  await act(async () => {
    tasks[0].resolve({ repo: repos[0], outcome: "error", message: "network" });
    tasks[1].resolve({ repo: repos[1], outcome: "success", message: "ok" });
    tasks[2].resolve({ repo: repos[2], outcome: "success", message: "ok" });
    await completion;
  });
  expect(run).toHaveBeenCalledTimes(3);
  expect(
    result.current.batch?.results.find((r) => r.repo.path === "/r3")?.message,
  ).toBe("已停止排队");
  busy.current.clear();
  run.mockResolvedValue({ repo: repos[0], outcome: "success", message: "ok" });
  await act(async () => result.current.retryFailed());
  expect(run).toHaveBeenLastCalledWith(repos[0], true, undefined, true);
  expect(run).toHaveBeenCalledTimes(4);
});

it("backs off unchanged background repositories while refreshing the selected repository", async () => {
  vi.useFakeTimers();
  vi.setSystemTime(100_000);
  const other = { ...repo, path: "/other" };
  const repos = { current: [repo, other] },
    busy = { current: new Set<string>() },
    selected = { current: repo.path },
    mounted = { current: true };
  api.status.mockImplementation(async (path) => ({
    ...status,
    path,
    checkedAt: Date.now(),
  }));
  const { result } = renderHook(() =>
    useRepositoryStatus(repos, busy, selected, mounted),
  );
  await act(() => result.current.refresh(true));
  await act(async () => {
    vi.advanceTimersByTime(15_000);
    await result.current.refresh();
  });
  api.status.mockClear();
  await act(async () => {
    vi.advanceTimersByTime(15_000);
    await result.current.refresh();
  });
  expect(api.status).toHaveBeenCalledTimes(1);
  expect(api.status).toHaveBeenCalledWith(repo.path);
  await act(() => result.current.refresh(true)); // throttled only for one second after last refresh
  await act(async () => {
    vi.advanceTimersByTime(1000);
    await result.current.refresh(true);
  });
  expect(api.status).toHaveBeenCalledWith(other.path);
});

it("allows cancelling an in-flight clone without registering a failed repository", async () => {
  api.clone.mockImplementation(
    () =>
      new Promise((_resolve, reject) => {
        api.cancelClone.mockImplementation(async () => {
          reject({ message: "已取消克隆，目标目录保留" });
        });
      }),
  );
  const added = vi.fn();
  render(<AcquireDialog mode="clone" onClose={vi.fn()} onAdded={added} />);
  fireEvent.change(screen.getByLabelText("远程仓库地址"), {
    target: { value: "https://example.test/repo" },
  });
  fireEvent.change(screen.getByLabelText("目标父目录"), {
    target: { value: "/tmp" },
  });
  fireEvent.click(screen.getByRole("button", { name: "克隆并添加" }));
  fireEvent.click(await screen.findByRole("button", { name: "取消克隆" }));
  expect(api.cancelClone).not.toHaveBeenCalled();
  const confirmation = screen.getByRole("dialog", { name: "取消克隆确认" });
  fireEvent.click(within(confirmation).getByRole("button", { name: "取消" }));
  expect(api.cancelClone).not.toHaveBeenCalled();
  fireEvent.click(screen.getByRole("button", { name: "取消克隆" }));
  fireEvent.click(
    within(screen.getByRole("dialog", { name: "取消克隆确认" })).getByRole(
      "button",
      { name: "取消克隆" },
    ),
  );
  await screen.findByText("已取消克隆，目标目录保留");
  expect(api.cancelClone).toHaveBeenCalledWith(expect.any(String));
  expect(added).not.toHaveBeenCalled();
});

it("restores cached history and page depth when returning to an unchanged repository", async () => {
  api.history.mockImplementation(async (_path: string, count: number) =>
    makeHistory(count),
  );
  const { result, rerender } = renderHook(
    ({ path, fingerprint }) => useRepositoryHistory(path, false, fingerprint),
    { initialProps: { path: "/a", fingerprint: "a-head" } },
  );
  await waitFor(() =>
    expect(result.current.history?.commits).toHaveLength(100),
  );
  await act(async () => {
    result.current.setLimit(200);
    await result.current.loadHistory("/a", 200);
  });
  rerender({ path: "/b", fingerprint: "b-head" });
  await waitFor(() =>
    expect(result.current.history?.commits).toHaveLength(100),
  );
  api.history.mockClear();
  rerender({ path: "/a", fingerprint: "a-head" });
  await waitFor(() =>
    expect(result.current.history?.commits).toHaveLength(200),
  );
  expect(result.current.limit).toBe(200);
  expect(api.history).not.toHaveBeenCalled();
});
