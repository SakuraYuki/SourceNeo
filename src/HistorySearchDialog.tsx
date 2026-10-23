import { useEffect, useRef, useState } from "react";
import { api } from "./api";
import { Dialog } from "./Dialog";
import { CommitDetails } from "./CommitDetails";
import {
  errorText,
  type Commit,
  type HistoryPage,
  type HistoryQuery,
  type Repo,
} from "./types";
export function HistorySearchDialog({
  repo,
  onClose,
}: {
  repo: Repo;
  onClose: () => void;
}) {
  const [query, setQuery] = useState<HistoryQuery>({
    field: "message",
    text: "",
  });
  const [all, setAll] = useState(true);
  const [result, setResult] = useState<HistoryPage | null>(null);
  const [selected, setSelected] = useState<Commit | null>(null);
  const [busy, setBusy] = useState(false),
    [error, setError] = useState("");
  const request = useRef(0),
    executed = useRef({ query, all });
  useEffect(
    () => () => {
      request.current++;
    },
    [],
  );
  async function search(more = false) {
    const id = ++request.current;
    setBusy(true);
    setError("");
    if (!more) {
      executed.current = { query: { ...query, text: query.text.trim() }, all };
      setResult(null);
      setSelected(null);
    }
    try {
      const page = await api.historyPage(
        repo.path,
        executed.current.all,
        executed.current.query,
        more ? result?.snapshot : null,
        more ? result?.commits.length : 0,
      );
      if (id === request.current)
        setResult({
          ...page,
          commits: [...(more ? (result?.commits ?? []) : []), ...page.commits],
        });
    } catch (e) {
      if (id === request.current) setError(errorText(e));
    } finally {
      if (id === request.current) setBusy(false);
    }
  }
  return (
    <Dialog label="搜索提交" onClose={onClose}>
      <div className="modal feature-modal wide-modal">
        <h2>搜索提交 · {repo.name}</h2>
        <p>
          搜索 Git 历史，包括尚未加载的提交。SHA
          可定位任意已知提交；其他条件按所选分支范围搜索。
        </p>
        <form
          className="search-controls"
          onSubmit={(e) => {
            e.preventDefault();
            void search();
          }}
        >
          <select
            aria-label="搜索字段"
            value={query.field}
            disabled={busy}
            onChange={(e) =>
              setQuery({
                ...query,
                field: e.target.value as HistoryQuery["field"],
              })
            }
          >
            <option value="message">提交说明</option>
            <option value="author">作者</option>
            <option value="sha">SHA</option>
          </select>
          <input
            aria-label="搜索提交内容"
            autoFocus
            placeholder="关键词或提交 SHA"
            value={query.text}
            disabled={busy}
            onChange={(e) => setQuery({ ...query, text: e.target.value })}
          />
          <label>
            <input
              type="checkbox"
              checked={all}
              disabled={busy || query.field === "sha"}
              onChange={(e) => setAll(e.target.checked)}
            />
            全部分支
          </label>
          <button disabled={busy || !query.text.trim()}>搜索</button>
        </form>
        {error && (
          <p role="alert" className="error-text">
            {error}
          </p>
        )}
        {busy && <p role="status">正在搜索…</p>}
        {result && (
          <>
            <p>
              找到 {result.commits.length}
              {result.hasMore ? "+" : ""} 条
              {result.shallow ? " · 浅克隆，结果限于已有对象" : ""}
            </p>
            <div className="commit-results">
              {result.commits.map((c) => (
                <button
                  key={c.hash}
                  onClick={() => setSelected(c)}
                  aria-pressed={selected?.hash === c.hash}
                >
                  <code>{c.hash.slice(0, 8)}</code>
                  <span>{c.subject}</span>
                  <small>
                    {c.author} · {c.date.slice(0, 10)}
                  </small>
                </button>
              ))}
            </div>
            {result.hasMore && result.commits.length < 5000 && (
              <button disabled={busy} onClick={() => void search(true)}>
                加载更多搜索结果
              </button>
            )}
            {result.hasMore && result.commits.length >= 5000 && (
              <p>已达 5,000 条，请缩小搜索范围。</p>
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
          <button onClick={onClose}>关闭</button>
        </div>
      </div>
    </Dialog>
  );
}
