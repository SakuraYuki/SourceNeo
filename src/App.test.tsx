// @vitest-environment jsdom
import { afterEach, beforeEach, describe, expect, it, vi } from "vitest";
import {
  act,
  cleanup,
  fireEvent,
  render,
  screen,
  waitFor,
  within,
} from "@testing-library/react";
import App from "./App";
import { installDomMocks, makeHistory, repo, status } from "./test/fixtures";
import type { History } from "./types";

const api = vi.hoisted(() => ({
  theme: vi.fn(),
  list: vi.fn(),
  add: vi.fn(),
  remove: vi.fn(),
  status: vi.fn(),
  history: vi.fn(),
  sync: vi.fn(),
  fetch: vi.fn(),
  remotes: vi.fn(),
  openTool: vi.fn(),
  toolSettings: vi.fn(),
  saveToolSettings: vi.fn(),
  branches: vi.fn(),
  switchBranch: vi.fn(),
  workingChanges: vi.fn(),
  logs: vi.fn(),
  diagnostic: vi.fn(),
  copy: vi.fn(),
  pick: vi.fn(),
}));
vi.mock("./api", async (original) => ({
  ...(await original<typeof import("./api")>()),
  api,
  desktop: true,
}));

function menuAction(label: string, name = repo.name) {
  fireEvent.click(screen.getByRole("button", { name: `更多操作 ${name}` }));
  fireEvent.click(screen.getByRole("menuitem", { name: label }));
}
function removeFromMenu(name: string) {
  menuAction("从列表移除…", name);
}

beforeEach(() => {
  vi.resetAllMocks();
  installDomMocks();
  localStorage.clear();
  localStorage.setItem("sourceneo.theme", "light");
  api.theme.mockResolvedValue(undefined);
  api.list.mockResolvedValue([repo]);
  api.status.mockResolvedValue(status);
  api.history.mockImplementation(async (_path: string, count: number) =>
    makeHistory(count),
  );
  api.sync.mockResolvedValue(status);
  api.logs.mockResolvedValue([]);
  api.branches.mockResolvedValue([
    {
      reference: "refs/heads/feature",
      name: "feature",
      remote: false,
      current: false,
    },
  ]);
});
afterEach(() => {
  cleanup();
  vi.unstubAllGlobals();
});

