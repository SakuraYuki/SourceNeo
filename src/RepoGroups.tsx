import { useEffect, useRef, useState } from "react";
import { Dialog } from "./Dialog";
import { errorText, type GroupEdit, type RepoGroup } from "./types";
import { Pencil, Trash2 } from "lucide-react";
import { IconButton } from "./IconButton";
import { PopConfirm } from "./PopConfirm";

export function RepoCheckbox({
  paths,
  checked,
  onToggle,
  label,
}: {
  paths: string[];
  checked: Set<string>;
  onToggle: (paths: string[], shiftKey: boolean) => void;
  label: string;
}) {
  const ref = useRef<HTMLInputElement>(null);
  const count = paths.filter((p) => checked.has(p)).length;
  useEffect(() => {
    if (ref.current)
      ref.current.indeterminate = count > 0 && count < paths.length;
  }, [count, paths.length]);
  return (
    <input
      ref={ref}
      className="repo-checkbox"
      type="checkbox"
      aria-label={label}
      checked={paths.length > 0 && count === paths.length}
      disabled={!paths.length}
      onChange={(e) =>
        onToggle(paths, (e.nativeEvent as MouseEvent).shiftKey === true)
      }
    />
  );
}

export function RepoGroupsDialog({
  groups,
  onEdit,
  onClose,
}: {
  groups: RepoGroup[];
  onEdit: (edit: GroupEdit) => Promise<void>;
  onClose: () => void;
}) {
  const [editing, setEditing] = useState<string | null>(null);
  const [name, setName] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const [removing, setRemoving] = useState<{
    name: string;
    origin: HTMLElement;
  } | null>(null);
  const guard = useRef(false);
  async function submit(edit: GroupEdit) {
    if (guard.current) return;
    guard.current = true;
    setPending(true);
    setError("");
    try {
      await onEdit(edit);
      setEditing(null);
      setName("");
      return true;
    } catch (e) {
      setError(errorText(e));
      return false;
    } finally {
      guard.current = false;
      setPending(false);
    }
  }
  return (
    <Dialog label="管理仓库分组" onClose={onClose}>
      <div className="modal" onClick={(e) => e.stopPropagation()}>
        <h2>管理仓库分组</h2>
        <p>每个仓库归属一个分组。删除分组后，仓库回到“未分组”。</p>
        <div className="group-list">
          {groups.map((g) => (
            <div className="group-editor-row" key={g.name}>
              <strong title={g.name}>{g.name}</strong>
              <span className="muted">{g.paths.length}</span>
              <IconButton
                disabled={pending}
                label={`重命名分组 ${g.name}`}
                icon={Pencil}
                onClick={() => {
                  setEditing(g.name);
                  setName(g.name);
                  setError("");
                }}
              />
              <IconButton
                className="danger-icon"
                disabled={pending}
                label={`删除分组 ${g.name}`}
                icon={Trash2}
                aria-haspopup="dialog"
                onClick={(event) => {
                  setError("");
                  setRemoving({ name: g.name, origin: event.currentTarget });
                }}
              />
            </div>
          ))}
        </div>
        <form
          onSubmit={(e) => {
            e.preventDefault();
            void submit(
              editing === null
                ? { action: "create", name }
                : { action: "rename", group: editing, name },
            );
          }}
        >
          <label htmlFor="repo-group-name">
            {editing === null ? "新分组名称" : `重命名“${editing}”`}
          </label>
          <input
            id="repo-group-name"
            value={name}
            maxLength={80}
            disabled={pending}
            onChange={(e) => setName(e.target.value)}
            placeholder="例如：工作项目"
          />
          {error && !removing && (
            <p className="error-text" role="alert">
              {error}
            </p>
          )}
          <div className="modal-actions">
            <button type="button" onClick={onClose}>
              关闭
            </button>
            {editing !== null && (
              <button
                type="button"
                disabled={pending}
                onClick={() => {
                  setEditing(null);
                  setName("");
                }}
              >
                取消重命名
              </button>
            )}
            <button className="primary" disabled={pending || !name.trim()}>
              {editing === null ? "创建分组" : "保存名称"}
            </button>
          </div>
        </form>
        {removing && (
          <PopConfirm
            origin={removing.origin}
            label="删除分组确认"
            title={`删除分组 ${removing.name}？`}
            description="组内仓库会回到“未分组”，磁盘上的仓库和改动都会保留。"
            confirmText="删除分组"
            busy={pending}
            error={error}
            onClose={() => setRemoving(null)}
            onConfirm={() => {
              void submit({ action: "remove", group: removing.name }).then(
                (success) => {
                  if (success) setRemoving(null);
                },
              );
            }}
          />
        )}
      </div>
    </Dialog>
  );
}

export function MoveReposDialog({
  groups,
  paths,
  onEdit,
  onClose,
}: {
  groups: RepoGroup[];
  paths: string[];
  onEdit: (edit: GroupEdit) => Promise<void>;
  onClose: () => void;
}) {
  const [target, setTarget] = useState("");
  const [error, setError] = useState("");
  const [pending, setPending] = useState(false);
  const guard = useRef(false);
  const mounted = useRef(true);
  useEffect(() => {
    mounted.current = true;
    return () => {
      mounted.current = false;
    };
  }, []);
  return (
    <Dialog label="移动仓库到分组" onClose={onClose}>
      <form
        className="modal"
        onClick={(e) => e.stopPropagation()}
        onSubmit={async (e) => {
          e.preventDefault();
          if (guard.current) return;
          guard.current = true;
          setPending(true);
          setError("");
          try {
            await onEdit({ action: "move", paths, group: target || null });
            if (mounted.current) onClose();
          } catch (e) {
            setError(errorText(e));
          } finally {
            guard.current = false;
            setPending(false);
          }
        }}
      >
        <h2>移动 {paths.length} 个仓库到分组</h2>
        <label htmlFor="move-repo-group">目标分组</label>
        <select
          id="move-repo-group"
          value={target}
          disabled={pending}
          onChange={(e) => setTarget(e.target.value)}
        >
          <option value="">未分组</option>
          {groups.map((g) => (
            <option key={g.name} value={g.name}>
              {g.name}
            </option>
          ))}
        </select>
        {error && (
          <p className="error-text" role="alert">
            {error}
          </p>
        )}
        <div className="modal-actions">
          <button type="button" onClick={onClose}>
            取消
          </button>
          <button className="primary" disabled={pending}>
            移动仓库
          </button>
        </div>
      </form>
    </Dialog>
  );
}
