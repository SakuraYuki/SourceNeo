import { useEffect, useState } from "react";
import { api } from "./api";
import { errorText, type ChangedFile } from "./types";

export function WorkingDiff({
  path,
  file,
}: {
  path: string;
  file: ChangedFile;
}) {
  const sides =
    file.status === "??"
      ? ["untracked"]
      : [
          ...(file.status[0] !== " " ? ["staged"] : []),
          ...(file.status[1] !== " " ? ["unstaged"] : []),
        ];
  const [side, setSide] = useState(sides[0]);
  const [patch, setPatch] = useState("");
  const [error, setError] = useState("");
  const [loading, setLoading] = useState(true);
  const [retry, setRetry] = useState(0);
  useEffect(() => {
    let cancelled = false;
    setLoading(true);
    setPatch("");
    setError("");
    void api
      .workingDiff(path, file.path, side)
      .then((value) => {
        if (!cancelled) setPatch(value);
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
  }, [path, file.path, file.status, side, retry]);
  return (
    <section aria-label="未提交差异" className="working-preview">
      <strong>{file.path}</strong>
      <div>
        {sides.map((value) => (
          <button
            key={value}
            aria-pressed={side === value}
            onClick={() => setSide(value)}
          >
            {value === "staged"
              ? "已暂存 · index 对 HEAD"
              : value === "unstaged"
                ? "未暂存 · 工作区对 index"
                : "未跟踪内容"}
          </button>
        ))}
      </div>
      {loading ? (
        <p>正在读取 diff…</p>
      ) : error ? (
        <p role="alert">{error}</p>
      ) : (
        <div className="file-patch">
          <pre>
            {patch.split("\n").map((line, i) => (
              <span
                key={i}
                className={
                  line.startsWith("+")
                    ? "diff-add"
                    : line.startsWith("-")
                      ? "diff-delete"
                      : ""
                }
              >
                {line || " "}
              </span>
            ))}
          </pre>
        </div>
      )}
      <button disabled={loading} onClick={() => setRetry((n) => n + 1)}>
        重试 diff
      </button>
    </section>
  );
}
