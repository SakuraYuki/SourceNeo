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
import type { History, Repo, RepoStatus } from "./types";

const api = vi.hoisted(() => ({
  theme: vi.fn(),
  list: vi.fn(),
  groups: vi.fn(),
  editGroups: vi.fn(),
  updateRepoFlags: vi.fn(),
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
  preview: vi.fn(),
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
  api.groups.mockResolvedValue([]);
  api.status.mockResolvedValue(status);
  api.history.mockImplementation(async (_path: string, count: number) =>
    makeHistory(count),
  );
  api.sync.mockResolvedValue(status);
  api.preview.mockResolvedValue({
    status,
    remote: "origin",
    policy: "ff",
    steps: [],
    blockers: [],
  });
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

it("defaults to current history and resets pagination when toggling all branches", async () => {
  render(<App />);
  await screen.findByText("100+");
  const toggle = screen.getByRole("switch", { name: "显示全部分支" });
  expect(toggle.getAttribute("aria-checked")).toBe("false");
  expect(api.history).toHaveBeenLastCalledWith(repo.path, 100, false);
  fireEvent.click(
    screen.getByRole("button", { name: "加载更早的 100 条提交" }),
  );
  await screen.findByText("200+");
  fireEvent.click(toggle);
  await screen.findByText("100+");
  expect(toggle.getAttribute("aria-checked")).toBe("true");
  expect(api.history).toHaveBeenLastCalledWith(repo.path, 100, true);
  fireEvent.click(
    screen.getByRole("button", { name: "加载更早的 100 条提交" }),
  );
  await screen.findByText("200+");
  expect(api.history).toHaveBeenLastCalledWith(repo.path, 200, true);
  fireEvent.click(toggle);
  await screen.findByText("100+");
  expect(toggle.getAttribute("aria-checked")).toBe("false");
  expect(api.history).toHaveBeenLastCalledWith(repo.path, 100, false);
});

it.each(["resolve", "reject"])(
  "ignores a late all-branches %s after returning to current history",
  async (result) => {
    render(<App />);
    await screen.findByText("100+");
    const pending = deferred<History>();
    api.history.mockReturnValueOnce(pending.promise);
    const toggle = screen.getByRole("switch", { name: "显示全部分支" });
    fireEvent.click(toggle);
    await screen.findByText("正在读取提交历史…");
    fireEvent.click(toggle);
    await screen.findByText("100+");
    await act(async () => {
      if (result === "resolve") pending.resolve(makeHistory(2));
      else pending.reject({ message: "旧范围的读取错误" });
    });
    expect(screen.getByText("100+")).toBeTruthy();
    expect(screen.queryByText("旧范围的读取错误")).toBeNull();
    expect(toggle.getAttribute("aria-checked")).toBe("false");
  },
);

it("uses the latest scope for retry and a sync started before toggling", async () => {
  const pending = deferred<RepoStatus>();
  api.sync.mockReturnValueOnce(pending.promise);
  render(<App />);
  await screen.findByText("100+");
  fireEvent.click(screen.getByRole("button", { name: "同步此仓库" }));
  api.history.mockRejectedValueOnce({ message: "新范围读取失败" });
  fireEvent.click(screen.getByRole("switch", { name: "显示全部分支" }));
  await screen.findByText("新范围读取失败");
  fireEvent.click(screen.getByRole("button", { name: "重试加载" }));
  await screen.findByText("100+");
  expect(api.history).toHaveBeenLastCalledWith(repo.path, 100, true);
  api.history.mockClear();
  await act(async () => pending.resolve(status));
  await waitFor(() =>
    expect(api.history).toHaveBeenLastCalledWith(repo.path, 100, true),
  );
});

describe("repository groups and bulk actions", () => {
  const other = {
    ...repo,
    path: "/test/other",
    name: "other",
    commonDir: "/test/other/.git",
  };
  const third = {
    ...repo,
    path: "/test/third",
    name: "third",
    commonDir: "/test/third/.git",
  };

  function checkbox(name: string) {
    return screen.getByRole("checkbox", {
      name: `选择仓库 ${name}`,
    }) as HTMLInputElement;
  }
  function row(name: string) {
    return screen.getByRole("button", {
      name: new RegExp(`^查看仓库 ${name}，`),
    });
  }

  function flagStore(initial: Repo[]) {
    let saved = initial;
    api.list.mockImplementation(async () => saved);
    api.updateRepoFlags.mockImplementation(async (path: string, patch) => {
      saved = saved.map((r) => (r.path === path ? { ...r, ...patch } : r));
      return saved.find((r) => r.path === path)!;
    });
  }

  it("pins inside the original group and automatic group, shares checks and runs each repository once", async () => {
    flagStore([repo, other, third]);
    api.groups.mockResolvedValue([
      { name: "工作", paths: [repo.path, other.path] },
    ]);
    api.fetch.mockImplementation(async (path: string) => ({ ...status, path }));
    const view = render(<App />);
    await screen.findByText("100+");
    menuAction("置顶仓库", other.name);
    const pinned = await screen.findByRole("group", { name: "置顶仓库" });
    const original = screen.getByRole("group", { name: "仓库分组 工作" });
    expect(
      within(original)
        .getAllByRole("checkbox")
        .slice(1)
        .map((x) => x.getAttribute("aria-label")),
    ).toEqual(["选择仓库 other", `选择仓库 ${repo.name}`]);
    fireEvent.click(
      within(pinned).getByRole("checkbox", { name: "选择仓库 other" }),
    );
    expect(
      (
        within(original).getByRole("checkbox", {
          name: "选择仓库 other",
        }) as HTMLInputElement
      ).checked,
    ).toBe(true);
    expect(screen.getByText("已选 1 个")).toBeTruthy();
    fireEvent.click(screen.getByRole("checkbox", { name: "全选当前可见仓库" }));
    expect(screen.getByText("已选 3 个")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "抓取所选" }));
    await screen.findByText(/批量抓取完成 · 3\/3 · 3 成功/);
    expect(api.fetch.mock.calls.map((c) => c[0]).sort()).toEqual(
      [repo.path, other.path, third.path].sort(),
    );
    expect(api.editGroups).not.toHaveBeenCalled();
    view.unmount();
    render(<App />);
    const restored = await screen.findByRole("group", { name: "置顶仓库" });
    fireEvent.click(
      within(restored).getByRole("button", { name: "更多操作 other" }),
    );
    fireEvent.click(screen.getByRole("menuitem", { name: "取消置顶" }));
    await waitFor(() =>
      expect(screen.queryByRole("group", { name: "置顶仓库" })).toBeNull(),
    );
    expect(api.updateRepoFlags).toHaveBeenLastCalledWith(other.path, {
      pinned: false,
    });
    expect(
      within(screen.getByRole("group", { name: "仓库分组 工作" }))
        .getAllByRole("checkbox")
        .slice(1)
        .map((x) => x.getAttribute("aria-label")),
    ).toEqual([`选择仓库 ${repo.name}`, "选择仓库 other"]);
  });

  it("keeps Shift anchors tied to a displayed occurrence when pinned repositories appear twice", async () => {
    flagStore([repo, { ...other, pinned: true }, { ...third, pinned: true }]);
    api.groups.mockResolvedValue([
      { name: "工作", paths: [repo.path, other.path] },
    ]);
    render(<App />);
    await screen.findByText("100+");
    const original = screen.getByRole("group", { name: "仓库分组 工作" });
    fireEvent.click(
      within(original).getByRole("button", { name: /^查看仓库 other，/ }),
    );
    fireEvent.click(
      within(original).getByRole("checkbox", { name: `选择仓库 ${repo.name}` }),
      { shiftKey: true },
    );
    expect(screen.getByText("已选 2 个")).toBeTruthy();
    expect(
      screen
        .getAllByRole("checkbox", { name: "选择仓库 third" })
        .every((x) => !(x as HTMLInputElement).checked),
    ).toBe(true);
    expect(
      screen
        .getAllByRole("checkbox", { name: "选择仓库 other" })
        .every((x) => (x as HTMLInputElement).checked),
    ).toBe(true);
  });

  it("blocks every sync entry and reports batch skips while allowing fetch, then allows sync after unmarking", async () => {
    flagStore([{ ...repo, syncDisabled: true }, other]);
    api.sync.mockImplementation(async (path: string) => ({ ...status, path }));
    api.fetch.mockImplementation(async (path: string) => ({ ...status, path }));
    render(<App />);
    await screen.findByText("100+");
    expect(
      (
        screen.getByRole("button", {
          name: `同步 ${repo.name}`,
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    expect(
      (screen.getByRole("button", { name: "同步此仓库" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    fireEvent.click(
      screen.getByRole("button", { name: `更多操作 ${repo.name}` }),
    );
    expect(
      (
        screen.getByRole("menuitem", {
          name: "同步此仓库",
        }) as HTMLButtonElement
      ).disabled,
    ).toBe(true);
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    fireEvent.click(screen.getByRole("button", { name: "同步预览" }));
    await screen.findByText("有效策略");
    expect(
      (screen.getByRole("button", { name: "执行同步" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    fireEvent.click(screen.getByRole("button", { name: /^同步全部/ }));
    await screen.findByText(/批量同步完成 · 2\/2 · 1 成功 · 1 跳过/);
    expect(api.sync.mock.calls.map((c) => c[0])).toEqual([other.path]);
    expect(screen.getByText(/已禁止同步，仍可抓取/)).toBeTruthy();
    fireEvent.click(screen.getByRole("checkbox", { name: "全选当前可见仓库" }));
    fireEvent.click(screen.getByRole("button", { name: "同步所选" }));
    await waitFor(() => expect(api.sync).toHaveBeenCalledTimes(2));
    await screen.findByText(/批量同步完成 · 2\/2 · 1 成功 · 1 跳过/);
    fireEvent.click(screen.getByRole("button", { name: "抓取全部" }));
    await screen.findByText(/批量抓取完成 · 2\/2 · 2 成功/);
    expect(api.fetch.mock.calls.map((c) => c[0]).sort()).toEqual(
      [repo.path, other.path].sort(),
    );
    menuAction("允许同步");
    await waitFor(() =>
      expect(
        (
          screen.getByRole("button", {
            name: "同步此仓库",
          }) as HTMLButtonElement
        ).disabled,
      ).toBe(false),
    );
    fireEvent.click(screen.getByRole("button", { name: "同步此仓库" }));
    await waitFor(() => expect(api.sync).toHaveBeenLastCalledWith(repo.path));
  });

  it("preserves marks on save failure and applies disabling only after a successful save", async () => {
    flagStore([repo]);
    render(<App />);
    await screen.findByText("100+");
    api.updateRepoFlags.mockRejectedValueOnce(new Error("磁盘已满"));
    menuAction("禁止同步（仍可抓取）");
    await screen.findByText(/保存仓库标记失败：磁盘已满/);
    expect(
      (screen.getByRole("button", { name: "同步此仓库" }) as HTMLButtonElement)
        .disabled,
    ).toBe(false);
    menuAction("禁止同步（仍可抓取）");
    await screen.findByText("禁止同步 · 可抓取");
    expect(
      (screen.getByRole("button", { name: "同步此仓库" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(api.updateRepoFlags).toHaveBeenLastCalledWith(repo.path, {
      syncDisabled: true,
    });
  });

  it("does not start a batch sync while the prohibition is being saved", async () => {
    flagStore([repo]);
    render(<App />);
    await screen.findByText("100+");
    const pending = deferred<Repo>();
    api.updateRepoFlags.mockReturnValueOnce(pending.promise);
    menuAction("禁止同步（仍可抓取）");
    fireEvent.click(screen.getByRole("button", { name: /^同步全部/ }));
    await screen.findByText(/批量同步完成 · 1\/1 · 1 跳过/);
    expect(screen.getByText(/仓库标记正在保存/)).toBeTruthy();
    expect(api.sync).not.toHaveBeenCalled();
    await act(async () => pending.resolve({ ...repo, syncDisabled: true }));
    await screen.findByText("禁止同步 · 可抓取");
    expect(
      (screen.getByRole("button", { name: "同步此仓库" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
  });

  it("toggles a focused repository with Space without switching history, repeating or affecting other controls", async () => {
    api.list.mockResolvedValue([repo, other]);
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(row(other.name));
    await waitFor(() =>
      expect(api.history).toHaveBeenLastCalledWith(other.path, 100, false),
    );
    api.history.mockClear();
    const target = row(other.name);
    expect(document.activeElement).toBe(target);
    expect(fireEvent.keyDown(target, { key: " " })).toBe(false);
    fireEvent.keyUp(target, { key: " " });
    expect(checkbox(other.name).checked).toBe(true);
    fireEvent.keyDown(target, { key: " ", repeat: true });
    expect(checkbox(other.name).checked).toBe(true);
    fireEvent.keyDown(target, { key: " " });
    expect(checkbox(other.name).checked).toBe(false);
    fireEvent.keyDown(target, { key: " ", ctrlKey: true });
    fireEvent.keyDown(screen.getByRole("textbox", { name: "搜索仓库" }), {
      key: " ",
    });
    expect(checkbox(other.name).checked).toBe(false);
    expect(api.history).not.toHaveBeenCalled();
    expect(api.sync).not.toHaveBeenCalled();
    expect(api.fetch).not.toHaveBeenCalled();
  });

  it("checks and unchecks a range in either direction from a fixed anchor", async () => {
    api.list.mockResolvedValue([repo, other, third]);
    render(<App />);
    await screen.findByText("100+");
    api.history.mockClear();
    fireEvent.click(checkbox(repo.name));
    fireEvent.click(checkbox(third.name), { shiftKey: true });
    expect([repo, other, third].map((r) => checkbox(r.name).checked)).toEqual([
      true,
      true,
      true,
    ]);
    // A shifted click keeps the original anchor; clicking a checked endpoint clears the range.
    fireEvent.click(checkbox(other.name), { shiftKey: true });
    expect([repo, other, third].map((r) => checkbox(r.name).checked)).toEqual([
      false,
      false,
      true,
    ]);
    fireEvent.click(screen.getByRole("button", { name: "清空选择" }));
    fireEvent.click(checkbox(third.name));
    fireEvent.click(checkbox(repo.name), { shiftKey: true });
    expect(screen.getByText("已选 3 个")).toBeTruthy();
    expect(api.history).not.toHaveBeenCalled();
  });

  it("uses a viewed row as the anchor for Shift-click and Shift-Space", async () => {
    api.list.mockResolvedValue([repo, other, third]);
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(row(repo.name));
    fireEvent.click(row(third.name), { shiftKey: true });
    expect(screen.getByText("已选 3 个")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "清空选择" }));
    fireEvent.click(row(third.name));
    fireEvent.keyDown(row(repo.name), { key: " ", shiftKey: true });
    expect(screen.getByText("已选 3 个")).toBeTruthy();
  });

  it("ranges only over expanded filtered rows in display order and preserves hidden checks", async () => {
    api.list.mockResolvedValue([repo, other, third]);
    api.groups.mockResolvedValue([
      { name: "工作", paths: [repo.path, third.path] },
    ]);
    render(<App />);
    await screen.findByText("100+");
    // Display order is repo, third, other rather than the underlying repository order.
    fireEvent.click(checkbox(repo.name));
    fireEvent.click(checkbox(third.name), { shiftKey: true });
    expect(checkbox(other.name).checked).toBe(false);
    expect(screen.getByText("已选 2 个")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /折叠分组 工作/ }));
    // The hidden anchor must fall back to selecting only the clicked visible row.
    fireEvent.click(checkbox(other.name), { shiftKey: true });
    expect(screen.getByText("已选 3 个")).toBeTruthy();
    expect(screen.getByText("（含 2 个隐藏项）")).toBeTruthy();
    fireEvent.change(screen.getByRole("textbox", { name: "搜索仓库" }), {
      target: { value: "other" },
    });
    fireEvent.keyDown(row(other.name), { key: " ", shiftKey: true });
    expect(screen.getByText("已选 2 个")).toBeTruthy();
    expect(screen.getByText("（含 2 个隐藏项）")).toBeTruthy();
  });

  it("skips a collapsed group between two visible range endpoints", async () => {
    api.list.mockResolvedValue([repo, other, third]);
    api.groups.mockResolvedValue([
      { name: "起点", paths: [repo.path] },
      { name: "隐藏", paths: [other.path] },
      { name: "终点", paths: [third.path] },
    ]);
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("button", { name: /折叠分组 隐藏/ }));
    fireEvent.click(checkbox(repo.name));
    fireEvent.click(checkbox(third.name), { shiftKey: true });
    expect(screen.getByText("已选 2 个")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: /展开分组 隐藏/ }));
    expect(checkbox(other.name).checked).toBe(false);
    expect(checkbox(repo.name).checked).toBe(true);
    expect(checkbox(third.name).checked).toBe(true);
  });

  it("keeps checked repositories separate from history and runs Fetch only for checked paths, including hidden ones", async () => {
    api.list.mockResolvedValue([repo, other, third]);
    api.fetch.mockImplementation(async (path: string) => ({ ...status, path }));
    render(<App />);
    await screen.findByText("100+");
    api.history.mockClear();
    fireEvent.click(
      screen.getByRole("checkbox", { name: `选择仓库 ${repo.name}` }),
    );
    fireEvent.change(screen.getByRole("textbox", { name: "搜索仓库" }), {
      target: { value: "other" },
    });
    fireEvent.click(screen.getByRole("checkbox", { name: "全选当前可见仓库" }));
    expect(screen.getByText("已选 2 个")).toBeTruthy();
    expect(api.history).not.toHaveBeenCalled();
    fireEvent.click(screen.getByRole("button", { name: "抓取所选" }));
    await screen.findByText(/批量抓取完成 · 2\/2 · 2 成功/);
    expect(api.fetch.mock.calls.map((c) => c[0]).sort()).toEqual(
      [repo.path, other.path].sort(),
    );
    expect(api.sync).not.toHaveBeenCalled();
    expect(screen.getByText("已选 2 个")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "清空选择" }));
    expect(screen.queryByRole("button", { name: "抓取所选" })).toBeNull();
  });

  it("selects a filtered group and excludes collapsed rows from visible select-all", async () => {
    api.list.mockResolvedValue([repo, other, third]);
    api.groups.mockResolvedValue([
      { name: "工作", paths: [repo.path, other.path] },
    ]);
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(
      screen.getByRole("checkbox", { name: "选择分组 工作 中的仓库" }),
    );
    expect(screen.getByText("已选 2 个")).toBeTruthy();
    fireEvent.click(
      screen.getByRole("checkbox", { name: `选择仓库 ${repo.name}` }),
    );
    expect(
      (
        screen.getByRole("checkbox", {
          name: "选择分组 工作 中的仓库",
        }) as HTMLInputElement
      ).indeterminate,
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: "清空选择" }));
    fireEvent.click(
      screen.getByRole("button", { name: /折叠分组 工作，2 个仓库/ }),
    );
    expect(
      screen.queryByRole("checkbox", { name: `选择仓库 ${repo.name}` }),
    ).toBeNull();
    fireEvent.click(screen.getByRole("checkbox", { name: "全选当前可见仓库" }));
    expect(screen.getByText("已选 1 个")).toBeTruthy();
    expect(
      (
        screen.getByRole("checkbox", {
          name: "选择仓库 third",
        }) as HTMLInputElement
      ).checked,
    ).toBe(true);
  });

  it("creates, renames and deletes groups and moves selected repositories with retry on save failure", async () => {
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("button", { name: "管理仓库分组" }));
    api.editGroups.mockResolvedValueOnce([{ name: "工作", paths: [] }]);
    fireEvent.change(screen.getByLabelText("新分组名称"), {
      target: { value: "工作" },
    });
    fireEvent.click(screen.getByRole("button", { name: "创建分组" }));
    await screen.findByRole("button", { name: "重命名分组 工作" });
    expect(api.editGroups).toHaveBeenLastCalledWith({
      action: "create",
      name: "工作",
    });
    fireEvent.click(screen.getByRole("button", { name: "重命名分组 工作" }));
    fireEvent.change(screen.getByLabelText("重命名“工作”"), {
      target: { value: "项目" },
    });
    api.editGroups.mockResolvedValueOnce([{ name: "项目", paths: [] }]);
    fireEvent.click(screen.getByRole("button", { name: "保存名称" }));
    await screen.findByRole("button", { name: "重命名分组 项目" });
    expect(api.editGroups).toHaveBeenLastCalledWith({
      action: "rename",
      group: "工作",
      name: "项目",
    });
    fireEvent.click(screen.getByRole("button", { name: "关闭" }));
    fireEvent.click(
      screen.getByRole("checkbox", { name: `选择仓库 ${repo.name}` }),
    );
    fireEvent.click(screen.getByRole("button", { name: "移动分组" }));
    fireEvent.change(screen.getByLabelText("目标分组"), {
      target: { value: "项目" },
    });
    api.editGroups.mockRejectedValueOnce(new Error("磁盘已满"));
    fireEvent.click(screen.getByRole("button", { name: "移动仓库" }));
    await screen.findByText("磁盘已满");
    expect(screen.getByText("已选 1 个")).toBeTruthy();
    api.editGroups.mockResolvedValueOnce([
      { name: "项目", paths: [repo.path] },
    ]);
    fireEvent.click(screen.getByRole("button", { name: "移动仓库" }));
    await waitFor(() =>
      expect(
        screen.queryByRole("dialog", { name: "移动仓库到分组" }),
      ).toBeNull(),
    );
    expect(api.editGroups).toHaveBeenLastCalledWith({
      action: "move",
      paths: [repo.path],
      group: "项目",
    });
    expect(
      screen.getByRole("button", { name: /折叠分组 项目，1 个仓库/ }),
    ).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "管理仓库分组" }));
    api.editGroups.mockResolvedValueOnce([]);
    const editsBeforeConfirm = api.editGroups.mock.calls.length;
    fireEvent.click(screen.getByRole("button", { name: "删除分组 项目" }));
    expect(api.editGroups).toHaveBeenCalledTimes(editsBeforeConfirm);
    fireEvent.click(
      within(screen.getByRole("dialog", { name: "删除分组确认" })).getByRole(
        "button",
        { name: "删除分组" },
      ),
    );
    await waitFor(() =>
      expect(
        screen.queryByRole("button", { name: "删除分组 项目" }),
      ).toBeNull(),
    );
    expect(api.editGroups).toHaveBeenLastCalledWith({
      action: "remove",
      group: "项目",
    });
    expect(
      screen.getByRole("checkbox", { name: `选择仓库 ${repo.name}` }),
    ).toBeTruthy();
    expect(api.remove).not.toHaveBeenCalled();
  });

  it("bounds batch concurrency, reserves queued repositories and reports failures and paused syncs separately", async () => {
    const fourth = {
      ...third,
      path: "/test/fourth",
      name: "fourth",
      commonDir: "/test/fourth/.git",
    };
    const targets = [repo, other, third, fourth];
    api.list.mockResolvedValue(targets);
    const pending = new Map<
      string,
      { resolve: (s: RepoStatus) => void; reject: (e: Error) => void }
    >();
    api.sync.mockImplementation(
      (path: string) =>
        new Promise((resolve, reject) =>
          pending.set(path, { resolve, reject }),
        ),
    );
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("checkbox", { name: "全选当前可见仓库" }));
    fireEvent.click(screen.getByRole("button", { name: "同步所选" }));
    expect(api.sync).toHaveBeenCalledTimes(3);
    expect(
      (screen.getByRole("button", { name: "同步 fourth" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    expect(
      (screen.getByRole("button", { name: "抓取所选" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    await act(async () =>
      pending.get(repo.path)!.reject(new Error("网络断开")),
    );
    await waitFor(() => expect(api.sync).toHaveBeenCalledTimes(4));
    await act(async () => {
      pending.get(other.path)!.resolve({
        ...status,
        path: other.path,
        kind: "attention",
        message: "存在未提交改动",
      });
      pending.get(third.path)!.resolve({ ...status, path: third.path });
      pending.get(fourth.path)!.resolve({ ...status, path: fourth.path });
    });
    await screen.findByText(/批量同步完成 · 4\/4 · 2 成功 · 1 需处理 · 1 失败/);
    expect(screen.getByText(/失败：网络断开/)).toBeTruthy();
    expect(screen.getByText(/需处理：存在未提交改动/)).toBeTruthy();
    expect(api.fetch).not.toHaveBeenCalled();
  });

  it("skips an already operating repository while completing the rest of the selection", async () => {
    api.list.mockResolvedValue([repo, other]);
    let finish!: (s: RepoStatus) => void;
    api.sync.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    api.fetch.mockResolvedValue({ ...status, path: other.path });
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("button", { name: `同步 ${repo.name}` }));
    fireEvent.click(screen.getByRole("checkbox", { name: "全选当前可见仓库" }));
    fireEvent.click(screen.getByRole("button", { name: "抓取所选" }));
    await screen.findByText(/批量抓取完成 · 2\/2 · 1 成功 · 1 跳过/);
    expect(api.fetch).toHaveBeenCalledTimes(1);
    expect(api.fetch).toHaveBeenCalledWith(other.path, undefined);
    await act(async () => finish(status));
  });

  it("does not let a late move response close a newly opened move dialog", async () => {
    let finish!: (groups: []) => void;
    api.editGroups.mockReturnValue(
      new Promise((resolve) => {
        finish = resolve;
      }),
    );
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(
      screen.getByRole("checkbox", { name: `选择仓库 ${repo.name}` }),
    );
    fireEvent.click(screen.getByRole("button", { name: "移动分组" }));
    fireEvent.click(screen.getByRole("button", { name: "移动仓库" }));
    fireEvent.click(screen.getByRole("button", { name: "取消" }));
    fireEvent.click(screen.getByRole("button", { name: "移动分组" }));
    await act(async () => finish([]));
    expect(screen.getByRole("dialog", { name: "移动仓库到分组" })).toBeTruthy();
  });
});

describe("repository dialogs", () => {
  it("refreshes the selected linked worktree history after fetching another linked worktree", async () => {
    const linked = { ...repo, path: "/test/linked", name: "linked" };
    api.list.mockResolvedValue([repo, linked]);
    api.fetch.mockResolvedValue({ ...status, path: linked.path });
    render(<App />);
    await screen.findByText("100+");
    api.history.mockClear();
    menuAction("抓取此仓库", linked.name);
    await waitFor(() =>
      expect(api.history).toHaveBeenCalledWith(repo.path, 100, false),
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
    expect(api.history).toHaveBeenLastCalledWith(repo.path, 100, false);
    fireEvent.keyDown(row, { key: "F10", shiftKey: true });
    fireEvent.keyDown(screen.getByRole("menu"), { key: "Escape" });
    expect(document.activeElement).toBe(row);
    fireEvent.contextMenu(row, { clientX: 500, clientY: 250 });
    fireEvent.click(screen.getByRole("menuitem", { name: "选择获取远端…" }));
    fireEvent.click(await screen.findByRole("button", { name: "backup" }));
    await waitFor(() =>
      expect(api.fetch).toHaveBeenCalledWith(other.path, "backup"),
    );
    expect(api.history).toHaveBeenLastCalledWith(repo.path, 100, false);
  });
  it("uses the details menu and disables Git actions for an operating repository", async () => {
    api.fetch.mockReturnValue(new Promise(() => {}));
    api.toolSettings.mockResolvedValue({ terminal: "", editor: "" });
    render(<App />);
    await screen.findByText("100+");
    expect(screen.queryByRole("button", { name: "在编辑器打开" })).toBeNull();
    fireEvent.click(screen.getByRole("button", { name: "抓取此仓库" }));
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
    fireEvent.click(screen.getByRole("button", { name: "抓取全部" }));
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
    expect(api.history).toHaveBeenLastCalledWith(repo.path, 100, false);
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
    const action = await screen.findByRole("button", {
      name: "查看未提交文件",
    });
    expect(action.textContent).toContain("查看未提交文件");
    fireEvent.click(action);
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
    expect(api.history).toHaveBeenLastCalledWith(repo.path, 100, false);
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
      expect(api.history).toHaveBeenLastCalledWith(repo.path, 200, false),
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

it.each(["flags-first", "list-first"])(
  "keeps an added repository and its selection when pin saving overlaps list reload (%s)",
  async (order) => {
    const added = {
      ...repo,
      path: "/test/added",
      name: "added",
      commonDir: "/test/added/.git",
    };
    const flag = deferred<Repo>();
    const list = deferred<Repo[]>();
    api.updateRepoFlags.mockReturnValueOnce(flag.promise);
    api.add.mockResolvedValueOnce(added);
    api.fetch.mockImplementation(async (path: string) => ({ ...status, path }));
    render(<App />);
    await screen.findByText("100+");
    menuAction("置顶仓库");
    api.list.mockReturnValueOnce(list.promise);
    fireEvent.click(screen.getByRole("button", { name: "手动输入仓库路径" }));
    const dialog = screen.getByRole("dialog", { name: "添加本地仓库" });
    fireEvent.change(within(dialog).getByRole("textbox"), {
      target: { value: added.path },
    });
    fireEvent.submit(dialog.querySelector("form")!);
    await waitFor(() => expect(api.list).toHaveBeenCalledTimes(2));
    // The list was captured before the pin was saved. Either reply may arrive first.
    if (order === "flags-first")
      await act(async () => flag.resolve({ ...repo, pinned: true }));
    await act(async () => list.resolve([repo, added]));
    if (order === "list-first")
      await act(async () => flag.resolve({ ...repo, pinned: true }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "添加本地仓库" })).toBeNull(),
    );
    expect(
      screen
        .getByRole("button", { name: /^查看仓库 added，/ })
        .getAttribute("aria-pressed"),
    ).toBe("true");
    expect(screen.getByRole("group", { name: "置顶仓库" })).toBeTruthy();
    fireEvent.click(screen.getByRole("checkbox", { name: "全选当前可见仓库" }));
    expect(screen.getByText("已选 2 个")).toBeTruthy();
    fireEvent.click(screen.getByRole("button", { name: "抓取所选" }));
    await screen.findByText(/批量抓取完成 · 2\/2 · 2 成功/);
    expect(api.fetch.mock.calls.map((c) => c[0]).sort()).toEqual(
      [repo.path, added.path].sort(),
    );
  },
);

it.each(["flags-first", "list-first"])(
  "removes stale checks and keeps synchronization prohibited when saving overlaps removal (%s)",
  async (order) => {
    const removed = {
      ...repo,
      path: "/test/removed",
      name: "removed",
      commonDir: "/test/removed/.git",
    };
    const flag = deferred<Repo>();
    const list = deferred<Repo[]>();
    api.list.mockResolvedValueOnce([repo, removed]);
    api.updateRepoFlags.mockReturnValueOnce(flag.promise);
    api.remove.mockResolvedValueOnce(undefined);
    render(<App />);
    await screen.findByText("100+");
    fireEvent.click(screen.getByRole("checkbox", { name: "选择仓库 removed" }));
    fireEvent.click(
      screen.getByRole("button", { name: /^查看仓库 removed，/ }),
    );
    menuAction("禁止同步（仍可抓取）");
    api.list.mockReturnValueOnce(list.promise);
    removeFromMenu(removed.name);
    const dialog = screen.getByRole("dialog", { name: "移除仓库" });
    fireEvent.click(within(dialog).getByRole("button", { name: "从列表移除" }));
    await waitFor(() => expect(api.list).toHaveBeenCalledTimes(2));
    if (order === "flags-first")
      await act(async () => flag.resolve({ ...repo, syncDisabled: true }));
    await act(async () => list.resolve([repo]));
    if (order === "list-first")
      await act(async () => flag.resolve({ ...repo, syncDisabled: true }));
    await waitFor(() =>
      expect(screen.queryByRole("dialog", { name: "移除仓库" })).toBeNull(),
    );
    expect(
      screen.queryByRole("button", { name: /^查看仓库 removed，/ }),
    ).toBeNull();
    expect(screen.getByText("多选仓库")).toBeTruthy();
    expect(screen.getByText("禁止同步 · 可抓取")).toBeTruthy();
    expect(
      (screen.getByRole("button", { name: "同步此仓库" }) as HTMLButtonElement)
        .disabled,
    ).toBe(true);
    fireEvent.click(screen.getByRole("button", { name: /^同步全部/ }));
    await screen.findByText(/批量同步完成 · 1\/1 · 1 跳过/);
    expect(api.sync).not.toHaveBeenCalled();
  },
);

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
      expect(api.history).toHaveBeenLastCalledWith(c.path, 100, false),
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
