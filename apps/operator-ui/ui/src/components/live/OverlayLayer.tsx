import type { Detection } from "../../domain/vision";
import { detectionConfidenceLabel } from "../../domain/vision";
import { StatusPill } from "../ui/StatusPill";

export function OverlayLayer({
  hasFrame,
  recording,
  detection,
}: {
  hasFrame: boolean;
  recording: boolean;
  detection: Detection | null | undefined;
}) {
  return (
    <div className="pointer-events-none absolute inset-x-3 top-3 flex items-start justify-between gap-3">
      <div className="flex flex-wrap gap-2">
        <StatusPill label={hasFrame ? "Live" : "Waiting"} tone={hasFrame ? "good" : "neutral"} />
        {recording ? <StatusPill label="Recording" tone="danger" /> : null}
      </div>
      <div className="rounded-md border border-border bg-canvas/85 px-3 py-2 text-right backdrop-blur">
        <p className="text-xs text-muted">Detection</p>
        <strong className="text-sm font-semibold text-text">
          {detectionConfidenceLabel(detection)}
        </strong>
      </div>
    </div>
  );
}
