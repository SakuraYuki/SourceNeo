import type { ComponentPropsWithoutRef } from "react";
import type { LucideIcon } from "lucide-react";

export function IconButton({
  label,
  icon: Icon,
  className = "",
  title,
  ...props
}: Omit<ComponentPropsWithoutRef<"button">, "children"> & {
  label: string;
  icon: LucideIcon;
}) {
  return (
    <button
      type="button"
      {...props}
      className={`icon-btn ${className}`}
      aria-label={label}
      title={title ?? label}
    >
      <Icon size={16} aria-hidden="true" />
    </button>
  );
}
