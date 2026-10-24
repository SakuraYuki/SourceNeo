import { useId, useLayoutEffect, useRef, useState } from "react";
import { AlertTriangle, Check, X } from "lucide-react";

/** Anchored, non-modal confirmation. Keeps the surrounding task visible. */
export function PopConfirm({
  origin,
  label,
  title,
  description,
  confirmText,
  tone = "danger",
  busy = false,
  error,
  onConfirm,
  onClose,
}: {
  origin: HTMLElement;
  label: string;
  title: string;
  description: string;
  confirmText: string;
  tone?: "warning" | "danger";
  busy?: boolean;
  error?: string;
  onConfirm: () => void;
  onClose: () => void;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  const cancel = useRef<HTMLButtonElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  const descriptionId = useId();
  const [position, setPosition] = useState({ left: 0, top: 0 });
  function dismiss() {
    close.current();
    if (origin.isConnected) origin.focus();
  }
  useLayoutEffect(() => {
    const panel = ref.current!;
    function place() {
      if (!origin.isConnected) {
        close.current();
        return;
      }
      const anchor = origin.getBoundingClientRect();
      const bounds = panel.getBoundingClientRect();
      const below = anchor.bottom + 8;
      const top =
        below + bounds.height <= window.innerHeight - 8
          ? below
          : anchor.top - bounds.height - 8;
      setPosition({
        left: Math.max(
          8,
          Math.min(
            anchor.right - bounds.width,
            window.innerWidth - bounds.width - 8,
          ),
        ),
        top: Math.max(8, Math.min(top, window.innerHeight - bounds.height - 8)),
      });
    }
    place();
    cancel.current?.focus();
    const outside = (event: PointerEvent) => {
      if (
        !panel.contains(event.target as Node) &&
        !origin.contains(event.target as Node)
      )
        close.current();
    };
    const scroll = (event: Event) => {
      if (!panel.contains(event.target as Node)) close.current();
    };
    document.addEventListener("pointerdown", outside, true);
    document.addEventListener("scroll", scroll, true);
    window.addEventListener("resize", place);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      document.removeEventListener("scroll", scroll, true);
      window.removeEventListener("resize", place);
    };
  }, [origin]);
  // Errors can change the panel height after an asynchronous action.
  useLayoutEffect(() => {
    const bounds = ref.current!.getBoundingClientRect();
    if (bounds.bottom > window.innerHeight - 8) {
      setPosition((old) => ({
        ...old,
        top: Math.max(8, window.innerHeight - bounds.height - 8),
      }));
    }
  }, [error]);
  return (
    <dialog
      open
      ref={ref}
      className={`pop-confirm ${tone}`}
      style={position}
      aria-label={label}
      aria-modal="false"
      aria-describedby={descriptionId}
      onKeyDown={(event) => {
        if (event.key === "Escape") {
          event.preventDefault();
          event.stopPropagation();
          dismiss();
        }
      }}
      onCancel={(event) => {
        event.preventDefault();
        event.stopPropagation();
        dismiss();
      }}
    >
      <AlertTriangle
        size={18}
        className="pop-confirm-icon"
        aria-hidden="true"
      />
      <div className="pop-confirm-content">
        <strong>{title}</strong>
        <p id={descriptionId}>{description}</p>
        {error && (
          <p role="alert" className="error-text">
            {error}
          </p>
        )}
        <div className="pop-confirm-actions">
          <button type="button" ref={cancel} onClick={dismiss}>
            <X size={13} />
            取消
          </button>
          <button
            type="button"
            className={tone}
            disabled={busy}
            onClick={onConfirm}
          >
            <Check size={13} />
            {busy ? "处理中…" : confirmText}
          </button>
        </div>
      </div>
    </dialog>
  );
}
