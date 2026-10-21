import { expect, it } from "vitest";
import { attention, workingFileKind, workingFileLabel } from "./attention";
import { status } from "./test/fixtures";
import type { RepoStatus } from "./types";

it("explains the actual reason instead of a generic attention label", () => {
  const cases: [Partial<RepoStatus>, string][] = [
    [{ kind: "attention", changed: 18 }, "18 项未提交"],
    [{ kind: "attention", upstream: null }, "未设上游"],
    [{ kind: "attention", operation: "rebase" }, "rebase 未完成"],
    [{ kind: "attention", head: "(initial)", upstream: null }, "尚无提交"],
    [{ kind: "attention", branch: "(detached)", upstream: null }, "游离 HEAD"],
    [
      { kind: "attention", message: "upstream 已失效，请检查远程分支" },
      "上游已失效",
    ],
    [{ kind: "conflict", conflicts: ["file.txt"] }, "1 项冲突"],
    [{ kind: "diverged", ahead: 2, behind: 3 }, "分支已分叉"],
    [{ kind: "error", errorKind: "auth" }, "认证失败"],
    [{ kind: "error", errorKind: "network" }, "网络失败"],
    [{ kind: "ahead", ahead: 2 }, "待推送 2"],
  ];
  for (const [overrides, label] of cases) {
    const info = attention({ ...status, ...overrides });
    expect(info.label).toBe(label);
    expect(info.action.length).toBeGreaterThan(8);
  }
  expect(
    attention({ ...status, kind: "behind", behind: 2, changed: 1 }).action,
  ).toContain("先检查并提交");
  expect(attention(status, "找不到 Git").reason).toBe("找不到 Git");
  expect(attention(status, "找不到 Git").action).toContain("Git 安装");
});

it("distinguishes staged, unstaged, untracked, renamed and conflicted files", () => {
  for (const [code, stage, kind] of [
    ["M ", "已暂存", "修改"],
    [" M", "未暂存", "修改"],
    ["MM", "已暂存 / 未暂存", "修改"],
    ["??", "未跟踪", "新增"],
    ["R ", "已暂存", "重命名"],
    [" D", "未暂存", "删除"],
    ["UU", "冲突", "冲突"],
  ]) {
    expect(workingFileLabel(code)).toBe(stage);
    expect(workingFileKind(code)).toBe(kind);
  }
});

it("identifies the paused operation for rebase and merge conflicts", () => {
  for (const operation of ["rebase", "merge"]) {
    const info = attention({
      ...status,
      kind: "conflict",
      operation,
      conflicts: ["shared.txt"],
    });
    expect(info.label).toContain(operation);
    expect(info.action).toContain("未推送");
    expect(info.action).toContain(`继续或中止 ${operation}`);
  }
  expect(
    attention({ ...status, kind: "diverged", ahead: 1, behind: 1 }).action,
  ).toContain("pull.rebase");
});
