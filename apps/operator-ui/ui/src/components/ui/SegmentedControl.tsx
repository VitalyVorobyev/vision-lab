import { clsx } from "clsx";

type Option<T extends string> = {
  value: T;
  label: string;
};

export function SegmentedControl<T extends string>({
  label,
  value,
  options,
  onChange,
}: {
  label: string;
  value: T;
  options: Option<T>[];
  onChange: (value: T) => void;
}) {
  return (
    <div>
      <p className="mb-1.5 text-xs font-medium text-muted">{label}</p>
      <div className="grid grid-cols-2 rounded-md border border-border bg-canvas p-0.5">
        {options.map((option) => (
          <button
            className={clsx(
              "min-h-8 rounded px-2 text-xs font-medium transition-colors focus-visible:outline-2 focus-visible:outline-focus",
              option.value === value
                ? "bg-surface-hover text-text"
                : "text-muted hover:bg-surface-strong hover:text-text",
            )}
            key={option.value}
            onClick={() => onChange(option.value)}
            type="button"
          >
            {option.label}
          </button>
        ))}
      </div>
    </div>
  );
}
