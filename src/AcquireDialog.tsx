import { useRef, useState } from "react";
import { api, pooled } from "./api";
import { Dialog } from "./Dialog";
import { errorText, type Repo, type ScanResult } from "./types";

export function AcquireDialog({
  mode,
  onClose,
  onAdded,
}: {
  mode: "clone" | "scan";
  onClose: () => void;
  onAdded: (repos: Repo[]) => Promise<void>;
}) {
  const [parent, setParent] = useState("");
  const [url, setUrl] = useState("");
  const nameEdited = useRef(false);
  const [name, setName] = useState("");
  const [depth, setDepth] = useState(3);
  const [busy, setBusy] = useState(false);
  const guard = useRef(false);
  const cloneId = useRef("");
  const [cancelling, setCancelling] = useState(false);
  const [error, setError] = useState("");
  const [scan, setScan] = useState<ScanResult | null>(null);
  const [checked, setChecked] = useState(new Set<string>());
  const [results, setResults] = useState<string[]>([]);
  async function choose() {
    try {
      const path = await api.pickFolder();
      if (typeof path === "string") {
        setParent(path);
        setScan(null);
      }
    } catch (e) {
      setError(errorText(e));
    }
  }
  async function submit() {
    if (guard.current) return;
    guard.current = true;
    setBusy(true);
    setError("");
    setResults([]);
    try {
      if (mode === "clone") {
        cloneId.current = crypto.randomUUID();
        setCancelling(false);
        const repo = await api.clone(url.trim(), parent, name, cloneId.current);
        await onAdded([repo]);
        onClose();
      } else {
        const result = await api.scan(parent, depth);
        setScan(result);
        setChecked(new Set(result.repos.map((r) => r.path)));
      }
    } catch (e) {
      setError(errorText(e));
    } finally {
      guard.current = false;
      setBusy(false);
    }
  }
  async function importSelected() {
    if (!scan || guard.current) return;
    guard.current = true;
    setBusy(true);
    setError("");
    setResults([]);
    const added: Repo[] = [];
    const failures = new Set<string>();
    try {
      await pooled(
        scan.repos.filter((r) => checked.has(r.path)),
        async (r) => {
          try {
            added.push(await api.add(r.path));
            setResults((old) => [...old, `已添加：${r.name}`]);
          } catch (e) {
            failures.add(r.path);
            setResults((old) => [...old, `${r.name}：${errorText(e)}`]);
          }
        },
      );
      await onAdded(added);
      setChecked(failures);
    } catch (e) {
      setError(errorText(e));
    } finally {
      guard.current = false;
      setBusy(false);
    }
  }
  const title = mode === "clone" ? "克隆远程仓库" : "扫描并导入仓库";
  return (
    <Dialog
      label={title}
      onClose={() => {
        if (!busy) onClose();
      }}
    >
      <form
        className="modal feature-modal"
        onSubmit={(e) => {
          e.preventDefault();
          void submit();
        }}
      >
        <h2>{title}</h2>
        {mode === "clone" ? (
          <>
            <p>
              支持 HTTPS 和 SSH。认证使用本机 Git 凭据助手或 SSH
              agent；请勿在地址中填写令牌。
            </p>
            <label>
              远程地址
              <input
                autoFocus
                aria-label="远程仓库地址"
                placeholder="https://host/owner/repository.git"
                value={url}
                disabled={busy}
                onChange={(e) => {
                  setUrl(e.target.value);
                  if (!nameEdited.current) {
                    const suggestion =
                      e.target.value
                        .trim()
                        .split(/[/:]/)
                        .at(-1)
                        ?.replace(/\.git$/, "") ?? "";
                    if (/^[\w.-]+$/.test(suggestion)) setName(suggestion);
                  }
                }}
              />
            </label>
          </>
        ) : (
          <p>
            查找本地仓库与
            worktree，勾选后导入。跳过依赖目录和目录链接；找到仓库后不继续深入其内部。
          </p>
        )}
        <label>
          {mode === "clone" ? "保存到父目录" : "扫描父目录"}
          <div className="input-row">
            <input
              aria-label="目标父目录"
              value={parent}
              disabled={busy}
              onChange={(e) => {
                setParent(e.target.value);
                setScan(null);
              }}
              placeholder="选择本地目录"
            />
            <button
              type="button"
              disabled={busy}
              aria-label="选择父目录"
              onClick={() => void choose()}
            >
              浏览…
            </button>
          </div>
        </label>
        {mode === "clone" ? (
          <label>
            新文件夹名称
            <input
              aria-label="新文件夹名称"
              value={name}
              disabled={busy}
              onChange={(e) => {
                nameEdited.current = true;
                setName(e.target.value);
              }}
            />
          </label>
        ) : (
          <label>
            扫描深度
            <select
              aria-label="扫描深度"
              value={depth}
              disabled={busy}
              onChange={(e) => {
                setDepth(Number(e.target.value));
                setScan(null);
              }}
            >
              {[1, 2, 3, 4, 5, 6].map((n) => (
                <option key={n} value={n}>
                  {n} 层
                </option>
              ))}
            </select>
          </label>
        )}
        {busy && (
          <p role="status">
            {mode === "clone"
              ? "正在克隆并加入列表，大型仓库可能需要数分钟。取消后保留目标目录，不自动删除。"
              : "正在处理，请稍候…"}
          </p>
        )}
        {error && (
          <p className="error-text" role="alert">
            {error}
          </p>
        )}
        {scan && (
          <section className="scan-results" aria-label="扫描结果">
            <p>
              发现 {scan.repos.length} 个仓库
              {scan.truncated ? "（已达扫描上限，请缩小范围）" : ""}
            </p>
            <button
              type="button"
              disabled={busy}
              onClick={() =>
                setChecked(
                  checked.size === scan.repos.length
                    ? new Set()
                    : new Set(scan.repos.map((r) => r.path)),
                )
              }
            >
              {checked.size === scan.repos.length ? "取消全选" : "全选结果"}
            </button>
            {scan.repos.map((r) => (
              <label className="scan-item" key={r.path}>
                <input
                  type="checkbox"
                  checked={checked.has(r.path)}
                  disabled={busy}
                  onChange={(e) =>
                    setChecked((old) => {
                      const next = new Set(old);
                      if (e.target.checked) next.add(r.path);
                      else next.delete(r.path);
                      return next;
                    })
                  }
                />
                <span>
                  <strong>{r.name}</strong>
                  <small>{r.path}</small>
                </span>
              </label>
            ))}
            {scan.warnings.map((w, i) => (
              <p className="error-text" key={i}>
                {w}
              </p>
            ))}
          </section>
        )}
        {results.length > 0 && (
          <ul className="import-results" aria-live="polite">
            {results.map((r, i) => (
              <li key={i}>{r}</li>
            ))}
          </ul>
        )}
        <div className="modal-actions">
          {mode === "clone" && busy && (
            <button
              type="button"
              disabled={cancelling}
              onClick={() => {
                setCancelling(true);
                void api.cancelClone(cloneId.current).catch((e) => {
                  setError(errorText(e));
                  setCancelling(false);
                });
              }}
            >
              {cancelling ? "正在取消…" : "取消克隆"}
            </button>
          )}
          <button type="button" disabled={busy} onClick={onClose}>
            关闭
          </button>
          <button
            disabled={
              busy || !parent || (mode === "clone" && (!url.trim() || !name))
            }
          >
            {mode === "clone" ? "克隆并添加" : "扫描目录"}
          </button>
          {mode === "scan" && scan && (
            <button
              type="button"
              className="primary"
              disabled={busy || !checked.size}
              onClick={() => void importSelected()}
            >
              导入所选（{checked.size}）
            </button>
          )}
        </div>
      </form>
    </Dialog>
  );
}
