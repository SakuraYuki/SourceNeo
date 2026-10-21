import { useEffect, useRef, useState } from "react";
import { api } from "./api";
import { Dialog } from "./Dialog";
import { errorText } from "./types";

export function ToolSettingsDialog({ onClose }: { onClose: () => void }) {
  const [settings, setSettings] = useState<{
    terminal: string;
    editor: string;
  } | null>(null);
  const [error, setError] = useState("");
  const [busy, setBusy] = useState(false);
  const [retry, setRetry] = useState(0);
  const active = useRef(true);
  useEffect(() => {
    active.current = true;
    return () => {
      active.current = false;
    };
  }, []);
  useEffect(() => {
    let cancelled = false;
    setError("");
    void api
      .toolSettings()
      .then((value) => {
        if (!cancelled) setSettings(value);
      })
      .catch((e) => {
        if (!cancelled) setError(errorText(e));
      });
    return () => {
      cancelled = true;
    };
  }, [retry]);
  return (
    <Dialog label="外部工具设置" onClose={onClose}>
      <form
        className="modal"
        onClick={(e) => e.stopPropagation()}
        onSubmit={(e) => {
          e.preventDefault();
          if (!settings || busy) return;
          setBusy(true);
          setError("");
          void api
            .saveToolSettings(settings)
            .then(() => {
              if (active.current) onClose();
            })
            .catch((e) => {
              if (active.current) setError(errorText(e));
            })
            .finally(() => {
              if (active.current) setBusy(false);
            });
        }}
      >
        <h2>外部工具设置</h2>
        <p>
          留空使用系统终端或自动检测 VS Code /
          Cursor。自定义程序填写可执行文件完整路径（Windows 为
          .exe），不填命令或参数。设置保存在本机仓库配置中。
        </p>
        {settings ? (
          <>
            <label>
              终端可执行文件
              <input
                disabled={busy}
                value={settings.terminal}
                onChange={(e) =>
                  setSettings({ ...settings, terminal: e.target.value })
                }
              />
            </label>
            <label>
              编辑器可执行文件
              <input
                disabled={busy}
                value={settings.editor}
                onChange={(e) =>
                  setSettings({ ...settings, editor: e.target.value })
                }
              />
            </label>
          </>
        ) : (
          !error && <p>正在读取设置…</p>
        )}
        {error && <p role="alert">{error}</p>}
        {!settings && error && (
          <button type="button" onClick={() => setRetry((n) => n + 1)}>
            重试读取设置
          </button>
        )}
        <div className="modal-actions">
          <button type="button" onClick={onClose}>
            取消
          </button>
          <button disabled={!settings || busy}>保存</button>
        </div>
      </form>
    </Dialog>
  );
}
