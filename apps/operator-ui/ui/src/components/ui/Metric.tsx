import { clsx } from "clsx";

type MetricProps = {
  label: string;
  value: string;
  tone?: "neutral" | "good" | "warn" | "danger";
};

const toneClass = {
  neutral: "text-text",
  good: "text-success-text",
  warn: "text-warning-text",
  danger: "text-danger-text",
};

export function Metric({ label, value, tone = "neutral" }: MetricProps) {
  return (
    <div className="min-w-0 border border-border bg-surface-muted px-3 py-2">
      <p className="truncate text-xs text-muted">{label}</p>
      <strong className={clsx("mt-1 block truncate font-mono text-sm font-semibold", toneClass[tone])}>
        {value}
      </strong>
    </div>
  );
}

export function MetricGrid({ items }: { items: MetricProps[] }) {
  return (
    <div className="grid grid-cols-2 gap-2">
      {items.map((item) => (
        <Metric key={item.label} {...item} />
      ))}
    </div>
  );
}
