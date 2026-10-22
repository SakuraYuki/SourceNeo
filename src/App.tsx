import { Fragment, useCallback, useEffect, useRef, useState } from "react";
import {
  Activity,
  ArrowDown,
  ArrowUp,
  Check,
  ChevronDown,
  ChevronRight,
  CircleAlert,
  FolderGit2,
  GitBranch,
  GitFork,
  GitMerge,
  History as HistoryIcon,
  LoaderCircle,
  Plus,
  RefreshCw,
  Search,
  Settings2,
  MoreHorizontal,
  X,
  Terminal,
  ShieldCheck,
  Sun,
  Moon,
  Columns2,
  Rows2,
} from "lucide-react";
import { api, desktop, pooled } from "./api";
import {
  errorText,
  type History,
  type LogEntry,
  type Repo,
  type RepoStatus,
  type RepoGroup,
  type GroupEdit,
} from "./types";
import { RepoCheckbox, RepoGroupsDialog, MoveReposDialog } from "./RepoGroups";
import {
  BatchReport,
  type BatchProgress,
  type BatchResult,
} from "./BatchReport";
import { HistoryView } from "./HistoryView";
import { Dialog } from "./Dialog";
import { BranchDialog } from "./BranchDialog";
import { AttentionDialog } from "./AttentionDialog";
import { ToolSettingsDialog } from "./ExternalTools";
import { RepoMenu, type MenuAction } from "./RepoMenu";
import { attention } from "./attention";
import {
  applyTheme,
  initialTheme,
  initialLayout,
  initialPanelWidth,
  MIN_PANEL_WIDTH,
  MAX_PANEL_WIDTH,
  savePreference,
  type Theme,
  type Layout,
} from "./preferences";
const REPO_ROW_HEIGHT = 30;
const emptyHistory: History = { commits: [], hasMore: false, shallow: false };
function time(t: number | null | undefined) {
  if (!t) return "尚未获取";
  const date = new Date(t * 1000);
  const clock = date.toLocaleTimeString("zh-CN", {
    hour: "2-digit",
    minute: "2-digit",
  });
  return date.toDateString() === new Date().toDateString()
    ? clock
    : `${date.getMonth() + 1}/${date.getDate()} ${clock}`;
}
function StatusIcon({ kind }: { kind: RepoStatus["kind"] }) {
  if (kind === "synced") return <Check size={13} />;
  if (kind === "ahead") return <ArrowUp size={13} />;
  if (kind === "behind") return <ArrowDown size={13} />;
  if (kind === "diverged") return <GitFork size={13} />;
  if (kind === "conflict") return <GitMerge size={13} />;
  return <CircleAlert size={13} />;
}
export default function App() {
  const [theme, setTheme] = useState<Theme>(initialTheme);
  const [layout, setLayout] = useState<Layout>(initialLayout);
  const [panelWidth, setPanelWidth] = useState(initialPanelWidth);
  const [viewportWidth, setViewportWidth] = useState(window.innerWidth);
  const isSide = layout === "side" && viewportWidth >= 1040;
  const maximumPanelWidth = Math.max(
    MIN_PANEL_WIDTH,
    Math.min(MAX_PANEL_WIDTH, viewportWidth - 610),
  );
  const effectivePanelWidth = Math.min(panelWidth, maximumPanelWidth);
  useEffect(() => {
    applyTheme(theme);
    savePreference("theme", theme);
    void api
      .theme(theme)
      .catch((e) => setNotice(`窗口主题切换失败：${errorText(e)}`));
  }, [theme]);
  useEffect(() => {
    savePreference("layout", layout);
  }, [layout]);
  useEffect(() => {
    savePreference("panelWidth", String(panelWidth));
  }, [panelWidth]);
  const [repos, setRepos] = useState<Repo[]>([]);
  const [groups, setGroups] = useState<RepoGroup[]>([]);
  const [groupsOpen, setGroupsOpen] = useState(false);
  const [movePaths, setMovePaths] = useState<string[] | null>(null);
  const [checked, setChecked] = useState<Set<string>>(new Set());
  const [collapsed, setCollapsed] = useState<Set<string>>(new Set());
  const [batch, setBatch] = useState<BatchProgress | null>(null);
  const batchGuard = useRef(false);
  const groupGuard = useRef(false);
  const groupRevision = useRef(0);
  const [statuses, setStatuses] = useState<Record<string, RepoStatus>>({});
  const [failures, setFailures] = useState<Record<string, string>>({});
  const [selected, setSelected] = useState("");
  const [busy, setBusy] = useState<Set<string>>(new Set());
  const [refreshing, setRefreshing] = useState(false);
  const [query, setQuery] = useState("");
  const [filter, setFilter] = useState(false);
  const [tab, setTab] = useState<"history" | "logs">("history");
  const [logs, setLogs] = useState<LogEntry[]>([]);
  const [history, setHistory] = useState<History | null>(null);
  const [historyLoading, setHistoryLoading] = useState(false);
  const [historyError, setHistoryError] = useState("");
  const [allBranches, setAllBranches] = useState(false);
  const allBranchesRef = useRef(allBranches);
  allBranchesRef.current = allBranches;
  const [limit, setLimit] = useState(100);
  const [notice, setNotice] = useState("");
  const [adding, setAdding] = useState(false);
  const [branchRepo, setBranchRepo] = useState<Repo | null>(null);
  const [branchRevision, setBranchRevision] = useState(0);
  const [menu, setMenu] = useState<{
    repo: Repo;
    x: number;
    y: number;
    origin: HTMLElement;
  } | null>(null);
  const [toolsOpen, setToolsOpen] = useState(false);
  const toolBusy = useRef(new Set<string>());
  const [openingTools, setOpeningTools] = useState<Set<string>>(new Set());
  const remoteRequest = useRef(0);
  const [fetchChoice, setFetchChoice] = useState<{
    repo: Repo;
    remotes: string[];
  } | null>(null);
  const [issuesOpen, setIssuesOpen] = useState(false);
  const [issuesPath, setIssuesPath] = useState<string | undefined>();
  const [addError, setAddError] = useState("");
  const [addSubmitting, setAddSubmitting] = useState(false);
  const addSession = useRef(0);
  const pendingAdd = useRef<number | null>(null);
  function closeAdd() {
    addSession.current++;
    pendingAdd.current = null;
    setAddSubmitting(false);
    setAdding(false);
  }
  const [pathInput, setPathInput] = useState("");
  const [remove, setRemove] = useState<Repo | null>(null);
  const [removeError, setRemoveError] = useState("");
  const [removeSubmitting, setRemoveSubmitting] = useState(false);
  const removeSession = useRef(0);
  const pendingRemove = useRef<number | null>(null);
  function closeRemove() {
    removeSession.current++;
    pendingRemove.current = null;
    setRemoveSubmitting(false);
    setRemove(null);
  }
  const [panelHeight, setPanelHeight] = useState(244);
  const [viewportHeight, setViewportHeight] = useState(window.innerHeight);
  useEffect(() => {
    const resize = () => {
      setViewportHeight(window.innerHeight);
      setViewportWidth(window.innerWidth);
    };
    window.addEventListener("resize", resize);
    return () => window.removeEventListener("resize", resize);
  }, []);
  const maximumPanelHeight = Math.max(
    106,
    viewportHeight -
      380 -
      (notice ? 60 : 0) -
      (batch ? viewportHeight * 0.23 + 8 : 0) -
      (!desktop ? 30 : 0),
  );
  const repoRef = useRef(repos);
  repoRef.current = repos;
  const busyRef = useRef(new Set<string>());
  const refreshGuard = useRef(false);
  const lastRefresh = useRef(0);
  const request = useRef(0);
  const selectedRef = useRef(selected);
  selectedRef.current = selected;
  const mounted = useRef(true);
  const repoListRequest = useRef(0);
  const reloadRepos = useCallback(async () => {
    const id = ++repoListRequest.current;
    const revision = groupRevision.current;
    try {
      const [list, savedGroups] = await Promise.all([api.list(), api.groups()]);
      if (!mounted.current || id !== repoListRequest.current) return null;
      repoRef.current = list;
      setRepos(list);
      if (revision === groupRevision.current) setGroups(savedGroups);
      setChecked(
        (old) =>
          new Set(list.filter((r) => old.has(r.path)).map((r) => r.path)),
      );
      setSelected((current) =>
        list.some((r) => r.path === current) ? current : (list[0]?.path ?? ""),
      );
      return list;
    } catch (e) {
      if (mounted.current && id === repoListRequest.current) throw e;
      return null;
    }
  }, []);
  async function editGroups(edit: GroupEdit) {
    if (groupGuard.current) throw new Error("分组正在保存，请稍后重试");
    groupGuard.current = true;
    groupRevision.current++;
    try {
      const next = await api.editGroups(edit);
      if (mounted.current) setGroups(next);
    } finally {
      groupRevision.current++;
      groupGuard.current = false;
    }
  }
  function toggleChecked(paths: string[]) {
    setChecked((old) => {
      const next = new Set(old);
      const all = paths.every((p) => old.has(p));
      paths.forEach((p) => (all ? next.delete(p) : next.add(p)));
      return next;
    });
  }
  const refreshLogs = useCallback(async () => {
    try {
      setLogs(await api.logs());
    } catch (e) {
      setNotice(errorText(e));
    }
  }, []);
  const readStatus = useCallback(async (repo: Repo) => {
    try {
      const s = await api.status(repo.path);
      if (!mounted.current) return;
      setStatuses((old) => ({ ...old, [repo.path]: s }));
      setFailures((old) => {
        const next = { ...old };
        delete next[repo.path];
        return next;
      });
    } catch (e) {
      if (mounted.current)
        setFailures((old) => ({ ...old, [repo.path]: errorText(e) }));
    }
  }, []);
  const refresh = useCallback(
    async (force = false) => {
      if (
        refreshGuard.current ||
        Date.now() - lastRefresh.current < (force ? 1000 : 10000)
      )
        return;
      refreshGuard.current = true;
      lastRefresh.current = Date.now();
      setRefreshing(true);
      try {
        await pooled(
          repoRef.current.filter((r) => !busyRef.current.has(r.path)),
          readStatus,
        );
        await refreshLogs();
      } finally {
        refreshGuard.current = false;
        if (mounted.current) setRefreshing(false);
      }
    },
    [readStatus, refreshLogs],
  );
  useEffect(() => {
    mounted.current = true;
    void reloadRepos()
      .then((list) => {
        if (list) void refresh(true);
      })
      .catch((e) => setNotice(errorText(e)));
    const timer = setInterval(() => {
      if (document.visibilityState === "visible") void refresh();
    }, 15000);
    const focus = () => {
      void refresh();
    };
    window.addEventListener("focus", focus);
    return () => {
      mounted.current = false;
      clearInterval(timer);
      window.removeEventListener("focus", focus);
    };
  }, [refresh, reloadRepos]);
  const loadHistory = useCallback(async (path: string, count: number) => {
    const id = ++request.current;
    if (!path) {
      setHistory(null);
      setHistoryError("");
      setHistoryLoading(false);
      return;
    }
    setHistoryLoading(true);
    setHistoryError("");
    try {
      const result = await api.history(path, count, allBranchesRef.current);
      if (id === request.current && selectedRef.current === path)
        setHistory(result);
    } catch (e) {
      if (id === request.current) setHistoryError(errorText(e));
    } finally {
      if (id === request.current) setHistoryLoading(false);
    }
  }, []);
  useEffect(() => {
    setLimit(100);
    setHistory(null);
    void loadHistory(selected, 100);
  }, [selected, allBranches, loadHistory]);
  const statusFingerprint =
    selected && statuses[selected]
      ? `${statuses[selected].head}:${statuses[selected].branch}:${statuses[selected].ahead}:${statuses[selected].behind}`
      : "";
  const lastFingerprint = useRef("");
  useEffect(() => {
    if (
      statusFingerprint &&
      lastFingerprint.current &&
      statusFingerprint !== lastFingerprint.current
    ) {
      setLimit(100);
      void loadHistory(selected, 100);
    }
    lastFingerprint.current = statusFingerprint;
  }, [selected, statusFingerprint, loadHistory]);
  const runSync = useCallback(
    async (
      repo: Repo,
      fetchOnly = false,
      remote?: string,
      reserved = false,
    ): Promise<BatchResult> => {
      if (!reserved && busyRef.current.has(repo.path))
        return { repo, outcome: "skipped", message: "仓库正在操作" };
      if (!reserved) busyRef.current.add(repo.path);
      setBusy(new Set(busyRef.current));
      try {
        const s = fetchOnly
          ? await api.fetch(repo.path, remote)
          : await api.sync(repo.path);
        setStatuses((old) => ({ ...old, [repo.path]: s }));
        if (fetchOnly) setBranchRevision((n) => n + 1);
        if (fetchOnly)
          await pooled(
            repoRef.current.filter(
              (r) => r.path !== repo.path && r.commonDir === repo.commonDir,
            ),
            readStatus,
          );
        setFailures((old) => {
          const next = { ...old };
          delete next[repo.path];
          return next;
        });
        return {
          repo,
          outcome:
            fetchOnly || s.kind === "synced"
              ? "success"
              : s.kind === "error"
                ? "error"
                : "attention",
          message: s.message,
        };
      } catch (e) {
        setNotice(`${repo.name}：${errorText(e)}`);
        await readStatus(repo);
        return { repo, outcome: "error", message: errorText(e) };
      } finally {
        busyRef.current.delete(repo.path);
        setBusy(new Set(busyRef.current));
        await refreshLogs();
        const historyTarget = repoRef.current.find(
          (r) => r.path === selectedRef.current,
        );
        if (
          historyTarget &&
          (historyTarget.path === repo.path ||
            (fetchOnly && historyTarget.commonDir === repo.commonDir))
        ) {
          setLimit(100);
          void loadHistory(historyTarget.path, 100);
        }
      }
    },
    [readStatus, refreshLogs, loadHistory],
  );
  async function chooseFetch(target: Repo) {
    const id = ++remoteRequest.current;
    try {
      const remotes = await api.remotes(target.path);
      if (
        !mounted.current ||
        id !== remoteRequest.current ||
        !repoRef.current.some((r) => r.path === target.path)
      )
        return;
      setFetchChoice({ repo: target, remotes });
    } catch (e) {
      if (mounted.current && id === remoteRequest.current)
        setNotice(errorText(e));
    }
  }
  async function runBatch(targets: Repo[], fetchOnly = false) {
    if (batchGuard.current || !targets.length) return;
    batchGuard.current = true;
    const queued: Repo[] = [],
      results: BatchResult[] = [];
    for (const target of targets) {
      if (busyRef.current.has(target.path))
        results.push({
          repo: target,
          outcome: "skipped",
          message: "仓库正在操作",
        });
      else {
        busyRef.current.add(target.path);
        queued.push(target);
      }
    }
    setBusy(new Set(busyRef.current));
    setBatch({
      label: fetchOnly ? "批量 Fetch" : "批量同步",
      total: targets.length,
      running: true,
      results,
    });
    try {
      await pooled(queued, async (target) => {
        const result = await runSync(target, fetchOnly, undefined, true);
        if (mounted.current)
          setBatch((old) =>
            old ? { ...old, results: [...old.results, result] } : old,
          );
      });
    } finally {
      batchGuard.current = false;
      if (mounted.current)
        setBatch((old) => (old ? { ...old, running: false } : old));
    }
  }
  async function changeBranch(
    target: Repo,
    reference: string,
    create: boolean,
  ) {
    if (busyRef.current.has(target.path))
      throw new Error("该仓库正在操作，请稍后重试");
    busyRef.current.add(target.path);
    setBusy(new Set(busyRef.current));
    try {
      const next = await api.switchBranch(target.path, reference, create);
      setStatuses((old) => ({ ...old, [target.path]: next }));
      setFailures((old) => {
        const copy = { ...old };
        delete copy[target.path];
        return copy;
      });
    } catch (e) {
      // A post-checkout hook can fail after Git has already switched HEAD.
      await readStatus(target);
      throw e;
    } finally {
      if (selectedRef.current === target.path) {
        setLimit(100);
        await loadHistory(target.path, 100);
      }
      await refreshLogs();
      busyRef.current.delete(target.path);
      setBusy(new Set(busyRef.current));
    }
  }
  async function addPath(
    path: string,
    fromDialog = false,
    session = addSession.current,
  ) {
    if (!path.trim() || (fromDialog && pendingAdd.current === session)) return;
    if (fromDialog) {
      pendingAdd.current = session;
      setAddSubmitting(true);
      setAddError("");
    }
    try {
      const repo = await api.add(path);
      const list = await reloadRepos();
      if (session === addSession.current) {
        if (list && repoRef.current.some((r) => r.path === repo.path))
          setSelected(repo.path);
        if (fromDialog) {
          closeAdd();
          setPathInput("");
        }
      }
      await readStatus(repo);
      await refreshLogs();
    } catch (e) {
      if (fromDialog) {
        if (session === addSession.current) setAddError(errorText(e));
      } else setNotice(errorText(e));
    } finally {
      if (fromDialog && session === addSession.current) {
        pendingAdd.current = null;
        setAddSubmitting(false);
      }
    }
  }
  async function removeRepo(target: Repo) {
    const session = removeSession.current;
    if (pendingRemove.current === session) return;
    pendingRemove.current = session;
    setRemoveSubmitting(true);
    setRemoveError("");
    try {
      await api.remove(target.path);
      await reloadRepos();
      if (session === removeSession.current) closeRemove();
    } catch (e) {
      if (session === removeSession.current) setRemoveError(errorText(e));
    } finally {
      if (session === removeSession.current) {
        pendingRemove.current = null;
        setRemoveSubmitting(false);
      }
    }
  }
  async function pick() {
    if (!desktop) {
      setNotice(
        "浏览器仅预览界面。请使用 npm run desktop 启动桌面应用来管理本地仓库。",
      );
      return;
    }
    try {
      const session = addSession.current;
      const path = await api.pick();
      if (typeof path === "string") await addPath(path, false, session);
    } catch (e) {
      setNotice(errorText(e));
    }
  }
  async function copyDiagnostic(path = selected) {
    if (!path) return;
    try {
      let text: string;
      try {
        text = await api.diagnostic(path);
      } catch (e) {
        text = `SourceNeo 诊断\n仓库：${path}\n读取失败：${errorText(e)}`;
      }
      await api.copy(text);
      setNotice("已复制诊断给 AI");
    } catch (e) {
      setNotice(`复制失败：${errorText(e)}`);
    }
  }
  function openRepoMenu(
    target: Repo,
    origin: HTMLElement,
    x?: number,
    y?: number,
  ) {
    if (x === undefined && menu?.origin === origin) {
      setMenu(null);
      return;
    }
    remoteRequest.current++;
    const bounds = origin.getBoundingClientRect();
    setMenu({
      repo: target,
      origin,
      x: x ?? bounds.left,
      y: y ?? bounds.bottom + 4,
    });
  }
  async function openTool(target: Repo, kind: string) {
    if (toolBusy.current.has(target.path)) return;
    toolBusy.current.add(target.path);
    setOpeningTools(new Set(toolBusy.current));
    try {
      await api.openTool(target.path, kind);
    } catch (e) {
      if (mounted.current) setNotice(`${target.name}：${errorText(e)}`);
    } finally {
      toolBusy.current.delete(target.path);
      if (mounted.current) setOpeningTools(new Set(toolBusy.current));
    }
  }
  function menuActions(target: Repo): MenuAction[] {
    const operating = busy.has(target.path);
    return [
      {
        label: "Fetch 此仓库",
        disabled: operating,
        run: () => void runSync(target, true),
      },
      {
        label: "选择获取远端…",
        disabled: operating,
        run: () => void chooseFetch(target),
      },
      {
        label: "同步此仓库",
        disabled: operating,
        run: () => void runSync(target),
      },
      {
        label: "切换分支…",
        disabled: operating,
        run: () => setBranchRepo(target),
      },
      {
        label: "查看未提交文件 / 处理建议",
        run: () => {
          setIssuesPath(target.path);
          setIssuesOpen(true);
        },
      },
      {
        label: "在终端打开",
        separator: true,
        disabled: openingTools.has(target.path),
        run: () => void openTool(target, "terminal"),
      },
      {
        label: "在编辑器打开",
        disabled: openingTools.has(target.path),
        run: () => void openTool(target, "editor"),
      },
      {
        label: "移动到分组…",
        separator: true,
        run: () => setMovePaths([target.path]),
      },
      {
        label: "复制仓库路径",
        separator: true,
        run: () => {
          void api
            .copy(target.path)
            .then(() => setNotice("已复制仓库路径"))
            .catch((e) => setNotice(errorText(e)));
        },
      },
      {
        label: "复制诊断给 AI",
        run: () => void copyDiagnostic(target.path),
      },
      { label: "外部工具设置", run: () => setToolsOpen(true) },
      {
        label: "从列表移除…",
        separator: true,
        danger: true,
        disabled: operating,
        run: () => {
          removeSession.current++;
          setRemoveError("");
          setRemove(target);
        },
      },
    ];
  }
  useEffect(() => {
    remoteRequest.current++;
    setMenu(null);
  }, [selected, query, filter]);
  const repo = repos.find((r) => r.path === selected),
    status = statuses[selected];
  const isIssue = (r: Repo) =>
    Boolean(
      failures[r.path] ||
        (statuses[r.path] && statuses[r.path].kind !== "synced"),
    );
  const issueCount = repos.filter(isIssue).length,
    syncedCount = repos.filter(
      (r) => statuses[r.path]?.kind === "synced" && !failures[r.path],
    ).length;
  const visible = repos.filter(
    (r) =>
      (!filter || isIssue(r)) &&
      `${r.name} ${r.path}`.toLowerCase().includes(query.toLowerCase()),
  );
  const groupedPaths = new Set(groups.flatMap((g) => g.paths));
  const sections = [
    ...groups.map((g) => ({
      key: g.name,
      name: g.name,
      repos: visible.filter((r) => g.paths.includes(r.path)),
    })),
    {
      key: "",
      name: "未分组",
      repos: visible.filter((r) => !groupedPaths.has(r.path)),
    },
  ].filter((g) => g.repos.length || (groups.length > 0 && !query && !filter));
  const expanded = sections.flatMap((g) =>
    groups.length > 0 && collapsed.has(g.key) ? [] : g.repos,
  );
  const checkedRepos = repos.filter((r) => checked.has(r.path));
  const hiddenCheckedCount = checkedRepos.filter(
    (r) => !expanded.some((v) => v.path === r.path),
  ).length;
  const selectedLogs = logs.filter((l) => !selected || l.path === selected);
  const effectivePanelHeight = Math.max(
    106,
    Math.min(
      panelHeight,
      maximumPanelHeight,
      repos.length
        ? 114 +
            (groups.length ? sections.length * 30 : 0) +
            Math.max(expanded.length, 1) * REPO_ROW_HEIGHT
        : 180,
    ),
  );
  function drag(e: React.PointerEvent) {
    const start = isSide ? e.clientX : e.clientY,
      initial = isSide ? effectivePanelWidth : effectivePanelHeight;
    e.currentTarget.setPointerCapture(e.pointerId);
    const move = (ev: PointerEvent) => {
      if (isSide)
        setPanelWidth(
          Math.max(
            MIN_PANEL_WIDTH,
            Math.min(maximumPanelWidth, initial + ev.clientX - start),
          ),
        );
      else
        setPanelHeight(
          Math.max(
            106,
            Math.min(maximumPanelHeight, initial + ev.clientY - start),
          ),
        );
    };
    const end = () => {
      window.removeEventListener("pointermove", move);
      window.removeEventListener("pointerup", end);
    };
    window.addEventListener("pointermove", move);
    window.addEventListener("pointerup", end);
  }
  return (
    <main className="app">
      <header className="app-header">
        <div className="brand">
          <div className="brand-mark">
            <GitBranch size={18} />
          </div>
          <strong>
            Source<span>Neo</span>
          </strong>
        </div>
        <div className="header-actions">
          <button
            className="icon-btn"
            aria-label={theme === "dark" ? "切换到浅色模式" : "切换到深色模式"}
            title={theme === "dark" ? "切换到浅色模式" : "切换到深色模式"}
            onClick={() => setTheme(theme === "dark" ? "light" : "dark")}
          >
            {theme === "dark" ? <Sun size={16} /> : <Moon size={16} />}
          </button>
          <button
            className="icon-btn"
            aria-label={layout === "side" ? "切换到上下布局" : "切换到左右布局"}
            title={
              viewportWidth < 1040
                ? "窄窗口自动使用上下布局，加宽后恢复所选布局"
                : layout === "side"
                  ? "切换到上下布局"
                  : "切换到左右布局"
            }
            disabled={viewportWidth < 1040}
            onClick={() => setLayout(layout === "side" ? "stacked" : "side")}
          >
            {layout === "side" ? <Rows2 size={16} /> : <Columns2 size={16} />}
          </button>
          <span className="safety">
            <ShieldCheck size={13} />
            安全同步
          </span>
          <button
            className="icon-btn"
            title="刷新本地状态（不会访问远程）"
            aria-label="刷新本地状态"
            disabled={refreshing}
            onClick={() => {
              void refresh(true);
              if (selected) void loadHistory(selected, limit);
            }}
          >
            <RefreshCw size={16} className={refreshing ? "spin" : ""} />
          </button>
          <button
            className="primary"
            disabled={!repos.length || busy.size > 0 || batch?.running}
            onClick={() => void runBatch(repoRef.current)}
          >
            {busy.size ? (
              <LoaderCircle className="spin" size={15} />
            ) : (
              <RefreshCw size={15} />
            )}
            同步全部{repos.length > 0 && <kbd>{repos.length}</kbd>}
          </button>
          <button
            disabled={!repos.length || busy.size > 0 || batch?.running}
            onClick={() => void runBatch(repoRef.current, true)}
            title="仅获取远程对象和远程跟踪引用"
          >
            Fetch 全部
          </button>
        </div>
      </header>
      <section className="workspace-heading">
        <div>
          <h1>仓库总览</h1>
        </div>
        <div className="overview-stats">
          <span>
            <i className="dot synced" />
            {syncedCount} 已同步
          </span>
          <button
            className="attention-overview"
            title="查看具体原因和未提交文件"
            aria-haspopup="dialog"
            disabled={issueCount === 0}
            aria-label={`查看 ${issueCount} 个需关注仓库`}
            onClick={() => {
              setIssuesPath(undefined);
              setIssuesOpen(true);
            }}
          >
            <i className="dot attention" />
            {issueCount} 需关注
            <ChevronDown size={12} />
          </button>
          <span className="muted">{repos.length} 个仓库</span>
        </div>
      </section>
      {!desktop && (
        <div className="preview-banner">
          <CircleAlert size={14} />
          当前为浏览器预览 · 本地 Git 操作请在桌面应用中使用
        </div>
      )}
      {notice && (
        <div className="notice" role="alert">
          <CircleAlert size={16} />
          <span>{notice}</span>
          <button
            aria-label="关闭提示"
            className="icon-btn"
            onClick={() => setNotice("")}
          >
            <X size={14} />
          </button>
        </div>
      )}
      {batch && <BatchReport batch={batch} onClose={() => setBatch(null)} />}
      <div
        className={`workspace ${isSide ? "side" : "stacked"} ${isSide && effectivePanelWidth < 300 ? "compact-sidebar" : ""}`}
        style={
          {
            "--repo-row-height": `${REPO_ROW_HEIGHT}px`,
            ...(isSide
              ? {
                  gridTemplateColumns: `${effectivePanelWidth}px 8px minmax(0, 1fr)`,
                }
              : {}),
          } as React.CSSProperties
        }
      >
        <section
          className="repo-panel"
          style={isSide ? undefined : { height: effectivePanelHeight }}
          aria-label="本地仓库"
        >
          <div className="panel-toolbar">
            <div className="section-title">
              <ChevronDown size={14} />
              <span>本地仓库</span>
              <span className="count">{repos.length}</span>
            </div>
            <label className="search">
              <Search size={13} />
              <input
                aria-label="搜索仓库"
                placeholder="搜索仓库…"
                value={query}
                onChange={(e) => setQuery(e.target.value)}
              />
            </label>
            <div className="toolbar-actions">
              <button
                className="icon-btn"
                aria-label="管理仓库分组"
                title="管理仓库分组"
                onClick={() => setGroupsOpen(true)}
              >
                <FolderGit2 size={14} />
              </button>
              <button
                className={`icon-btn ${filter ? "active" : ""}`}
                aria-label="仅显示需关注仓库"
                aria-pressed={filter}
                title="仅显示需关注仓库"
                onClick={() => setFilter((v) => !v)}
              >
                <Settings2 size={14} />
              </button>
              <button
                className="text-btn"
                aria-label="添加仓库"
                title="添加仓库"
                onClick={() => void pick()}
              >
                <Plus size={15} />
                <span>添加仓库</span>
              </button>
              <button
                className="icon-btn"
                aria-label="手动输入仓库路径"
                title="手动输入仓库路径"
                onClick={() => {
                  addSession.current++;
                  setAddError("");
                  setAdding(true);
                }}
              >
                <ChevronDown size={12} />
              </button>
            </div>
          </div>
          <div className="batch-toolbar" aria-label="仓库多选操作">
            <RepoCheckbox
              paths={expanded.map((r) => r.path)}
              checked={checked}
              onToggle={toggleChecked}
              label="全选当前可见仓库"
            />
            <span className="selection-count">
              {checkedRepos.length
                ? `已选 ${checkedRepos.length} 个`
                : "多选仓库"}
            </span>
            {hiddenCheckedCount > 0 && (
              <span className="selection-count">
                （含 {hiddenCheckedCount} 个隐藏项）
              </span>
            )}
            {checkedRepos.length > 0 && (
              <>
                <button
                  className="text-btn"
                  onClick={() => setChecked(new Set())}
                >
                  清空选择
                </button>
                <button
                  disabled={batch?.running}
                  onClick={() => void runBatch(checkedRepos)}
                >
                  同步所选
                </button>
                <button
                  disabled={batch?.running}
                  onClick={() => void runBatch(checkedRepos, true)}
                >
                  Fetch 所选
                </button>
                <button
                  onClick={() => setMovePaths(checkedRepos.map((r) => r.path))}
                >
                  移动分组
                </button>
              </>
            )}
          </div>
          <div className="repo-table">
            <div className="repo-columns">
              <div className="repo-column-labels">
                <span>仓库</span>
                <span>当前分支</span>
                <span>同步状态</span>
                <span className="remote-time">远程更新</span>
              </div>
              <span className="repo-action-heading" />
            </div>
            {!repos.length ? (
              <div className="empty-repos">
                <FolderGit2 size={29} />
                <div>
                  <strong>把你的仓库放在一起</strong>
                  <p>
                    添加本地 Git 仓库或 worktree，一眼查看同步状态与提交历史。
                  </p>
                </div>
                <button onClick={() => void pick()}>
                  <Plus size={14} />
                  添加第一个仓库
                </button>
              </div>
            ) : !visible.length ? (
              <div className="no-matches">
                没有匹配的仓库
                <button
                  className="text-btn"
                  onClick={() => {
                    setQuery("");
                    setFilter(false);
                  }}
                >
                  清除筛选
                </button>
              </div>
            ) : (
              sections.map((section) => (
                <Fragment key={section.key}>
                  {groups.length > 0 && (
                    <div className="repo-group-heading">
                      <RepoCheckbox
                        paths={section.repos.map((r) => r.path)}
                        checked={checked}
                        onToggle={toggleChecked}
                        label={`选择分组 ${section.name} 中的仓库`}
                      />
                      <button
                        className="text-btn"
                        aria-expanded={!collapsed.has(section.key)}
                        aria-label={`${collapsed.has(section.key) ? "展开" : "折叠"}分组 ${section.name}，${section.repos.length} 个仓库`}
                        title={section.name}
                        onClick={() =>
                          setCollapsed((old) => {
                            const next = new Set(old);
                            if (next.has(section.key)) next.delete(section.key);
                            else next.add(section.key);
                            return next;
                          })
                        }
                      >
                        {collapsed.has(section.key) ? (
                          <ChevronRight size={13} />
                        ) : (
                          <ChevronDown size={13} />
                        )}
                        <strong>{section.name}</strong>
                        <span className="count">{section.repos.length}</span>
                      </button>
                    </div>
                  )}
                  {(!groups.length || !collapsed.has(section.key)
                    ? section.repos
                    : []
                  ).map((r) => {
                    const s = statuses[r.path],
                      failed = failures[r.path],
                      loading = busy.has(r.path),
                      kind = failed ? "error" : s?.kind;
                    const info = attention(s, failed);
                    return (
                      <div
                        key={r.path}
                        className={`repo-row ${r.path === selected ? "selected" : ""} ${checked.has(r.path) ? "checked" : ""} ${loading ? "operating" : ""} ${isIssue(r) ? "has-issue" : ""}`}
                        onContextMenu={(e) => {
                          e.preventDefault();
                          const origin =
                            e.currentTarget.querySelector<HTMLButtonElement>(
                              ".repo-select",
                            )!;
                          openRepoMenu(r, origin, e.clientX, e.clientY);
                        }}
                        onKeyDown={(e) => {
                          if (
                            e.key === "ContextMenu" ||
                            (e.shiftKey && e.key === "F10")
                          ) {
                            e.preventDefault();
                            openRepoMenu(r, e.target as HTMLElement);
                          }
                        }}
                      >
                        <RepoCheckbox
                          paths={[r.path]}
                          checked={checked}
                          onToggle={toggleChecked}
                          label={`选择仓库 ${r.name}`}
                        />
                        <button
                          className="repo-select"
                          onClick={() => setSelected(r.path)}
                          aria-label={`查看仓库 ${r.name}，分支 ${s?.branch ?? "读取中"}，${info.label}，${s?.changed ?? 0} 个未提交改动，待推送 ${s?.ahead ?? 0}，待拉取 ${s?.behind ?? 0}`}
                          aria-pressed={r.path === selected}
                          title={`${r.path}\n分支：${s?.branch ?? "读取中"}\n${failed || s?.message || "正在读取状态"}\n远程更新：${s?.lastFetched ? new Date(s.lastFetched * 1000).toLocaleString("zh-CN") : "尚未获取"}`}
                        >
                          <span className="repo-name" title={r.path}>
                            <FolderGit2 size={17} />
                            <strong>{r.name}</strong>
                            {repos.some(
                              (other) =>
                                other.path !== r.path &&
                                other.commonDir === r.commonDir,
                            ) && (
                              <span
                                className="worktree-tag"
                                title="共享 Git 元数据的 worktree"
                              >
                                WT
                              </span>
                            )}
                          </span>
                          <span
                            className={`branch ${s?.changed ? "has-changes" : ""}`}
                          >
                            <GitBranch size={13} />
                            <span className="ellipsis">
                              {s?.branch === "(detached)"
                                ? "游离 HEAD"
                                : (s?.branch ?? "读取中…")}
                            </span>
                            {!!s?.changed &&
                              !info.label.endsWith("项未提交") && (
                                <span className="changes" title="未提交改动数">
                                  {s.changed} M
                                </span>
                              )}
                          </span>
                          <span className="status-cell">
                            {loading ? (
                              <span className="badge working">
                                <LoaderCircle size={12} className="spin" />
                                操作中
                              </span>
                            ) : kind ? (
                              <span
                                className={`badge ${kind}`}
                                title={info.reason}
                              >
                                <StatusIcon kind={kind} />
                                {info.label}
                              </span>
                            ) : (
                              <span className="muted">正在读取</span>
                            )}
                            {!!s?.ahead && (
                              <span className="delta">
                                <ArrowUp size={11} />
                                {s.ahead}
                              </span>
                            )}
                            {!!s?.behind && (
                              <span className="delta">
                                <ArrowDown size={11} />
                                {s.behind}
                              </span>
                            )}
                          </span>
                          <span
                            className="remote-time"
                            title={
                              s?.lastFetched
                                ? new Date(s.lastFetched * 1000).toLocaleString(
                                    "zh-CN",
                                  )
                                : "SourceNeo 尚未成功获取远程状态；当前展示本地已有的远程引用"
                            }
                          >
                            {time(s?.lastFetched)}
                          </span>
                        </button>
                        <div className="repo-row-actions">
                          <button
                            className="icon-btn"
                            title={`同步 ${r.name}`}
                            aria-label={`同步 ${r.name}`}
                            disabled={loading}
                            onClick={() => void runSync(r)}
                          >
                            <RefreshCw
                              size={13}
                              className={loading ? "spin" : ""}
                            />
                          </button>
                          <button
                            className="icon-btn"
                            title={`更多操作 ${r.name}`}
                            aria-label={`更多操作 ${r.name}`}
                            aria-haspopup="menu"
                            aria-expanded={
                              menu?.repo.path === r.path &&
                              menu.origin.getAttribute("aria-label") ===
                                `更多操作 ${r.name}`
                            }
                            onClick={(e) => openRepoMenu(r, e.currentTarget)}
                          >
                            <MoreHorizontal size={13} />
                          </button>
                        </div>
                      </div>
                    );
                  })}
                </Fragment>
              ))
            )}
          </div>
        </section>
        <div
          className="splitter"
          role="separator"
          aria-label={isSide ? "调整仓库列表宽度" : "调整仓库列表高度"}
          aria-orientation={isSide ? "vertical" : "horizontal"}
          aria-valuenow={isSide ? effectivePanelWidth : effectivePanelHeight}
          aria-valuemin={isSide ? MIN_PANEL_WIDTH : 106}
          aria-valuemax={isSide ? maximumPanelWidth : maximumPanelHeight}
          tabIndex={0}
          onPointerDown={drag}
          onKeyDown={(e) => {
            const keys = isSide
              ? ["ArrowLeft", "ArrowRight"]
              : ["ArrowUp", "ArrowDown"];
            if (keys.includes(e.key)) {
              e.preventDefault();
              const delta = e.key === keys[0] ? -20 : 20;
              if (isSide)
                setPanelWidth(
                  Math.max(
                    MIN_PANEL_WIDTH,
                    Math.min(maximumPanelWidth, effectivePanelWidth + delta),
                  ),
                );
              else
                setPanelHeight(
                  Math.max(
                    106,
                    Math.min(maximumPanelHeight, effectivePanelHeight + delta),
                  ),
                );
            }
          }}
        >
          <span />
        </div>
        <section className="detail-panel">
          <div className="detail-heading">
            <div className="detail-name">
              <FolderGit2 size={16} />
              <strong>{repo?.name ?? "提交历史"}</strong>
              {repo && (
                <>
                  <ChevronRight size={13} />
                  <GitBranch size={12} />
                  <button
                    className="text-btn branch-trigger"
                    aria-label="切换分支"
                    disabled={busy.has(repo.path)}
                    onClick={() => setBranchRepo(repo)}
                    title="切换或新建分支"
                  >
                    {status?.branch ?? "读取中"}
                    <ChevronDown size={12} />
                  </button>
                  <span
                    className="detail-remote-time"
                    title={
                      status?.lastFetched
                        ? new Date(status.lastFetched * 1000).toLocaleString(
                            "zh-CN",
                          )
                        : "SourceNeo 尚未成功获取远程状态"
                    }
                  >
                    远程：{time(status?.lastFetched)}
                  </span>
                </>
              )}
            </div>
            {repo && (
              <div className="detail-actions">
                <button
                  className="text-btn"
                  disabled={busy.has(repo.path)}
                  onClick={() => void runSync(repo, true)}
                >
                  Fetch 此仓库
                </button>
                <button
                  className="icon-btn"
                  aria-label="当前仓库更多操作"
                  title="更多操作"
                  aria-haspopup="menu"
                  aria-expanded={
                    menu?.repo.path === repo.path &&
                    menu.origin.getAttribute("aria-label") ===
                      "当前仓库更多操作"
                  }
                  onClick={(e) => openRepoMenu(repo, e.currentTarget)}
                >
                  <MoreHorizontal size={16} />
                </button>
              </div>
            )}
          </div>
          {repo && (
            <div
              className={`repo-summary ${failures[selected] ? "error" : (status?.kind ?? "")}`}
            >
              <div>
                <span className="summary-icon">
                  {status ? (
                    <StatusIcon
                      kind={failures[selected] ? "error" : status.kind}
                    />
                  ) : (
                    <Activity size={13} />
                  )}
                </span>
                <span>{attention(status, failures[selected]).reason}</span>
              </div>
              {isIssue(repo) && (
                <button
                  className="text-btn"
                  onClick={() => {
                    setIssuesPath(selected);
                    setIssuesOpen(true);
                  }}
                >
                  {status?.changed ? "查看未提交文件" : "查看处理建议"}
                </button>
              )}
              <button
                className="text-btn"
                disabled={busy.has(selected)}
                onClick={() => void runSync(repo)}
              >
                {busy.has(selected) ? (
                  <LoaderCircle size={13} className="spin" />
                ) : (
                  <RefreshCw size={13} />
                )}
                同步此仓库
              </button>
            </div>
          )}
          <div className="tabs">
            <button
              className={tab === "history" ? "active" : ""}
              onClick={() => setTab("history")}
            >
              <HistoryIcon size={14} />
              提交历史
              {history && (
                <span>
                  {history.commits.length}
                  {history.hasMore ? "+" : ""}
                </span>
              )}
            </button>
            <button
              className={tab === "logs" ? "active" : ""}
              onClick={() => {
                setTab("logs");
                void refreshLogs();
              }}
            >
              <Terminal size={14} />
              操作日志
              {selectedLogs.some((l) => l.level === "error") && (
                <i className="dot error" />
              )}
            </button>
            <div className="tab-spacer" />
            {tab === "history" && (
              <div className="history-options">
                <button
                  className="history-scope"
                  role="switch"
                  aria-checked={allBranches}
                  aria-label="显示全部分支"
                  disabled={!repo}
                  title="关闭时仅显示当前 HEAD 的历史（含已合并提交）；开启后显示全部分支历史"
                  onClick={() => setAllBranches((value) => !value)}
                >
                  <span>全部分支</span>
                  <span className="switch-track" aria-hidden="true" />
                </button>
                <span className="graph-legend">
                  <i />
                  本地分支
                  <i className="remote" />
                  远程分支 <span className="muted">拓扑排序</span>
                </span>
              </div>
            )}
          </div>
          {tab === "history" ? (
            <HistoryView
              key={selected}
              repoPath={selected}
              history={selected ? history : emptyHistory}
              loading={historyLoading}
              error={historyError}
              onRetry={() => void loadHistory(selected, limit)}
              onMore={() => {
                const next = limit + 100;
                setLimit(next);
                void loadHistory(selected, next);
              }}
            />
          ) : (
            <div className="log-list" role="log" aria-label="Git 操作日志">
              {selectedLogs.length ? (
                selectedLogs.map((l, i) => (
                  <div
                    key={`${l.time}-${i}`}
                    className={`log-entry ${l.level}`}
                  >
                    <time>
                      {new Date(l.time * 1000).toLocaleTimeString("zh-CN", {
                        hour12: false,
                      })}
                    </time>
                    <span className="log-level">
                      {l.level === "error"
                        ? "失败"
                        : l.level === "warning"
                          ? "暂停"
                          : l.level === "success"
                            ? "完成"
                            : "信息"}
                    </span>
                    <span>{l.message}</span>
                  </div>
                ))
              ) : (
                <div className="history-empty">
                  <Terminal size={27} />
                  <p>还没有操作日志</p>
                  <span>同步过程与失败原因会记录在这里</span>
                </div>
              )}
            </div>
          )}
        </section>
      </div>
      <footer>
        <span>
          <i className={`dot ${busy.size ? "attention" : "synced"}`} />
          {busy.size ? `${busy.size} 个仓库正在操作` : "就绪"}
          <span className="footer-divider" />
          Git CLI
        </span>
        <span>
          遵循 Git 同步配置 · 普通推送 · 冲突时暂停
          <span className="footer-divider" />
          v0.1.2
        </span>
      </footer>
      {menu && repos.some((r) => r.path === menu.repo.path) && (
        <RepoMenu
          key={`${menu.repo.path}:${menu.x}:${menu.y}`}
          {...menu}
          label={`仓库操作 · ${menu.repo.name}`}
          actions={menuActions(menu.repo)}
          onClose={() => setMenu(null)}
        />
      )}
      {toolsOpen && <ToolSettingsDialog onClose={() => setToolsOpen(false)} />}
      {adding && (
        <Dialog label="添加本地仓库" onClose={closeAdd}>
          <form
            className="modal"
            onClick={(e) => e.stopPropagation()}
            onSubmit={(e) => {
              e.preventDefault();
              if (desktop) void addPath(pathInput, true);
              else setAddError("请在桌面应用中添加仓库");
            }}
          >
            <h2>添加本地仓库</h2>
            <p>输入仓库或 worktree 的完整路径。</p>
            <input
              autoFocus
              disabled={addSubmitting}
              aria-label="仓库完整路径"
              aria-invalid={Boolean(addError)}
              aria-describedby={addError ? "add-repo-error" : undefined}
              placeholder="输入本地 Git 仓库目录"
              value={pathInput}
              onChange={(e) => {
                setPathInput(e.target.value);
                setAddError("");
              }}
            />
            {addError && (
              <p id="add-repo-error" className="error-text" role="alert">
                {addError}
              </p>
            )}
            <div className="modal-actions">
              <button type="button" onClick={closeAdd}>
                取消
              </button>
              <button
                className="primary"
                disabled={addSubmitting || !pathInput.trim()}
              >
                添加仓库
              </button>
            </div>
          </form>
        </Dialog>
      )}
      {branchRepo && (
        <BranchDialog
          key={`${branchRepo.path}:${branchRevision}`}
          repo={branchRepo}
          onClose={() => setBranchRepo(null)}
          onSwitch={(reference, create) =>
            changeBranch(branchRepo, reference, create)
          }
        />
      )}
      {groupsOpen && (
        <RepoGroupsDialog
          groups={groups}
          onEdit={editGroups}
          onClose={() => setGroupsOpen(false)}
        />
      )}
      {movePaths && (
        <MoveReposDialog
          paths={movePaths}
          groups={groups}
          onEdit={editGroups}
          onClose={() => setMovePaths(null)}
        />
      )}
      {fetchChoice && (
        <Dialog label="选择获取远端" onClose={() => setFetchChoice(null)}>
          <div className="modal" onClick={(e) => e.stopPropagation()}>
            <h2>选择获取远端 · {fetchChoice.repo.name}</h2>
            <p>
              仅更新远程跟踪引用，不拉取或推送。无远端时请先用外部工具添加
              remote。
            </p>
            {fetchChoice.remotes.map((remote) => (
              <button
                key={remote}
                onClick={() => {
                  const target = fetchChoice.repo;
                  setFetchChoice(null);
                  void runSync(target, true, remote);
                }}
              >
                {remote}
              </button>
            ))}
            <button onClick={() => setFetchChoice(null)}>关闭</button>
          </div>
        </Dialog>
      )}
      {issuesOpen && (
        <AttentionDialog
          key={issuesPath ?? "all"}
          repos={repos.filter((r) =>
            issuesPath ? r.path === issuesPath : isIssue(r),
          )}
          statuses={statuses}
          failures={failures}
          initialPath={issuesPath}
          onClose={() => setIssuesOpen(false)}
          onSelect={(target) => {
            setSelected(target.path);
            setQuery("");
            setFilter(true);
            setIssuesOpen(false);
          }}
        />
      )}
      {remove && (
        <Dialog label="移除仓库" onClose={closeRemove}>
          <div
            role="dialog"
            aria-modal="true"
            aria-label="移除仓库"
            className="modal"
            onClick={(e) => e.stopPropagation()}
          >
            <h2>移除 {remove.name}？</h2>
            <p>仅从 SourceNeo 列表移除，磁盘上的仓库和所有改动都会保留。</p>
            {removeError && (
              <p className="error-text" role="alert">
                {removeError}
              </p>
            )}
            <div className="modal-actions">
              <button onClick={closeRemove}>取消</button>
              <button
                className="danger"
                disabled={removeSubmitting}
                onClick={() => void removeRepo(remove)}
              >
                从列表移除
              </button>
            </div>
          </div>
        </Dialog>
      )}
    </main>
  );
}