describe("repository dialogs", () => {
  it("refreshes the selected linked worktree history after fetching another linked worktree", async () => {
    const linked = { ...repo, path: "/test/linked", name: "linked" };
    api.list.mockResolvedValue([repo, linked]);
    api.fetch.mockResolvedValue({ ...status, path: linked.path });
    render(<App />);
    await screen.findByText("100+");
    api.history.mockClear();
    menuAction("Fetch 此仓库", linked.name);
    await waitFor(() =>
      expect(api.history).toHaveBeenCalledWith(repo.path, 100),
    );
  });
  it("ignores a late remote selector after switching repositories", async () => {
    const other = {
      ...repo,
      path: "/test/other",
      name: "other",
      commonDir: "/test/other/.git",
    };
    let finish!: (value: string[]) => void;
    api.list.mockResolvedValue([repo, other]);
    api.remotes.mockImplementation(
      () =>
        new Promise((resolve) => {
          finish = resolve;
        }),
    );
    render(<App />);
    await screen.findByText("100+");
    menuAction("选择获取远端…");
    fireEvent.click(screen.getByRole("button", { name: /查看仓库 other/ }));
    await act(async () => finish(["origin"]));
    expect(screen.queryByRole("dialog", { name: "选择获取远端" })).toBeNull();
  });
  it("targets the right-clicked repository without changing the selected history", async () => {
    const other = {
      ...repo,
      path: "/other & 中文",
      name: "Other",
      commonDir: "/other/.git",
    };
    api.list.mockResolvedValue([repo, other]);
    api.copy.mockResolvedValue(undefined);
    api.remotes.mockResolvedValue(["origin", "backup"]);
    api.fetch.mockResolvedValue({ ...status, path: other.path });
    render(<App />);
    await screen.findByText("100+");
    const row = screen.getByRole("button", { name: /查看仓库 Other/ });
    fireEvent.contextMenu(row, { clientX: 500, clientY: 250 });
    expect(screen.getByRole("menu", { name: "仓库操作 · Other" })).toBeTruthy();
    fireEvent.click(screen.getByRole("menuitem", { name: "复制仓库路径" }));
    expect(api.copy).toHaveBeenCalledWith(other.path);
    expect(screen.queryByRole("menu")).toBeNull();
    expect(api.history).toHaveBeenLastCalledWith(repo.path, 100);
    fireEvent.keyDown(row, { key: "F10", shiftKey: true });
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(document.activeElement).toBe(row);
    fireEvent.contextMenu(row, { clientX: 500, clientY: 250 });
    fireEvent.click(screen.getByRole("menuitem", { name: "选择获取远端…" }));
    fireEvent.click(await screen.findByRole("button", { name: "backup" }));
    await waitFor(() =>
      expect(api.fetch).toHaveBeenCalledWith(other.path, "backup"),
    );
    expect(api.history).toHaveBeenLastCalledWith(repo.path, 100);
  });
  it("uses the details menu and disables Git actions for an operating repository", async () => {
    api.fetch.mockReturnValue(new Promise(() => {}));
    api.toolSettings.mockResolvedValue({ terminal: "", editor: "" });
    render(<App />);
    await screen.findByText("100+");
    expect(screen.queryByRole("button", { name: "在编辑器打开" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "Fetch 此仓库" }));
    fireEvent.click(screen.getByRole("button", { name: "当前仓库更多操作" }));
    const trigger = screen.getByRole("button", { name: "当前仓库更多操作" });
    fireEvent.pointerDown(trigger);
    fireEvent.click(trigger);
    expect(screen.queryByRole("menu")).toBeNull();
    fireEvent.click(trigger);
    expect(
      (
        screen.getByRole("menuitem", {
          name: "同步此仓库",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (
        screen.getByRole("menuitem", {
          name: "从列表移除…",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (
        screen.getByRole("menuitem", {
          name: "在终端打开",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(false);
    fireEvent.click(screen.getByRole("menuitem", { name: "外部工具设置" }));
    expect(screen.queryByRole("menu")).toBeNull();
    await screen.findByLabelText("编辑器可执行文件");
  });
  it("fetches all with failure isolation and never calls sync", async () => {
    const other = {
      ...repo,
      path: "/test/other",
      name: "other",
      commonDir: "/test/other/.git",
    };
    api.list.mockResolvedValue([repo, other]);
    api.fetch.mockImplementation(async (path: string) => {
      if (path === repo.path) throw { message: "fetch failed" };
      return { ...status, path, lastFetched: 20 };
    });
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("button", { name: "Fetch 全部" }));
    await waitFor(() => expect(api.fetch).toHaveBeenCalledTimes(2));
    expect(api.sync).not.toHaveBeenCalled();
    await screen.findByText(/test-repo：fetch failed/);
  });
  it("selects a named fetch remote and retries tool errors", async () => {
    api.remotes.mockResolvedValue(["origin", "backup"]);
    api.fetch.mockResolvedValue({ ...status, lastFetched: 20 });
    api.openTool
      .mockRejectedValueOnce({ message: "missing editor" })
      .mockResolvedValueOnce(undefined);
    render(<App />);
    await screen.findByText("100+");
    menuAction("选择获取远端…");
    fireEvent.click(await screen.findByRole("button", { name: "backup" }));
    await waitFor(() =>
      expect(api.fetch).toHaveBeenCalledWith(repo.path, "backup"),
    );
    menuAction("在编辑器打开");
    await screen.findByText(/missing editor/);
    menuAction("在编辑器打开");
    await waitFor(() => expect(api.openTool).toHaveBeenCalledTimes(2));
  });
  it("refreshes actual branch and history after a switch reports a hook failure", async () => {
    api.switchBranch.mockImplementation(async () => {
      api.status.mockResolvedValue({
        ...status,
        branch: "feature",
        head: "feature-head",
      });
      api.history.mockResolvedValue(makeHistory(1));
      api.branches.mockResolvedValue([
        {
          reference: "refs/heads/feature",
          name: "feature",
          remote: false,
          current: true,
        },
      ]);
      throw { message: "Switched to branch 'feature' · post-checkout failed" };
    });
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("button", { name: "切换分支" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "feature 切换" }),
    );
    expect(await screen.findByRole("alert")).toHaveProperty(
      "textContent",
      "Switched to branch 'feature' · post-checkout failed",
    );
    expect(
      screen.getByRole("button", { name: "切换分支" }).textContent,
    ).toContain("feature");
    expect(
      (
        (await screen.findByRole("button", {
          name: "feature 当前",
        })) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    await waitFor(() => expect(screen.queryByText("100+")).toBeNull());
    expect(api.history).toHaveBeenLastCalledWith(repo.path, 100);
  });
  it("opens the attention summary and locates the repository even when search hides it", async () => {
    api.status.mockResolvedValue({ ...status, kind: "attention", changed: 18 });
    render(<App />);
    await screen.findByText("18 项未提交");
    expect(screen.queryByText("18 M")).toBeNull();
    fireEvent.change(screen.getByRole("textbox", { name: "搜索仓库" }), {
      target: { value: "no-matches" },
    });
    fireEvent.click(
      screen.getByRole("button", { name: "查看 1 个需关注仓库" }),
    );
    const dialog = screen.getByRole("dialog", { name: "需关注详情" });
    expect(within(dialog).getByText(/有 18 个文件存在未提交改动/)).toBeTruthy();
    fireEvent.click(within(dialog).getByRole("button", { name: "定位仓库" }));
    expect(screen.queryByRole("dialog", { name: "需关注详情" })).toBeNull();
    expect(screen.getByRole("textbox", { name: "搜索仓库" })).toHaveProperty(
      "value",
      "",
    );
    expect(
      screen
        .getByRole("button", { name: /查看仓库 test-repo/ })
        .getAttribute("aria-pressed"),
    ).toBe("true");
  });
  it("opens the selected repository's actual uncommitted files", async () => {
    api.status.mockResolvedValue({ ...status, kind: "attention", changed: 1 });
    api.workingChanges.mockResolvedValue([
      { path: "src/changed.ts", oldPath: null, status: " M" },
    ]);
    render(<App />);
    fireEvent.click(
      await screen.findByRole("button", { name: "查看未提交文件" }),
    );
    await screen.findByText("src/changed.ts");
    expect(screen.getByText("未暂存")).toBeTruthy();
    expect(api.workingChanges).toHaveBeenCalledExactlyOnceWith(repo.path);
  });
  it("switches branches and refreshes status and history from the first page", async () => {
    api.switchBranch.mockResolvedValue({
      ...status,
      branch: "feature",
      head: "feature-head",
    });
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("button", { name: "切换分支" }));
    fireEvent.click(
      await screen.findByRole("button", { name: "feature 切换" }),
    );
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "切换分支" })).toBeNull(),
    );
    expect(api.switchBranch).toHaveBeenCalledExactlyOnceWith(
      repo.path,
      "refs/heads/feature",
      false,
    );
    expect(
      screen.getByRole("button", { name: "切换分支" }).textContent,
    ).toContain("feature");
    expect(api.history).toHaveBeenLastCalledWith(repo.path, 100);
  });
  it("shows add failures inside the open dialog and permits correcting the path", async () => {
    api.add.mockRejectedValueOnce({ message: "找不到 Git 仓库" });
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("button", { name: "手动输入仓库路径" }));
    const dialog = screen.getByRole("dialog", { name: "添加本地仓库" });
    const field = within(dialog).getByRole("textbox", { name: "仓库完整路径" });
    fireEvent.change(field, { target: { value: "/invalid" } });
    fireEvent.submit(dialog.querySelector("form")!);
    expect((await within(dialog).findByRole("alert")).textContent).toContain(
      "找不到 Git 仓库",
    );
    expect((field as HTMLInputElement).value).toBe("/invalid");
    expect(dialog.hasAttribute("open")).toBe(true);
    api.add.mockResolvedValueOnce(repo);
    fireEvent.change(field, { target: { value: repo.path } });
    fireEvent.submit(dialog.querySelector("form")!);
    await waitFor(() => expect(dialog.isConnected).toBe(false));
    expect(api.add).toHaveBeenLastCalledWith(repo.path);
    expect(screen.queryByRole("alert")).toBeNull();
  });

  it("shows remove failures inside the dialog and clears them on reopening", async () => {
    api.remove.mockRejectedValueOnce({ message: "无法保存仓库列表" });
    render(<App />);
    await screen.findByText("100+");
    removeFromMenu(repo.name);
    const dialog = document.querySelector("dialog")!;
    fireEvent.click(within(dialog).getByRole("button", { name: "从列表移除" }));
    expect((await within(dialog).findByRole("alert")).textContent).toContain(
      "无法保存仓库列表",
    );
    expect(dialog.hasAttribute("open")).toBe(true);
    fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
    removeFromMenu(repo.name);
    expect(
      within(document.querySelector("dialog")!).queryByRole("alert"),
    ).toBeNull();
    api.remove.mockResolvedValueOnce(undefined);
    api.list.mockResolvedValueOnce([]);
    fireEvent.click(screen.getByRole("button", { name: "从列表移除" }));
    await waitFor(() => expect(document.querySelector("dialog")).toBeNull());
    expect(screen.getByText("把你的仓库放在一起")).toBeTruthy();
  });
});

it.each(["success", "failure"])(
  "resets pagination after a %s sync with unchanged HEAD",
  async (result) => {
    render(<App />);
    await screen.findByText("100+");
    for (const count of [200, 300, 400, 500]) {
      fireEvent.click(
        screen.getByRole("button", { name: "加载更早的 100 条提交" }),
      );
      await screen.findByText(`${count}+`);
    }
    if (result === "failure")
      api.sync.mockRejectedValueOnce({ message: "测试同步失败" });
    fireEvent.click(screen.getByRole("button", { name: "同步此仓库" }));
    await screen.findByText("100+");
    fireEvent.click(
      screen.getByRole("button", { name: "加载更早的 100 条提交" }),
    );
    await waitFor(() =>
      expect(api.history).toHaveBeenLastCalledWith(repo.path, 200),
    );
    await screen.findByText("200+");
  },
);

async function removeFinalRepo() {
  removeFromMenu(repo.name);
  api.remove.mockResolvedValueOnce(undefined);
  api.list.mockResolvedValueOnce([]);
  fireEvent.click(screen.getByRole("button", { name: "从列表移除" }));
  await screen.findByText("把你的仓库放在一起");
}

it("clears history errors after removing the final repository", async () => {
  api.history.mockRejectedValueOnce({ message: "历史读取失败" });
  render(<App />);
  await screen.findByText("历史读取失败");
  await removeFinalRepo();
  await waitFor(() => expect(screen.queryByText("历史读取失败")).toBeNull());
  expect(screen.queryByRole("button", { name: "重试加载" })).toBeNull();
  expect(screen.getByText("暂无提交历史")).toBeTruthy();
});

it.each(["resolve", "reject"])(
  "clears pending history when removing the final repository and ignores a late %s",
  async (result) => {
    let resolve!: (history: History) => void;
    let reject!: (error: unknown) => void;
    api.history.mockReturnValueOnce(
      new Promise<History>((onResolve, onReject) => {
        resolve = onResolve;
        reject = onReject;
      }),
    );
    render(<App />);
    await screen.findByText("正在读取提交历史…");
    await removeFinalRepo();
    await waitFor(() =>
      expect(screen.queryByText("正在读取提交历史…")).toBeNull(),
    );
    await act(async () => {
      if (result === "resolve") resolve(makeHistory(100));
      else reject({ message: "迟到的历史错误" });
    });
    expect(screen.getByText("暂无提交历史")).toBeTruthy();
    expect(screen.queryByText("Commit 0")).toBeNull();
    expect(screen.queryByText("迟到的历史错误")).toBeNull();
    expect(screen.queryByRole("button", { name: "重试加载" })).toBeNull();
  },
);

function deferred<T>() {
  let resolve!: (value: T) => void;
  let reject!: (reason: unknown) => void;
  const promise = new Promise<T>((yes, no) => {
    resolve = yes;
    reject = no;
  });
  return { promise, resolve, reject };
}

it.each(["success", "failure"])(
  "keeps a new add dialog and selection intact after an old %s",
  async (result) => {
    const pending = deferred<typeof repo>();
    const added = { ...repo, path: "/test/added", name: "added" };
    api.add.mockReturnValueOnce(pending.promise);
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("button", { name: "手动输入仓库路径" }));
    let dialog = document.querySelector("dialog")!;
    fireEvent.change(within(dialog).getByRole("textbox"), {
      target: { value: added.path },
    });
    fireEvent.submit(dialog.querySelector("form")!);
    fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
    fireEvent.click(screen.getByRole("button", { name: "手动输入仓库路径" }));
    dialog = document.querySelector("dialog")!;
    fireEvent.change(within(dialog).getByRole("textbox"), {
      target: { value: "/test/new-input" },
    });
    api.list.mockResolvedValue([repo, added]);
    await act(async () => {
      if (result === "success") pending.resolve(added);
      else pending.reject({ message: "旧添加请求失败" });
    });
    expect(dialog.isConnected).toBe(true);
    expect(
      (within(dialog).getByRole("textbox") as HTMLInputElement).value,
    ).toBe("/test/new-input");
    expect(within(dialog).queryByRole("alert")).toBeNull();
    expect(
      screen
        .getByRole("button", { name: /查看仓库 test-repo/ })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    if (result === "success")
      expect(
        screen.getByRole("button", { name: /查看仓库 added/ }),
      ).toBeTruthy();
  },
);

it.each(["success", "failure"])(
  "preserves the current selection and new remove dialog after an old %s",
  async (result) => {
    const b = { ...repo, path: "/test/b", name: "Repo B" };
    const c = { ...repo, path: "/test/c", name: "Repo C" };
    api.list.mockResolvedValueOnce([repo, b, c]);
    const pending = deferred<void>();
    api.remove.mockReturnValueOnce(pending.promise);
    render(<App />);
    await screen.findByText("100+");
    removeFromMenu(repo.name);
    let dialog = document.querySelector("dialog")!;
    fireEvent.click(within(dialog).getByRole("button", { name: "从列表移除" }));
    fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
    fireEvent.click(screen.getByRole("button", { name: /查看仓库 Repo C/ }));
    await waitFor(() =>
      expect(api.history).toHaveBeenLastCalledWith(c.path, 100),
    );
    removeFromMenu("Repo B");
    dialog = document.querySelector("dialog")!;
    api.list.mockResolvedValue([b, c]);
    await act(async () => {
      if (result === "success") pending.resolve();
      else pending.reject({ message: "旧移除请求失败" });
    });
    expect(dialog.isConnected).toBe(true);
    expect(within(dialog).getByText("移除 Repo B？")).toBeTruthy();
    expect(within(dialog).queryByRole("alert")).toBeNull();
    expect(
      screen
        .getByRole("button", { name: /查看仓库 Repo C/ })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    if (result === "success")
      expect(
        screen.queryByRole("button", { name: /查看仓库 test-repo/ }),
      ).toBeNull();
  },
);

it("prevents duplicate add submissions and editing while the request is pending", async () => {
  const pending = deferred<typeof repo>();
  api.add.mockReturnValue(pending.promise);
  render(<App />);
  await screen.findByText("100+");
  fireEvent.click(screen.getByRole("button", { name: "手动输入仓库路径" }));
  const dialog = document.querySelector("dialog")!;
  const input = within(dialog).getByRole("textbox") as HTMLInputElement;
  fireEvent.change(input, { target: { value: repo.path } });
  fireEvent.submit(dialog.querySelector("form")!);
  fireEvent.submit(dialog.querySelector("form")!);
  expect(api.add).toHaveBeenCalledTimes(1);
  expect(input.disabled).toBe(true);
  await act(async () => pending.reject({ message: "可以重试" }));
  expect(input.disabled).toBe(false);
  expect(within(dialog).getByRole("alert").textContent).toContain("可以重试");
});

it("prevents duplicate remove submissions while the request is pending", async () => {
  const pending = deferred<void>();
  api.remove.mockReturnValue(pending.promise);
  render(<App />);
  await screen.findByText("100+");
  removeFromMenu(repo.name);
  const button = within(document.querySelector("dialog")!).getByRole("button", {
    name: "从列表移除",
  }) as HTMLButtonElement;
  fireEvent.click(button);
  fireEvent.click(button);
  expect(api.remove).toHaveBeenCalledTimes(1);
  expect(button.disabled).toBe(true);
  await act(async () => pending.reject({ message: "可以重试" }));
  expect(button.disabled).toBe(false);
});

it("ignores an old add-list snapshot after a newer removal", async () => {
  const added = { ...repo, path: "/test/added", name: "added" };
  const stale = deferred<(typeof repo)[]>();
  api.add.mockResolvedValueOnce(added);
  api.remove.mockResolvedValueOnce(undefined);
  api.list
    .mockResolvedValueOnce([repo])
    .mockReturnValueOnce(stale.promise)
    .mockResolvedValue([added]);
  render(<App />);
  await screen.findByText("100+");
  fireEvent.click(screen.getByRole("button", { name: "手动输入仓库路径" }));
  let dialog = document.querySelector("dialog")!;
  fireEvent.change(within(dialog).getByRole("textbox"), {
    target: { value: added.path },
  });
  fireEvent.submit(dialog.querySelector("form")!);
  await waitFor(() => expect(api.list).toHaveBeenCalledTimes(2));
  fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
  removeFromMenu(repo.name);
  dialog = document.querySelector("dialog")!;
  fireEvent.click(within(dialog).getByRole("button", { name: "从列表移除" }));
  await waitFor(() =>
    expect(
      screen.queryByRole("button", { name: /查看仓库 test-repo/ }),
    ).toBeNull(),
  );
  await act(async () => stale.resolve([repo, added]));
  expect(
    screen.queryByRole("button", { name: /查看仓库 test-repo/ }),
  ).toBeNull();
  expect(
    screen
      .getByRole("button", { name: /查看仓库 added/ })
      .getAttribute("aria-pressed"),
  ).toBe("true");
});

it("ignores an old remove-list snapshot after a newer addition", async () => {
  const kept = { ...repo, path: "/test/kept", name: "kept" };
  const added = { ...repo, path: "/test/added", name: "added" };
  const stale = deferred<(typeof repo)[]>();
  api.remove.mockResolvedValueOnce(undefined);
  api.add.mockResolvedValueOnce(added);
  api.list
    .mockResolvedValueOnce([repo, kept])
    .mockReturnValueOnce(stale.promise)
    .mockResolvedValue([kept, added]);
  render(<App />);
  await screen.findByText("100+");
  removeFromMenu(repo.name);
  let dialog = document.querySelector("dialog")!;
  fireEvent.click(within(dialog).getByRole("button", { name: "从列表移除" }));
  await waitFor(() => expect(api.list).toHaveBeenCalledTimes(2));
  fireEvent.click(within(dialog).getByRole("button", { name: "取消" }));
  fireEvent.click(screen.getByRole("button", { name: "手动输入仓库路径" }));
  dialog = document.querySelector("dialog")!;
  fireEvent.change(within(dialog).getByRole("textbox"), {
    target: { value: added.path },
  });
  fireEvent.submit(dialog.querySelector("form")!);
  await waitFor(() =>
    expect(
      screen
        .getByRole("button", { name: /查看仓库 added/ })
        .getAttribute("aria-pressed"),
    ).toBe("true"),
  );
  await act(async () => stale.resolve([kept]));
  expect(
    screen
      .getByRole("button", { name: /查看仓库 added/ })
      .getAttribute("aria-pressed"),
  ).toBe("true");
  expect(
    screen.queryByRole("button", { name: /查看仓库 test-repo/ }),
  ).toBeNull();
});

it.each(["success", "failure"])(
  "ignores a late startup list %s after adding a repository",
  async (result) => {
    const startup = deferred<(typeof repo)[]>();
    api.list.mockReturnValueOnce(startup.promise).mockResolvedValue([repo]);
    api.add.mockResolvedValueOnce(repo);
    render(<App />);
    fireEvent.click(screen.getByRole("button", { name: "手动输入仓库路径" }));
    const dialog = document.querySelector("dialog")!;
    fireEvent.change(within(dialog).getByRole("textbox"), {
      target: { value: repo.path },
    });
    fireEvent.submit(dialog.querySelector("form")!);
    await screen.findByText("100+");
    await act(async () => {
      if (result === "success") startup.resolve([]);
      else startup.reject({ message: "过期的启动列表错误" });
    });
    expect(
      screen
        .getByRole("button", { name: /查看仓库 test-repo/ })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(screen.queryByText("过期的启动列表错误")).toBeNull();
  },
);

it.each(["manual", "picker"])(
  "passes the original %s path to the backend before existence-based trimming",
  async (source) => {
    const path = `${repo.path} `;
    api.add.mockResolvedValueOnce(repo);
    render(<App />);
    await screen.findByText("100+");
    if (source === "picker") {
      api.pick.mockResolvedValueOnce(path);
      fireEvent.click(screen.getByRole("button", { name: "添加仓库" }));
    } else {
      fireEvent.click(screen.getByRole("button", { name: "手动输入仓库路径" }));
      const dialog = document.querySelector("dialog")!;
      fireEvent.change(within(dialog).getByRole("textbox"), {
        target: { value: path },
      });
      fireEvent.submit(dialog.querySelector("form")!);
    }
    await waitFor(() => expect(api.add).toHaveBeenCalledWith(path));
  },
);
