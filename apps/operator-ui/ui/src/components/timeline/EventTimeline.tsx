import type { EventSummary } from "../../domain/system";
import { TimelineItem } from "./TimelineItem";

export function EventTimeline({ events }: { events: EventSummary[] }) {
  return (
    <section className="border border-border bg-surface">
      <header className="flex min-h-11 items-center justify-between border-b border-border px-4">
        <h2 className="text-sm font-semibold text-text">Event Timeline</h2>
        <span className="text-xs text-muted">{events.length} recent</span>
      </header>
      <div className="max-h-52 overflow-auto">
        {events.length > 0 ? (
          events.map((event) => (
            <TimelineItem
              event={event}
              key={`${event.source.component.component_name}-${event.sequence}-${event.summary}`}
            />
          ))
        ) : (
          <p className="px-4 py-5 text-sm text-muted">No operator events yet.</p>
        )}
      </div>
    </section>
  );
}
