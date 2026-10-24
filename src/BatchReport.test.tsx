// @vitest-environment jsdom
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  within,
} from "@testing-library/react";
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import { BatchReport, type BatchProgress } from "./BatchReport";
import { repo } from "./test/fixtures";

const active: BatchProgress = {
  label: "批量同步",
  total: 1,
  running: true,
  results: [],
  targets: [repo],
  fetchOnly: false,
  startedAt: 100,
  stopped: false,
};
const done: BatchProgress = {
  ...active,
  running: false,
  results: [{ repo, outcome: "success", message: "已同步" }],
};
function report(batch: BatchProgress | null) {
  return (
    <BatchReport
      batch={batch}
      onStop={vi.fn()}
      onRetry={vi.fn()}
      progress={[]}
    />
  );
}
beforeEach(() => vi.useFakeTimers());
afterEach(() => {
  cleanup();
  vi.useRealTimers();
});

it("hides the empty task entry and opens on a new batch, without reopening on progress updates", () => {
  const { rerender } = render(report(null));
  expect(screen.queryByRole("button", { name: "任务状态" })).toBeNull();
  rerender(report(active));
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "收起批量任务状态" }));
  rerender(report({ ...active, stopped: true }));
  expect(screen.queryByRole("region", { name: "批量任务状态" })).toBeNull();
  rerender(report({ ...active, startedAt: 200 }));
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
});

it("collapses an automatically opened panel with Escape while focus stays on the original action", () => {
  const view = (batch: BatchProgress | null) => (
    <>
      <button>同步全部</button>
      {report(batch)}
    </>
  );
  const { rerender, unmount } = render(view(null));
  const origin = screen.getByRole("button", { name: "同步全部" });
  origin.focus();
  rerender(view(active));
  expect(document.activeElement).toBe(origin);
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();

  fireEvent.keyDown(document.activeElement!, { key: "Escape" });
  expect(screen.queryByRole("region", { name: "批量任务状态" })).toBeNull();
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "批量同步进行中，0/1，展开" }),
  );
  rerender(view({ ...active, stopped: true }));
  expect(screen.queryByRole("region", { name: "批量任务状态" })).toBeNull();

  origin.focus();
  expect(fireEvent.keyDown(origin, { key: "Escape" })).toBe(true);
  expect(document.activeElement).toBe(origin);
  rerender(view({ ...active, startedAt: 200 }));
  unmount();
  expect(fireEvent.keyDown(window, { key: "Escape" })).toBe(true);
});

it("leaves Escape to an open dialog instead of collapsing the task panel behind it", () => {
  render(
    <>
      {report(active)}
      <dialog open aria-label="其他操作">
        <button>关闭操作</button>
      </dialog>
    </>,
  );
  const close = screen.getByRole("button", { name: "关闭操作" });
  close.focus();
  expect(fireEvent.keyDown(close, { key: "Escape" })).toBe(true);
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
  expect(document.activeElement).toBe(close);
});

it("respects Escape already handled by another control", () => {
  render(
    <>
      <input
        aria-label="其他输入"
        onKeyDown={(event) => event.preventDefault()}
      />
      {report(active)}
    </>,
  );
  const input = screen.getByRole("textbox", { name: "其他输入" });
  input.focus();
  expect(fireEvent.keyDown(input, { key: "Escape" })).toBe(false);
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
  expect(document.activeElement).toBe(input);
});

it("collapses five seconds after all success and preserves results when reopened", () => {
  const { rerender } = render(report(active));
  rerender(report(done));
  act(() => vi.advanceTimersByTime(4999));
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
  act(() => vi.advanceTimersByTime(1));
  expect(screen.queryByRole("region", { name: "批量任务状态" })).toBeNull();
  fireEvent.click(
    screen.getByRole("button", { name: "批量同步结果，1/1，展开" }),
  );
  expect(screen.getByText(/成功：已同步/)).toBeTruthy();
  act(() => vi.advanceTimersByTime(10000));
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
});

