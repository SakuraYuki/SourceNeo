import { useEffect, useLayoutEffect, useMemo, useRef, useState } from "react";
import { GitCommitHorizontal, ChevronDown, GitBranch } from "lucide-react";
import type { Commit, History } from "./types";
import { layoutGraph, palette, segmentPath } from "./graph";
import { CommitDetails } from "./CommitDetails";
const ROW = 32;
function refLabel(ref: string) {
  return ref.replace(/^refs\/(heads|remotes|tags)\//, "");
}
export function HistoryView({
  repoPath,
  history,
  loading,
  error,
  onMore,
  onRetry,
}: {
  repoPath: string;
  history: History | null;
  loading: boolean;
  error: string;
  onMore: () => void;
  onRetry: () => void;
}) {
  const container = useRef<HTMLDivElement>(null);
  const [scroll, setScroll] = useState(0);
  const [height, setHeight] = useState(400);
  const [selected, setSelected] = useState<Commit | null>(null);
  const rows = useMemo(() => layoutGraph(history?.commits ?? []), [history]);
  const graphWidth = Math.max(64, ...rows.map((r) => r.width * 18 + 16));
  const showRows = !error && rows.length > 0;
  useEffect(() => {
    if (!container.current) return;
    const observer = new ResizeObserver((e) =>
      setHeight(e[0].contentRect.height),
    );
    observer.observe(container.current);
    return () => observer.disconnect();
  }, [showRows]);
  useLayoutEffect(() => {
    setSelected(null);
    setScroll(0);
    if (container.current) container.current.scrollTop = 0;
  }, [repoPath, history?.commits[0]?.hash, showRows]);
  const start = Math.max(0, Math.floor(scroll / ROW) - 8),
    end = Math.min(rows.length, Math.ceil((scroll + height) / ROW) + 8);
  return (
    <div
      className="history-content"
      style={{ "--commit-row-height": `${ROW}px` } as React.CSSProperties}
    >
      {history?.shallow && (
        <div className="inline-note">浅克隆仓库 · 历史在克隆边界截断</div>
      )}
      {error ? (
        <div className="history-empty">
          <p className="error-text">{error}</p>
          <button onClick={onRetry}>重试加载</button>
        </div>
      ) : !rows.length ? (
        <div className="history-empty">
          <GitCommitHorizontal size={30} />
          <p>{loading ? "正在读取提交历史…" : "暂无提交历史"}</p>
          <span>提交后，这里会展示真实的分支与合并关系</span>
        </div>
      ) : (
        <>
          <div
            className="history-columns"
            style={{
              gridTemplateColumns: `${graphWidth}px minmax(200px,1fr) 130px 90px`,
            }}
          >
            <span>提交图</span>
            <span>提交 / 分支</span>
            <span>作者</span>
            <span>日期</span>
          </div>
          <div
            className="history-scroll"
            ref={container}
            onScroll={(e) => setScroll(e.currentTarget.scrollTop)}
          >
            <div
              style={{
                height: rows.length * ROW,
                position: "relative",
                minWidth: graphWidth + 470,
              }}
            >
              {rows.slice(start, end).map((row, i) => (
                <button
                  key={row.commit.hash}
                  className={`commit-row ${selected?.hash === row.commit.hash ? "selected" : ""}`}
                  style={{
                    top: (start + i) * ROW,
                    gridTemplateColumns: `${graphWidth}px minmax(200px,1fr) 130px 90px`,
                  }}
                  onClick={() => setSelected(row.commit)}
                  title={`${row.commit.hash}\n父提交：${row.commit.parents.join(", ") || "无（根提交）"}\n${row.commit.subject}`}
                >
                  <svg width={graphWidth} height={ROW} aria-hidden="true">
                    {row.segments.map((s, j) => (
                      <path
                        key={j}
                        d={segmentPath(s, ROW)}
                        fill="none"
                        stroke={palette[s.color]}
                        strokeWidth={1.8}
                      />
                    ))}
                    <circle
                      cx={16 + row.lane * 18}
                      cy={ROW / 2}
                      r={row.commit.parents.length > 1 ? 5 : 4}
                      fill="var(--panel)"
                      stroke={palette[row.color]}
                      strokeWidth={2}
                    />
                    {row.commit.parents.length > 1 && (
                      <circle
                        cx={16 + row.lane * 18}
                        cy={ROW / 2}
                        r={1.8}
                        fill={palette[row.color]}
                      />
                    )}
                  </svg>
                  <span className="commit-subject">
                    {row.commit.refs.map((ref) => (
                      <span
                        key={ref}
                        className={`ref ${ref === "HEAD" ? "head" : ref.startsWith("refs/remotes/") ? "remote" : ref.startsWith("refs/tags/") ? "tag" : ""}`}
                      >
                        {ref !== "HEAD" && <GitBranch size={10} />}{" "}
                        {refLabel(ref)}
                      </span>
                    ))}
                    <span className="subject-text">{row.commit.subject}</span>
                    <code>{row.commit.hash.slice(0, 7)}</code>
                  </span>
                  <span className="muted ellipsis">{row.commit.author}</span>
                  <span className="muted mono">
                    {row.commit.date.slice(0, 10)}
                  </span>
                </button>
              ))}
            </div>
            <div className="load-more">
              {history?.hasMore && rows.length < 5000 ? (
                <button disabled={loading} onClick={onMore}>
                  <ChevronDown size={14} />
                  {loading ? "正在加载…" : "加载更早的 100 条提交"}
                </button>
              ) : (
                <span>
                  {history?.hasMore
                    ? "已显示 5,000 条提交，较早历史未加载"
                    : "已到达当前可见历史的起点"}
                </span>
              )}
            </div>
          </div>
        </>
      )}
      {selected && (
        <CommitDetails
          key={`${repoPath}:${selected.hash}`}
          repoPath={repoPath}
          commit={selected}
          onClose={() => setSelected(null)}
        />
      )}
    </div>
  );
}
