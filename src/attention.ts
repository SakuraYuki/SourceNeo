import { labels, type RepoStatus } from "./types";

export function attention(status?: RepoStatus, failure?: string) {
  if (failure)
    return {
      label: "读取失败",
      reason: failure,
      action:
        "根据具体错误检查仓库路径、访问权限或 Git 安装，修正后刷新本地状态。",
    };
  if (!status)
    return {
      label: "读取中",
      reason: "正在读取本地状态",
      action: "请等待状态读取完成。",
    };
  const reason = status.message;
  if (status.kind === "conflict" || status.conflicts.length)
    return {
      label: `${status.operation ? `${status.operation} · ` : ""}${status.conflicts.length} 项冲突`,
      reason,
      action: status.operation
        ? `同步已暂停且未推送。检查冲突文件，在外部 Git 工具解决后继续或中止 ${status.operation}，再重新同步。`
        : "检查冲突文件，解决冲突并完成当前 Git 操作，再重新同步。",
    };
  if (status.operation)
    return {
      label: `${status.operation} 未完成`,
      reason,
      action: `先完成或由外部 Git 工具中止 ${status.operation}，再同步；本工具不会自动中止。`,
    };
  if (status.kind === "error")
    return {
      label:
        status.errorKind === "auth"
          ? "认证失败"
          : status.errorKind === "network"
            ? "网络失败"
            : "同步失败",
      reason,
      action:
        status.errorKind === "auth"
          ? "先在终端完成 Git 认证，再重试同步。"
          : status.errorKind === "network"
            ? "检查网络和远端连接，再重试同步。"
            : "查看具体错误或复制诊断，处理原因后重试同步。",
    };
  if (status.head === "(initial)")
    return {
      label: "尚无提交",
      reason,
      action: "先创建首次提交，再配置上游分支。",
    };
  if (status.branch === "(detached)")
    return {
      label: "游离 HEAD",
      reason,
      action: status.changed
        ? "先用外部 Git 工具保护当前提交和未提交改动，再切换到本地分支。"
        : "使用分支入口切换到本地分支；如需保留当前提交，先新建分支。",
    };
  if (!status.upstream)
    return {
      label: "未设上游",
      reason,
      action: status.changed
        ? "先检查并提交改动，再在外部 Git 工具中为当前分支设置上游；不会自动推送新分支。"
        : "选择已有远程分支创建跟踪分支，或在外部 Git 工具中为当前分支设置上游。",
    };
  if (status.kind === "diverged")
    return {
      label: "分支已分叉",
      reason,
      action:
        "工作区干净时可点击同步，按分支 / pull.rebase 与 pull.ff 配置整合两端提交；若仅允许快进或未配置策略，请先检查 Git 配置。分叉不代表文件冲突。",
    };
  const dirty = status.changed > 0;
  if (status.kind === "ahead" || status.kind === "behind")
    return {
      label: `${labels[status.kind]} ${status.kind === "ahead" ? status.ahead : status.behind}`,
      reason,
      action: dirty
        ? "先检查并提交本地改动，再同步当前分支。"
        : "点击同步此仓库，按 Git 配置整合远端提交，再执行普通推送。",
    };
  if (status.kind === "attention" && status.reason === "upstreamMissing")
    return {
      label: "上游已失效",
      reason,
      action: "检查远程分支是否存在，并在外部 Git 工具中修正上游设置。",
    };
  if (dirty)
    return {
      label: `${status.changed} 项未提交`,
      reason: `有 ${status.changed} 个文件存在未提交改动。`,
      action: "查看下面的文件清单，确认后用 AI 或外部 Git 工具提交，再同步。",
    };
  return {
    label: labels[status.kind],
    reason,
    action:
      status.kind === "synced"
        ? "无需处理；已同步表示与本地已知的远程引用一致。"
        : "查看具体原因，处理后刷新或复制诊断。",
  };
}

export function workingFileLabel(status: string) {
  if (["DD", "AU", "UD", "UA", "DU", "AA", "UU"].includes(status))
    return "冲突";
  if (status === "??") return "未跟踪";
  const labels: string[] = [];
  if (status[0] && status[0] !== " ") labels.push("已暂存");
  if (status[1] && status[1] !== " ") labels.push("未暂存");
  return labels.join(" / ") || "改动";
}

export function workingFileKind(status: string) {
  if (["DD", "AU", "UD", "UA", "DU", "AA", "UU"].includes(status))
    return "冲突";
  if (status === "??") return "新增";
  if (status.includes("R")) return "重命名";
  if (status.includes("D")) return "删除";
  if (status.includes("A")) return "新增";
  if (status.includes("T")) return "类型变化";
  return "修改";
}