it("shows the operation and progress while running, then a compact completion label", () => {
  const fetching = { ...active, label: "批量抓取", fetchOnly: true };
  const { rerender } = render(report(fetching));
  expect(screen.getByText("抓取 0/1")).toBeTruthy();
  rerender(report({ ...done, label: "批量抓取", fetchOnly: true }));
  expect(screen.getByText("抓取完成")).toBeTruthy();
  expect(screen.queryByText("已完成 1/1")).toBeNull();
  rerender(
    report({ ...done, results: [{ repo, outcome: "error", message: "失败" }] }),
  );
  expect(screen.getByText("同步 1 项失败")).toBeTruthy();
});

it.each(["error", "attention", "skipped"] as const)(
  "keeps %s results available until manually collapsed",
  (outcome) => {
    render(
      report({ ...done, results: [{ repo, outcome, message: "需要处理" }] }),
    );
    act(() => vi.advanceTimersByTime(10000));
    expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
    fireEvent.keyDown(screen.getByRole("region", { name: "批量任务状态" }), {
      key: "Escape",
    });
    expect(screen.queryByRole("region", { name: "批量任务状态" })).toBeNull();
  },
);

it("clears the previous completion timer when another batch starts", () => {
  const { rerender } = render(report(done));
  act(() => vi.advanceTimersByTime(4000));
  rerender(report({ ...active, startedAt: 200 }));
  act(() => vi.advanceTimersByTime(2000));
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
});

it("stops queued tasks only after confirmation and lets Escape dismiss only the confirmation", () => {
  const onStop = vi.fn();
  render(
    <BatchReport
      batch={active}
      onStop={onStop}
      onRetry={vi.fn()}
      progress={[]}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "停止排队" }));
  let confirmation = screen.getByRole("dialog", { name: "停止排队确认" });
  expect(onStop).not.toHaveBeenCalled();
  expect(document.activeElement).toBe(
    within(confirmation).getByRole("button", { name: "取消" }),
  );
  fireEvent.keyDown(document.activeElement!, { key: "Escape" });
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "停止排队" }),
  );
  fireEvent.keyDown(document.activeElement!, { key: "Escape" });
  expect(screen.queryByRole("region", { name: "批量任务状态" })).toBeNull();
  expect(onStop).not.toHaveBeenCalled();
  expect(document.activeElement).toBe(
    screen.getByRole("button", { name: "批量同步进行中，0/1，展开" }),
  );
  fireEvent.click(document.activeElement!);
  fireEvent.click(screen.getByRole("button", { name: "停止排队" }));
  confirmation = screen.getByRole("dialog", { name: "停止排队确认" });
  fireEvent.click(
    within(confirmation).getByRole("button", { name: "停止排队" }),
  );
  expect(onStop).toHaveBeenCalledTimes(1);
  expect(screen.queryByRole("dialog")).toBeNull();
});

it("discards stop confirmations when the panel closes or the task completes", () => {
  const { rerender } = render(report(active));
  fireEvent.click(screen.getByRole("button", { name: "停止排队" }));
  fireEvent.click(screen.getByRole("button", { name: "收起批量任务状态" }));
  fireEvent.click(
    screen.getByRole("button", { name: "批量同步进行中，0/1，展开" }),
  );
  expect(screen.queryByRole("dialog")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "停止排队" }));
  rerender(report(done));
  expect(screen.queryByRole("dialog")).toBeNull();
  rerender(report({ ...active, startedAt: 200 }));
  expect(screen.queryByRole("dialog")).toBeNull();
});

it("dismisses a confirmation on an outside click without stopping tasks", () => {
  const onStop = vi.fn();
  render(
    <BatchReport
      batch={active}
      onStop={onStop}
      onRetry={vi.fn()}
      progress={[]}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: "停止排队" }));
  fireEvent.pointerDown(screen.getByRole("heading", { name: "批量同步" }));
  expect(screen.queryByRole("dialog")).toBeNull();
  expect(screen.getByRole("region", { name: "批量任务状态" })).toBeTruthy();
  expect(onStop).not.toHaveBeenCalled();
});
