import { useEffect, useRef, type ReactNode } from "react";

export function Dialog({
  label,
  onClose,
  children,
  dismissible = true,
}: {
  label: string;
  onClose: () => void;
  children: ReactNode;
  dismissible?: boolean;
}) {
  const ref = useRef<HTMLDialogElement>(null);
  useEffect(() => {
    const dialog = ref.current!;
    dialog.showModal();
    return () => dialog.close();
  }, []);
  return (
    <dialog
      ref={ref}
      className="modal-shell"
      aria-label={label}
      onCancel={(e) => {
        e.preventDefault();
        if (dismissible) onClose();
      }}
      onClick={(e) => {
        if (dismissible && e.target === e.currentTarget) onClose();
      }}
    >
      {children}
    </dialog>
  );
}
