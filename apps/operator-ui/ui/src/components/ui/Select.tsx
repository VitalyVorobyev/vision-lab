import type { SelectHTMLAttributes } from "react";
import { clsx } from "clsx";

type SelectProps = SelectHTMLAttributes<HTMLSelectElement> & {
  label: string;
};

export function Select({ className, label, children, ...props }: SelectProps) {
  return (
    <label className="grid gap-1.5">
      <span className="text-xs font-medium text-muted">{label}</span>
      <select
        className={clsx(
          "min-h-9 rounded-md border border-border bg-canvas px-3 text-sm text-text",
          "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus",
          className,
        )}
        {...props}
      >
        {children}
      </select>
    </label>
  );
}
