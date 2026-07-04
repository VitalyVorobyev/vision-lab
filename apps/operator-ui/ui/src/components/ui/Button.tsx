import type { ButtonHTMLAttributes, ReactNode } from "react";
import { clsx } from "clsx";

type ButtonVariant = "primary" | "secondary" | "danger" | "ghost";

type ButtonProps = ButtonHTMLAttributes<HTMLButtonElement> & {
  icon?: ReactNode;
  variant?: ButtonVariant;
  busy?: boolean;
};

const variantClass: Record<ButtonVariant, string> = {
  primary:
    "border-accent bg-accent text-[var(--color-accent-contrast)] hover:border-accent-strong hover:bg-accent-strong",
  secondary: "border-border bg-surface-strong text-text hover:border-accent hover:bg-surface-hover",
  danger: "border-danger/60 bg-danger/15 text-danger-text hover:border-danger hover:bg-danger/25",
  ghost: "border-transparent bg-transparent text-muted hover:border-border hover:bg-surface-strong hover:text-text",
};

export function Button({
  children,
  className,
  icon,
  variant = "secondary",
  busy = false,
  disabled,
  ...props
}: ButtonProps) {
  return (
    <button
      className={clsx(
        "inline-flex min-h-9 items-center justify-center gap-2 rounded-md border px-3 text-sm font-medium transition-colors",
        "focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus",
        "disabled:cursor-not-allowed disabled:border-border disabled:bg-disabled disabled:text-muted",
        variantClass[variant],
        className,
      )}
      disabled={disabled || busy}
      type="button"
      {...props}
    >
      {icon ? <span className="grid size-4 place-items-center [&_svg]:size-4">{icon}</span> : null}
      <span>{busy ? "Working" : children}</span>
    </button>
  );
}
