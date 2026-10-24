import { useEffect, useState } from "react";
import { api } from "./api";
import { Dialog } from "./Dialog";
import { CommitDetails } from "./CommitDetails";
import { errorText, type Commit, type Comparison, type Repo } from "./types";
import { RefreshCw, X } from "lucide-react";
import { IconButton } from "./IconButton";
export function CompareDialog({
  repo,
  onClose,
}: {
  repo: Repo;
  onClose: () => void;
}) {
  const [comparison, setComparison] = useState<Comparison | null>(null),
    [error, setError] = useState("");
  const [selected, setSelected] = useState<Commit | null>(null),
    [file, setFile] = useState("");
  const [patch, setPatch] = useState(""),
    [patchError, setPatchError] = useState(""),
    [loading, setLoading] = useState(false);
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let active = true;
    setComparison(null);
    setError("");
    setSelected(null);
    setFile("");
    void api
      .compare(repo.path)
      .then((r) => {
        if (active) setComparison(r);
      })
      .catch((e) => {
        if (active) setError(errorText(e));
      });
    return () => {
      active = false;
    };
  }, [repo.path, revision]);
  useEffect(() => {
    let active = true;
    setPatch("");
    setPatchError("");
    setLoading(Boolean(file));
    if (comparison && file)
      void api
        .compareDiff(repo.path, comparison.head, comparison.target, file)
        .then((p) => {
          if (active) setPatch(p);
        })
        .catch((e) => {
          if (active) setPatchError(errorText(e));
        })
        .finally(() => {
          if (active) setLoading(false);
        });
    return () => {
      active = false;
    };
  }, [repo.path, comparison, file]);
  return (
    <Dialog label="与上游比较" onClose={onClose}>
      <div className="modal feature-modal wide-modal">
        <h2>与上游比较 · {repo.name}</h2>
        <p>使用本地已知的上游引用，不自动联网。需要最新远端信息时先 抓取。</p>
        {error ? (
          <p role="alert" className="error-text">
            {error}
          </p>
        ) : !comparison ? (
          <p role="status">正在比较…</p>
        ) : (
          <>
            <p>
              HEAD {comparison.head.slice(0, 8)} ↔ {comparison.upstream}{" "}
              {comparison.target.slice(0, 8)}
            </p>
            <div className="comparison-columns">
              {(
                [
                  ["本地独有", comparison.local],
                  ["上游独有", comparison.remote],
                ] as const
              ).map(([label, history]) => (
                <section key={label}>
                  <h3>
                    {label} · {history.commits.length}
                    {history.hasMore ? "+" : ""}
                  </h3>
                  <div className="commit-results">
                    {history.commits.map((c) => (
                      <button
                        key={c.hash}
                        onClick={() => {
                          setSelected(c);
                          setFile("");
                        }}
                      >
                        <code>{c.hash.slice(0, 8)}</code>
                        <span>{c.subject}</span>
                      </button>
                    ))}
                  </div>
                  {history.hasMore && <small>仅展示前 100 条独有提交。</small>}
                  {!history.commits.length && <p>无</p>}
                </section>
              ))}
            </div>
            <h3>文件差异 · 上游 → 当前 HEAD（{comparison.files.length}）</h3>
            <div className="compare-files">
              {comparison.files.map((f) => (
                <button
                  key={f.path}
                  aria-pressed={file === f.path}
                  onClick={() => {
                    setFile(f.path);
                    setSelected(null);
                  }}
                >
                  {f.status} · {f.oldPath ? `${f.oldPath} → ` : ""}
                  {f.path}
                </button>
              ))}
            </div>
            {file && (
              <section aria-label="分支文件差异">
                <strong>{file}</strong>
                {loading ? (
                  <p>正在读取 diff…</p>
                ) : patchError ? (
                  <p role="alert" className="error-text">
                    {patchError}
                  </p>
                ) : (
                  <>
                    <pre className="comparison-patch">
                      {patch.split("\n").slice(0, 2000).join("\n") ||
                        "无文本差异"}
                    </pre>
                    {patch.split("\n").length > 2000 && (
                      <p>仅显示前 2,000 行。</p>
                    )}
                  </>
                )}
              </section>
            )}
          </>
        )}
        {selected && (
          <CommitDetails
            key={selected.hash}
            repoPath={repo.path}
            commit={selected}
            onClose={() => setSelected(null)}
          />
        )}
        <div className="modal-actions">
          <button onClick={onClose}>
            <X size={14} />
            关闭
          </button>
          <IconButton
            label="刷新比较"
            icon={RefreshCw}
            onClick={() => setRevision((n) => n + 1)}
          />
        </div>
      </div>
    </Dialog>
  );
}
