// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { AttentionDialog } from "./AttentionDialog";
import { repo, status, installDomMocks } from "./test/fixtures";
const workingChanges = vi.hoisted(() => vi.fn());
vi.mock("./api", () => ({ api: { workingChanges } }));
beforeEach(() => {
  vi.resetAllMocks();
  installDomMocks();
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("shows why each repository needs attention, its next step and actual file names", async () => {
  workingChanges.mockResolvedValue([
    { path: "src/new.ts", oldPath: "src/old.ts", status: "RM" },
    { path: "notes.txt", oldPath: null, status: "??" },
  ]);
  const onSelect = vi.fn();
  render(
    <AttentionDialog
      repos={[repo]}
      statuses={{ [repo.path]: { ...status, kind: "attention", changed: 2 } }}
      failures={{}}
      onClose={vi.fn()}
      onSelect={onSelect}
    />,
  );
  expect(screen.getByText("2 项未提交")).toBeTruthy();
  expect(screen.getByText(/下一步：.*查看下面的文件清单/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "查看 2 个未提交文件" }));
  await screen.findByText("notes.txt");
  expect(screen.getByText("已暂存 / 未暂存")).toBeTruthy();
  expect(screen.getByText("未跟踪")).toBeTruthy();
  expect(screen.getByText("重命名")).toBeTruthy();
  expect(screen.getByText(/src\/old.ts →/)).toBeTruthy();
  fireEvent.click(screen.getByRole("button", { name: "定位仓库" }));
  expect(onSelect).toHaveBeenCalledExactlyOnceWith(repo);
});

it("retries reading errors and ignores a late response from a closed file list", async () => {
  let finish!: (files: unknown[]) => void;
  workingChanges
    .mockRejectedValueOnce({ message: "无法读取目录" })
    .mockImplementationOnce(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
  render(
    <AttentionDialog
      repos={[repo]}
      statuses={{ [repo.path]: { ...status, kind: "attention", changed: 1 } }}
      failures={{}}
      initialPath={repo.path}
      onClose={vi.fn()}
      onSelect={vi.fn()}
    />,
  );
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    "无法读取目录",
  );
  fireEvent.click(screen.getByRole("button", { name: "重试文件列表" }));
  fireEvent.click(screen.getByRole("button", { name: "收起文件清单" }));
  await act(async () =>
    finish([{ path: "stale.txt", oldPath: null, status: "??" }]),
  );
  expect(screen.queryByText("stale.txt")).toBeNull();
});

it("explains a clean repository's missing upstream without loading unrelated files", () => {
  render(
    <AttentionDialog
      repos={[repo]}
      statuses={{
        [repo.path]: {
          ...status,
          kind: "attention",
          upstream: null,
          message: "尚未设置 upstream",
        },
      }}
      failures={{}}
      initialPath={repo.path}
      onClose={vi.fn()}
      onSelect={vi.fn()}
    />,
  );
  expect(screen.getByRole("heading", { name: "仓库处理建议" })).toBeTruthy();
  expect(screen.getByText("未设上游")).toBeTruthy();
  expect(screen.getByText(/下一步：.*设置上游/)).toBeTruthy();
  expect(workingChanges).not.toHaveBeenCalled();
});
