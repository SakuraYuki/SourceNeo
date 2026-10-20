// @vitest-environment jsdom
import { afterEach, beforeEach, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
} from "@testing-library/react";
import { BranchDialog } from "./BranchDialog";
import { installDomMocks, repo } from "./test/fixtures";

const branches = vi.hoisted(() => vi.fn());
vi.mock("./api", () => ({ api: { branches } }));
beforeEach(() => {
  vi.resetAllMocks();
  installDomMocks();
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

it("lists local and remote branches and keeps switch errors in the dialog for retry", async () => {
  branches.mockResolvedValue([
    {
      reference: "refs/heads/main",
      name: "main",
      remote: false,
      current: true,
    },
    {
      reference: "refs/remotes/origin/feature",
      name: "origin/feature",
      remote: true,
      current: false,
    },
  ]);
  const onSwitch = vi
    .fn()
    .mockRejectedValueOnce({ message: "存在未提交改动" })
    .mockResolvedValue(undefined);
  const onClose = vi.fn();
  render(<BranchDialog repo={repo} onClose={onClose} onSwitch={onSwitch} />);
  const current = await screen.findByRole("button", { name: "main 当前" });
  expect((current as HTMLButtonElement).disabled).toBe(true);
  fireEvent.click(
    screen.getByRole("button", { name: "origin/feature 创建并切换" }),
  );
  expect(await screen.findByRole("alert")).toHaveProperty(
    "textContent",
    "存在未提交改动",
  );
  expect(onClose).not.toHaveBeenCalled();
  fireEvent.click(
    screen.getByRole("button", { name: "origin/feature 创建并切换" }),
  );
  await vi.waitFor(() => expect(onClose).toHaveBeenCalledOnce());
  expect(onSwitch).toHaveBeenLastCalledWith(
    "refs/remotes/origin/feature",
    false,
  );
});

it("can retry loading and create a branch without duplicate submissions", async () => {
  branches
    .mockRejectedValueOnce({ message: "读取分支失败" })
    .mockResolvedValue([]);
  let resolve!: () => void;
  const onSwitch = vi.fn(
    () =>
      new Promise<void>((done) => {
        resolve = done;
      }),
  );
  const onClose = vi.fn();
  render(<BranchDialog repo={repo} onClose={onClose} onSwitch={onSwitch} />);
  await screen.findByRole("alert");
  fireEvent.click(screen.getByRole("button", { name: "重新加载" }));
  const input = await screen.findByLabelText("从当前 HEAD 新建分支");
  fireEvent.change(input, { target: { value: "feature/new" } });
  const button = screen.getByRole("button", { name: "新建并切换" });
  fireEvent.click(button);
  fireEvent.click(button);
  expect(onSwitch).toHaveBeenCalledExactlyOnceWith("feature/new", true);
  resolve();
  await vi.waitFor(() => expect(onClose).toHaveBeenCalledOnce());
});

it("preserves the switch error when refreshing the branch list also fails", async () => {
  branches
    .mockResolvedValueOnce([
      {
        reference: "refs/heads/feature",
        name: "feature",
        remote: false,
        current: false,
      },
    ])
    .mockRejectedValueOnce({ message: "读取失败" });
  render(
    <BranchDialog
      repo={repo}
      onClose={vi.fn()}
      onSwitch={vi.fn().mockRejectedValue({ message: "post-checkout failed" })}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "feature 切换" }));
  await vi.waitFor(() => {
    expect(screen.getByRole("alert").textContent).toContain(
      "post-checkout failed",
    );
    expect(screen.getByRole("alert").textContent).toContain(
      "分支列表读取失败：读取失败",
    );
  });
});

it("does not close a reopened dialog when an earlier switch finishes", async () => {
  branches.mockResolvedValue([
    {
      reference: "refs/heads/feature",
      name: "feature",
      remote: false,
      current: false,
    },
  ]);
  let finish!: () => void;
  const onSwitch = vi.fn(
    () =>
      new Promise<void>((resolve) => {
        finish = resolve;
      }),
  );
  const onClose = vi.fn();
  const view = render(
    <BranchDialog
      key="old"
      repo={repo}
      onClose={onClose}
      onSwitch={onSwitch}
    />,
  );
  fireEvent.click(await screen.findByRole("button", { name: "feature 切换" }));
  view.rerender(
    <BranchDialog
      key="new"
      repo={repo}
      onClose={onClose}
      onSwitch={onSwitch}
    />,
  );
  await screen.findByRole("button", { name: "feature 切换" });
  await act(async () => finish());
  expect(onClose).not.toHaveBeenCalled();
  expect(screen.getByRole("dialog", { name: "切换分支" })).toBeTruthy();
});
