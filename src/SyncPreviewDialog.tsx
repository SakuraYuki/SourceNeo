import { useEffect, useState } from "react";
import { api } from "./api";
import { Dialog } from "./Dialog";
import { errorText, type Repo, type SyncPreview } from "./types";
export function SyncPreviewDialog({
  repo,
  busy,
  onClose,
  onSync,
}: {
  repo: Repo;
  busy: boolean;
  onClose: () => void;
  onSync: () => void;
}) {
  const [preview, setPreview] = useState<SyncPreview | null>(null);
  const [error, setError] = useState("");
  const [revision, setRevision] = useState(0);
  useEffect(() => {
    let active = true;
    setPreview(null);
    setError("");
    void api
      .preview(repo.path)
      .then((p) => {
        if (active) setPreview(p);
      })
      .catch((e) => {
        if (active) setError(errorText(e));
      });
    return () => {
      active = false;
    };
  }, [repo.path, revision]);
  return (
    <Dialog label="同步前预览" onClose={onClose}>
      <div className="modal feature-modal">
        <h2>同步前预览 · {repo.name}</h2>
        <p>
          预览不联网。实际同步会先
          Fetch，再重新校验；远端更新、冲突或被忽略文件可能改变结果。
        </p>
        {error ? (
          <p role="alert" className="error-text">
            {error}
          </p>
        ) : !preview ? (
          <p role="status">正在检查同步计划…</p>
        ) : (
          <>
            <dl className="preview-facts">
              <dt>分支</dt>
              <dd>{preview.status.branch}</dd>
              <dt>远端 / 上游</dt>
              <dd>
                {preview.remote ?? "未配置"} /{" "}
                {preview.status.upstream ?? "未配置"}
              </dd>
              <dt>有效策略</dt>
              <dd>{preview.policy || "不可用"}</dd>
              <dt>最后成功获取</dt>
              <dd>
                {preview.status.lastFetched
                  ? new Date(preview.status.lastFetched * 1000).toLocaleString(
                      "zh-CN",
                    )
                  : "从未获取，远端状态未知"}
              </dd>
              <dt>当前已知差异</dt>
              <dd>
                本地独有 {preview.status.ahead} · 上游独有{" "}
                {preview.status.behind}
              </dd>
            </dl>
            {preview.blockers.length > 0 && (
              <div className="inline-note">
                <strong>需要先处理</strong>
                <ul>
                  {preview.blockers.map((b, i) => (
                    <li key={i}>{b}</li>
                  ))}
                </ul>
              </div>
            )}
            <strong>预计步骤</strong>
            <ol>
              {preview.steps.map((s, i) => (
                <li key={i}>{s}</li>
              ))}
            </ol>
          </>
        )}
        <div className="modal-actions">
          <button onClick={onClose}>关闭</button>
          <button disabled={busy} onClick={() => setRevision((n) => n + 1)}>
            重新预览
          </button>
          <button
            className="primary"
            disabled={busy || !preview || preview.blockers.length > 0}
            onClick={onSync}
          >
            执行同步
          </button>
        </div>
      </div>
    </Dialog>
  );
}
