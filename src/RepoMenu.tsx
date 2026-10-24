import { useLayoutEffect, useRef, useState } from "react";
import { createPortal } from "react-dom";
import type { LucideIcon } from "lucide-react";

export interface MenuAction {
  label: string;
  icon?: LucideIcon;
  run: () => void;
  disabled?: boolean;
  danger?: boolean;
  separator?: boolean;
}

export function RepoMenu({
  x,
  y,
  origin,
  label,
  actions,
  onClose,
}: {
  x: number;
  y: number;
  origin: HTMLElement;
  label: string;
  actions: MenuAction[];
  onClose: () => void;
}) {
  const ref = useRef<HTMLDivElement>(null);
  const close = useRef(onClose);
  close.current = onClose;
  const [position, setPosition] = useState<{
    left: number;
    top: number;
    maxHeight?: number;
  }>({ left: x, top: y });
  function dismiss(restore = false) {
    if (restore && origin.isConnected) origin.focus();
    close.current();
  }
  useLayoutEffect(() => {
    const menu = ref.current!;
    const bounds = menu.getBoundingClientRect();
    const left = Math.max(8, Math.min(x, window.innerWidth - bounds.width - 8));
    if (origin.getAttribute("aria-haspopup") === "menu") {
      const anchor = origin.getBoundingClientRect();
      const below = window.innerHeight - anchor.bottom - 12;
      const above = anchor.top - 12;
      const upward = below < bounds.height && above > below;
      const maxHeight = Math.max(48, upward ? above : below);
      const height = Math.min(bounds.height, maxHeight);
      setPosition({
        left,
        top: Math.max(8, upward ? anchor.top - 4 - height : anchor.bottom + 4),
        maxHeight,
      });
    } else {
      setPosition({
        left,
        top: Math.max(8, Math.min(y, window.innerHeight - bounds.height - 8)),
      });
    }
    menu.querySelector<HTMLButtonElement>("button:not(:disabled)")?.focus();
    const outside = (event: PointerEvent) => {
      if (
        origin.getAttribute("aria-haspopup") === "menu" &&
        origin.contains(event.target as Node)
      )
        return;
      if (!menu.contains(event.target as Node)) close.current();
    };
    const hide = () => close.current();
    const scroll = (event: Event) => {
      if (!menu.contains(event.target as Node)) hide();
    };
    document.addEventListener("pointerdown", outside, true);
    window.addEventListener("resize", hide);
    window.addEventListener("blur", hide);
    document.addEventListener("scroll", scroll, true);
    return () => {
      document.removeEventListener("pointerdown", outside, true);
      window.removeEventListener("resize", hide);
      window.removeEventListener("blur", hide);
      document.removeEventListener("scroll", scroll, true);
    };
  }, [x, y, origin]);
  return createPortal(
    <div
      ref={ref}
      role="menu"
      aria-label={label}
      className="repo-menu"
      style={position}
      onContextMenu={(e) => e.preventDefault()}
      onBlur={(e) => {
        if (
          e.relatedTarget === origin &&
          origin.getAttribute("aria-haspopup") === "menu"
        )
          return;
        if (!e.currentTarget.contains(e.relatedTarget as Node)) close.current();
      }}
      onKeyDown={(e) => {
        if (e.key === "Escape") {
          e.preventDefault();
          dismiss(true);
          return;
        }
        if (e.key === "Tab") {
          dismiss(true);
          return;
        }
        if (!["ArrowDown", "ArrowUp", "Home", "End"].includes(e.key)) return;
        e.preventDefault();
        const items = Array.from(
          ref.current!.querySelectorAll<HTMLButtonElement>(
            "button:not(:disabled)",
          ),
        );
        const i = items.indexOf(document.activeElement as HTMLButtonElement);
        const next =
          e.key === "Home"
            ? 0
            : e.key === "End"
              ? items.length - 1
              : (i + (e.key === "ArrowDown" ? 1 : -1) + items.length) %
                items.length;
        items[next]?.focus();
      }}
    >
      <div className="repo-menu-title">{label}</div>
      {actions.map((action) => (
        <button
          key={action.label}
          type="button"
          role="menuitem"
          tabIndex={-1}
          disabled={action.disabled}
          className={`${action.danger ? "menu-danger" : ""} ${action.separator ? "menu-separated" : ""}`}
          onClick={() => {
            dismiss(true);
            action.run();
          }}
        >
          {action.icon && <action.icon size={14} aria-hidden="true" />}
          {action.label}
        </button>
      ))}
    </div>,
    document.body,
  );
}
