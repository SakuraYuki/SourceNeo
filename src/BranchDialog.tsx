import { useEffect, useRef, useState } from "react";
import { GitBranch, X } from "lucide-react";
import { api } from "./api";
import { Dialog } from "./Dialog";
import { errorText, type Branch, type Repo } from "./types";

export function BranchDialog({
  repo,
  onClose,
  onSwitch,
}: {
  repo: Repo;
  onClose: () => void;
  onSwitch: (reference: string, create: boolean) => Promise<void>;
}) {
  const [branches, setBranches] = useState<Branch[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const [name, setName] = useState("");
  const [reload, setReload] = useState(0);
  const active = useRef(true);
  const guard = useRef(false);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    void api
      .branches(repo.path)
      .then((result) => {
        if (!cancelled) setBranches(result);
      })
      .catch((e) => {
        if (!cancelled)
          setError((previous) =>
            previous
              ? `${previous}\n分支列表读取失败：${errorText(e)}`
              : errorText(e),
          );
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [repo.path, reload]);
  async function choose(reference: string, create = false) {
    if (guard.current) return;
    guard.current = true;
    setPending(true);
    setError("");
    try {
      await onSwitch(reference, create);
      if (active.current) onClose();
    } catch (e) {
      if (active.current) {
        setError(errorText(e));
        setReload((n) => n + 1);
      }
    } finally {
      guard.current = false;
      if (active.current) setPending(false);
    }
  }
  return (
    <Dialog label="切换分支" onClose={onClose}>
      <div className="modal branch-modal" onClick={(e) => e.stopPropagation()}>
        <div className="inspector-heading">
          <h2>切换分支 · {repo.name}</h2>
          <button
            className="icon-btn"
            aria-label="关闭分支窗口"
            onClick={onClose}
          >
            <X size={16} />
          </button>
        </div>
        <p>
          仅使用本地已有引用。选择远程分支会创建同名本地跟踪分支；存在未提交改动时暂停操作。
        </p>
        {error && (
          <p role="alert" className="error-text">
            {error}
          </p>
        )}
        {loading ? (
          <p>正在读取分支…</p>
        ) : (
          <>
            <div className="branch-list">
              {[false, true].map((remote) => (
                <section key={String(remote)}>
                  <h3>{remote ? "远程分支" : "本地分支"}</h3>
                  {branches
                    .filter((b) => b.remote === remote)
                    .map((b) => (
                      <button
                        key={b.reference}
                        aria-label={`${b.name} ${b.current ? "当前" : remote ? "创建并切换" : "切换"}`}
                        disabled={pending || b.current}
                        onClick={() => void choose(b.reference)}
                        title={b.name}
                      >
                        <GitBranch size={14} />
                        <span className="ellipsis">{b.name}</span>
                        <small>
                          {b.current ? "当前" : remote ? "创建并切换" : "切换"}
                        </small>
                      </button>
                    ))}
                  {!branches.some((b) => b.remote === remote) && (
                    <p>暂无{remote ? "远程" : "本地"}分支</p>
                  )}
                </section>
              ))}
            </div>
            <form
              className="new-branch"
              onSubmit={(e) => {
                e.preventDefault();
                void choose(name, true);
              }}
            >
              <label htmlFor="new-branch-name">从当前 HEAD 新建分支</label>
              <div>
                <input
                  id="new-branch-name"
                  value={name}
                  disabled={pending}
                  onChange={(e) => setName(e.target.value)}
                  placeholder="feature/new-branch"
                />
                <button disabled={pending || !name}>新建并切换</button>
              </div>
            </form>
          </>
        )}
        <div className="modal-actions">
          <button
            disabled={pending || loading}
            onClick={() => {
              setError("");
              setReload((n) => n + 1);
            }}
          >
            重新加载
          </button>
          <button onClick={onClose}>取消</button>
        </div>
      </div>
    </Dialog>
  );
}
