// @vitest-environment jsdom
import { afterEach, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { WorkingDiff } from "./WorkingDiff";
const workingDiff = vi.hoisted(() => vi.fn());
vi.mock("./api", () => ({ api: { workingDiff } }));
afterEach(() => {
  cleanup();
  vi.resetAllMocks();
});
it("keeps the new repository and file when an old preview arrives late", async () => {
  let finish!: (value: string) => void;
  workingDiff
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    )
    .mockResolvedValueOnce("new repository content");
  const view = render(
    <WorkingDiff
      path="old-repo"
      file={{ path: "old-file", oldPath: null, status: "??" }}
    />,
  );
  view.rerender(
    <WorkingDiff
      path="new-repo"
      file={{ path: "new-file", oldPath: null, status: "??" }}
    />,
  );
  await screen.findByText("new repository content");
  await act(async () => finish("stale old file"));
  expect(screen.queryByText("stale old file")).toBeNull();
  expect(screen.getByText("new repository content")).toBeTruthy();
});
it("switches index/worktree and ignores late responses, retries failures", async () => {
  let finish!: (value: string) => void;
  workingDiff
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    )
    .mockRejectedValueOnce({ message: "changed externally" })
    .mockResolvedValueOnce("+worktree");
  const view = render(
    <WorkingDiff
      path="repo"
      file={{ path: "中文 &.txt", oldPath: null, status: "MM" }}
    />,
  );
  fireEvent.click(screen.getByRole("button", { name: /未暂存/ }));
  await screen.findByText("changed externally");
  await act(async () => finish("old staged response"));
  expect(screen.queryByText("old staged response")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "重试 diff" }));
  await screen.findByText("+worktree");
  expect(workingDiff).toHaveBeenLastCalledWith(
    "repo",
    "中文 &.txt",
    "unstaged",
  );
  view.unmount();
});
it("ignores late preview responses after closing", async () => {
  let finish!: (value: string) => void;
  workingDiff.mockImplementation(
    () =>
      new Promise((resolve) => {
        finish = resolve;
      }),
  );
  const view = render(
    <WorkingDiff
      path="repo"
      file={{ path: "file", oldPath: null, status: "??" }}
    />,
  );
  expect(workingDiff).toHaveBeenCalledWith("repo", "file", "untracked");
  view.unmount();
  await act(async () => finish("late content"));
  expect(screen.queryByText("late content")).toBeNull();
});
