import { clsx } from "clsx";

type StatusTone = "neutral" | "good" | "warn" | "danger" | "accent";

const toneClass: Record<StatusTone, string> = {
  neutral: "border-border bg-surface-strong text-muted",
  good: "border-success/50 bg-success/15 text-success-text",
  warn: "border-warning/50 bg-warning/15 text-warning-text",
  danger: "border-danger/50 bg-danger/15 text-danger-text",
  accent: "border-accent/60 bg-accent/15 text-accent-text",
};

export function StatusPill({
  label,
  tone = "neutral",
  className,
}: {
  label: string;
  tone?: StatusTone;
  className?: string;
}) {
  return (
    <span
      className={clsx(
        "inline-flex min-h-6 items-center rounded-md border px-2 text-xs font-medium",
        toneClass[tone],
        className,
      )}
    >
      {label}
    </span>
  );
}
