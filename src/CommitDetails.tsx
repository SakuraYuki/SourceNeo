import { useEffect, useState } from "react";
import { X, Copy, Check } from "lucide-react";
import { api } from "./api";
import { errorText, type Commit, type CommitChanges } from "./types";

export function CommitDetails({
  repoPath,
  commit,
  onClose,
}: {
  repoPath: string;
  commit: Commit;
  onClose: () => void;
}) {
  const [changes, setChanges] = useState<CommitChanges | null>(null);
  const [file, setFile] = useState("");
  const [patch, setPatch] = useState("");
  const [error, setError] = useState("");
  const [diffError, setDiffError] = useState("");
  const [loading, setLoading] = useState(true);
  const [diffLoading, setDiffLoading] = useState(false);
  const [reload, setReload] = useState(0);
  const [diffReload, setDiffReload] = useState(0);
  const [copied, setCopied] = useState(false);
  const lines = patch.split("\n");
  useEffect(() => {
    let cancelled = false;
    setChanges(null);
    setFile("");
    setError("");
    setLoading(true);
    void api
      .changes(repoPath, commit.hash)
      .then((value) => {
        if (cancelled) return;
        setChanges(value);
        setFile(value.files[0]?.path ?? "");
      })
      .catch((e) => {
        if (!cancelled) setError(errorText(e));
      })
      .finally(() => {
        if (!cancelled) setLoading(false);
      });
    return () => {
      cancelled = true;
    };
  }, [repoPath, commit.hash, reload]);
  useEffect(() => {
    let cancelled = false;
    setPatch("");
    setDiffError("");
    setDiffLoading(Boolean(file));
    if (file) {
      void api
        .diff(repoPath, commit.hash, file)
        .then((value) => {
          if (!cancelled) setPatch(value);
        })
        .catch((e) => {
          if (!cancelled) setDiffError(errorText(e));
        })
        .finally(() => {
          if (!cancelled) setDiffLoading(false);
        });
    }
    return () => {
      cancelled = true;
    };
  }, [repoPath, commit.hash, file, diffReload]);
  return (
    <section className="commit-inspector" aria-label="提交变更详情">
      <div className="inspector-heading">
        <strong title={commit.subject}>
          {commit.hash.slice(0, 12)} · {commit.subject}
        </strong>
        <button
          className="icon-btn"
          aria-label="复制提交 SHA"
          title={copied ? "已复制" : "复制提交 SHA"}
          onClick={() => {
            void api
              .copy(commit.hash)
              .then(() => setCopied(true))
              .catch(() => setCopied(false));
          }}
        >
          {copied ? <Check size={14} /> : <Copy size={14} />}
        </button>
        <button
          className="icon-btn"
          aria-label="收起提交详情"
          onClick={onClose}
        >
          <X size={14} />
        </button>
      </div>
      <div
        className="comparison-note"
        title={`父提交：${commit.parents.join(" · ") || changes?.parent || "无"}`}
      >
        {loading
          ? "正在读取提交…"
          : commit.parents.length > 1
            ? "合并提交 · 与第一父提交比较"
            : changes?.parent
              ? "与父提交比较"
              : "根提交 · 显示全部新增文件"}
        {changes && ` · ${changes.files.length} 个文件`}
        {changes?.parent && ` · ${changes.parent.slice(0, 12)}`}
      </div>
      {loading ? (
        <p className="inspector-message">正在读取变更文件…</p>
      ) : error ? (
        <div className="inspector-message">
          <p role="alert" className="error-text">
            {error}
          </p>
          <button onClick={() => setReload((n) => n + 1)}>重试文件列表</button>
        </div>
      ) : !changes?.files.length ? (
        <p className="inspector-message">此提交没有文件变更</p>
      ) : (
        <div className="commit-files-layout">
          <div className="changed-files" role="list" aria-label="变更文件">
            {changes.files.map((item) => (
              <button
                key={item.path}
                aria-label={`${item.status[0]} ${item.oldPath ? `${item.oldPath} → ` : ""}${item.path}`}
                aria-pressed={file === item.path}
                onClick={() => setFile(item.path)}
                title={
                  item.oldPath ? `${item.oldPath} → ${item.path}` : item.path
                }
              >
                <span className={`file-status status-${item.status[0]}`}>
                  {item.status[0]}
                </span>
                <span>
                  {item.oldPath && <small>{item.oldPath} → </small>}
                  {item.path}
                </span>
              </button>
            ))}
          </div>
          <div
            className="file-patch"
            aria-label="文件差异"
            aria-busy={diffLoading}
          >
            {diffLoading ? (
              <p className="inspector-message">正在读取 diff…</p>
            ) : diffError ? (
              <div className="inspector-message">
                <p role="alert" className="error-text">
                  {diffError}
                </p>
                <button onClick={() => setDiffReload((n) => n + 1)}>
                  重试 diff
                </button>
              </div>
            ) : (
              <>
                {lines.length > 2000 && (
                  <p className="comparison-note">
                    仅预览前 2,000 行；完整差异请用外部 Git 工具查看。
                  </p>
                )}
                <pre>
                  {patch
                    ? lines.slice(0, 2000).map((line, i) => (
                        <span
                          key={i}
                          className={
                            line.startsWith("+")
                              ? "diff-add"
                              : line.startsWith("-")
                                ? "diff-delete"
                                : line.startsWith("@@")
                                  ? "diff-hunk"
                                  : ""
                          }
                        >
                          {line || " "}
                        </span>
                      ))
                    : "无文本差异（可能仅有文件模式变更）"}
                </pre>
              </>
            )}
          </div>
        </div>
      )}
    </section>
  );
}
