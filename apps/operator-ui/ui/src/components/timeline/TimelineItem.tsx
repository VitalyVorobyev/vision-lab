import type { EventSummary } from "../../domain/system";

export function TimelineItem({ event }: { event: EventSummary }) {
  return (
    <div className="grid min-h-9 grid-cols-[104px_64px_minmax(0,1fr)] items-center gap-3 border-b border-border px-3 py-2 text-xs last:border-b-0">
      <span className="truncate text-muted">{event.source.component.component_type}</span>
      <strong className="font-mono font-semibold text-accent-text">#{event.sequence}</strong>
      <p className="truncate text-text">{event.summary}</p>
    </div>
  );
}
