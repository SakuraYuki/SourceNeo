// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { CommitDetails } from "./CommitDetails";
import { makeHistory } from "./test/fixtures";

const api = vi.hoisted(() => ({
  changes: vi.fn(),
  diff: vi.fn(),
  copy: vi.fn(),
}));
vi.mock("./api", () => ({ api }));
const commit = makeHistory(1).commits[0];
const changes = {
  hash: commit.hash,
  parent: "parent",
  files: [
    { path: "new 文件.txt", oldPath: "old.txt", status: "R100" },
    { path: "second.txt", oldPath: null, status: "M" },
  ],
};
beforeEach(() => {
  vi.resetAllMocks();
  api.changes.mockResolvedValue(changes);
  api.copy.mockResolvedValue(undefined);
});
afterEach(cleanup);

it("lists renamed paths and ignores a late diff response after choosing another file", async () => {
  let finishOld!: (patch: string) => void;
  api.diff
    .mockImplementationOnce(
      () =>
        new Promise<string>((resolve) => {
          finishOld = resolve;
        }),
    )
    .mockResolvedValueOnce("+second change");
  render(<CommitDetails repoPath="/repo" commit={commit} onClose={vi.fn()} />);
  const first = await screen.findByRole("button", {
    name: "R old.txt → new 文件.txt",
  });
  expect(first.getAttribute("aria-pressed")).toBe("true");
  await vi.waitFor(() =>
    expect(api.diff).toHaveBeenCalledWith("/repo", commit.hash, "new 文件.txt"),
  );
  fireEvent.click(screen.getByRole("button", { name: "M second.txt" }));
  await screen.findByText("+second change");
  await act(async () => finishOld("+stale diff must not replace selection"));
  expect(
    screen.queryByText("+stale diff must not replace selection"),
  ).toBeNull();
  expect(screen.getByText("+second change")).toBeTruthy();
});

it("shows loading failures with retries for both the file list and diff", async () => {
  api.changes
    .mockRejectedValueOnce({ message: "读取文件失败" })
    .mockResolvedValue(changes);
  api.diff
    .mockRejectedValueOnce({ message: "读取差异失败" })
    .mockResolvedValue("+retry succeeds");
  render(<CommitDetails repoPath="/repo" commit={commit} onClose={vi.fn()} />);
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    "读取文件失败",
  );
  fireEvent.click(screen.getByRole("button", { name: "重试文件列表" }));
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    "读取差异失败",
  );
  fireEvent.click(screen.getByRole("button", { name: "重试 diff" }));
  await screen.findByText("+retry succeeds");
});

it("does not restore an obsolete commit's file list and handles empty commits", async () => {
  let finishOld!: (value: typeof changes) => void;
  api.changes
    .mockImplementationOnce(
      () =>
        new Promise<typeof changes>((resolve) => {
          finishOld = resolve;
        }),
    )
    .mockResolvedValueOnce({ hash: "new", parent: null, files: [] });
  const view = render(
    <CommitDetails
      key="old"
      repoPath="/repo"
      commit={commit}
      onClose={vi.fn()}
    />,
  );
  view.rerender(
    <CommitDetails
      key="new"
      repoPath="/other"
      commit={{ ...commit, hash: "new" }}
      onClose={vi.fn()}
    />,
  );
  await screen.findByText("此提交没有文件变更");
  await act(async () => finishOld(changes));
  expect(screen.queryByRole("button", { name: /new 文件/ })).toBeNull();
  expect(api.diff).not.toHaveBeenCalled();
});

it("labels first-parent comparison and caps large diff previews", async () => {
  api.diff.mockResolvedValue(
    Array.from({ length: 2002 }, (_, n) => `+line ${n}`).join("\n"),
  );
  render(
    <CommitDetails
      repoPath="/repo"
      commit={{ ...commit, parents: ["one", "two"] }}
      onClose={vi.fn()}
    />,
  );
  await screen.findByText(/仅预览前 2,000 行/);
  expect(screen.getByText(/合并提交 · 与第一父提交比较/)).toBeTruthy();
  expect(screen.getByText("+line 1999")).toBeTruthy();
  expect(screen.queryByText("+line 2000")).toBeNull();
});

it("copies the inspected commit SHA and can collapse details", async () => {
  api.diff.mockResolvedValue("+change");
  const onClose = vi.fn();
  render(<CommitDetails repoPath="/repo" commit={commit} onClose={onClose} />);
  await screen.findByText("+change");
  fireEvent.click(screen.getByRole("button", { name: "复制提交 SHA" }));
  await vi.waitFor(() =>
    expect(api.copy).toHaveBeenCalledExactlyOnceWith(commit.hash),
  );
  fireEvent.click(screen.getByRole("button", { name: "收起提交详情" }));
  expect(onClose).toHaveBeenCalledOnce();
});
