import type { ReactNode } from "react";
import { clsx } from "clsx";

type PanelProps = {
  title: string;
  eyebrow?: string;
  action?: ReactNode;
  children: ReactNode;
  className?: string;
};

export function Panel({ title, eyebrow, action, children, className }: PanelProps) {
  return (
    <section className={clsx("border-b border-border bg-surface p-4", className)}>
      <header className="mb-3 flex min-h-7 items-start justify-between gap-3">
        <div>
          {eyebrow ? <p className="text-xs font-medium uppercase text-muted">{eyebrow}</p> : null}
          <h2 className="text-sm font-semibold text-text">{title}</h2>
        </div>
        {action}
      </header>
      {children}
    </section>
  );
}
