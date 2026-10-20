import { useEffect, useState } from "react";
import { X } from "lucide-react";
import { api } from "./api";
import { Dialog } from "./Dialog";
import { attention, workingFileLabel, workingFileKind } from "./attention";
import {
  errorText,
  type ChangedFile,
  type Repo,
  type RepoStatus,
} from "./types";

function WorkingFiles({ path }: { path: string }) {
  const [files, setFiles] = useState<ChangedFile[]>([]);
  const [loading, setLoading] = useState(true);
  const [error, setError] = useState("");
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setError("");
    void api
      .workingChanges(path)
      .then((result) => {
        if (!cancelled) setFiles(result);
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
  }, [path, retry]);
  return (
    <div className="working-files">
      {loading ? (
        <p>正在读取未提交文件…</p>
      ) : error ? (
        <p role="alert" className="error-text">
          {error}
        </p>
      ) : files.length ? (
        <>
          <p>当前本地共有 {files.length} 个变更文件</p>
          <ul>
            {files.map((file) => (
              <li key={`${file.status}:${file.path}`}>
                <span className="working-label">
                  {workingFileLabel(file.status)}
                </span>
                <span
                  title={
                    file.oldPath ? `${file.oldPath} → ${file.path}` : file.path
                  }
                >
                  {file.oldPath && <small>{file.oldPath} → </small>}
                  {file.path}
                </span>
                <span
                  className="working-kind"
                  title={`Git 状态：${file.status}`}
                >
                  {workingFileKind(file.status)}
                </span>
              </li>
            ))}
          </ul>
        </>
      ) : (
        <p>当前没有未提交文件；请刷新主界面状态。</p>
      )}
      <button
        className="text-btn"
        disabled={loading}
        onClick={() => setRetry((n) => n + 1)}
      >
        {error ? "重试文件列表" : "刷新文件列表"}
      </button>
    </div>
  );
}

export function AttentionDialog({
  repos,
  statuses,
  failures,
  initialPath,
  onClose,
  onSelect,
}: {
  repos: Repo[];
  statuses: Record<string, RepoStatus>;
  failures: Record<string, string>;
  initialPath?: string;
  onClose: () => void;
  onSelect: (repo: Repo) => void;
}) {
  const showFiles = Boolean(initialPath && statuses[initialPath]?.changed);
  const [expanded, setExpanded] = useState(showFiles ? initialPath : "");
  return (
    <Dialog label="需关注详情" onClose={onClose}>
      <div
        className="modal attention-modal"
        onClick={(e) => e.stopPropagation()}
      >
        <div className="inspector-heading">
          <h2>
            {initialPath
              ? showFiles
                ? "未提交文件"
                : "仓库处理建议"
              : `需关注的仓库（${repos.length}）`}
          </h2>
          <button
            className="icon-btn"
            aria-label="关闭需关注详情"
            onClick={onClose}
          >
            <X size={16} />
          </button>
        </div>
        <div className="attention-items">
          {!repos.length && <p>当前没有需要关注的仓库。</p>}
          {repos.map((repo) => {
            const status = statuses[repo.path];
            const info = attention(status, failures[repo.path]);
            return (
              <section
                key={repo.path}
                className="attention-item"
                aria-label={repo.name}
              >
                <div className="attention-item-heading">
                  <strong title={repo.path}>{repo.name}</strong>
                  <span>{info.label}</span>
                </div>
                <p className="attention-branch" title={repo.path}>
                  分支：{status?.branch ?? "未知"}
                </p>
                <p>{info.reason}</p>
                <p className="attention-action">下一步：{info.action}</p>
                <div className="attention-actions">
                  <button className="text-btn" onClick={() => onSelect(repo)}>
                    定位仓库
                  </button>
                  {!!status?.changed && (
                    <button
                      className="text-btn"
                      aria-expanded={expanded === repo.path}
                      onClick={() =>
                        setExpanded(expanded === repo.path ? "" : repo.path)
                      }
                    >
                      {expanded === repo.path
                        ? "收起文件清单"
                        : `查看 ${status.changed} 个未提交文件`}
                    </button>
                  )}
                </div>
                {expanded === repo.path && <WorkingFiles path={repo.path} />}
              </section>
            );
          })}
        </div>
        <div className="modal-actions">
          <button onClick={onClose}>关闭</button>
        </div>
      </div>
    </Dialog>
  );
}
