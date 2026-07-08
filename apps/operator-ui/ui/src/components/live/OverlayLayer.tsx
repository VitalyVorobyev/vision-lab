import { Eye, EyeOff } from "lucide-react";

import type { OverlayKey, OverlayVisibility } from "../../domain/overlays";
import type { Detection } from "../../domain/vision";
import { detectionConfidenceLabel } from "../../domain/vision";
import { StatusPill } from "../ui/StatusPill";

export function OverlayLayer({
  hasFrame,
  recording,
  detection,
  overlays,
  onToggleOverlay,
}: {
  hasFrame: boolean;
  recording: boolean;
  detection: Detection | null | undefined;
  overlays: OverlayVisibility;
  onToggleOverlay: (key: OverlayKey) => void;
}) {
  return (
    <div className="pointer-events-none absolute inset-3 flex items-start justify-between gap-3">
      <div className="flex flex-wrap gap-2">
        <StatusPill label={hasFrame ? "Live" : "Waiting"} tone={hasFrame ? "good" : "neutral"} />
        {recording ? <StatusPill label="Recording" tone="danger" /> : null}
      </div>
      <div className="flex w-[210px] max-w-[52vw] flex-col gap-2">
        {overlays.summary ? (
          <div className="rounded-md border border-border bg-canvas/90 px-3 py-2 text-right backdrop-blur">
            <p className="text-xs text-muted">Detection</p>
            <strong className="text-sm font-semibold text-text">
              {detectionConfidenceLabel(detection)}
            </strong>
          </div>
        ) : null}
        <div className="pointer-events-auto overflow-hidden rounded-md border border-border bg-canvas/90 backdrop-blur">
          <div className="border-b border-border px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.07em] text-muted">
            Overlays
          </div>
          <div className="py-1">
            {overlayRows.map((row) => (
              <button
                aria-pressed={overlays[row.key]}
                className="flex min-h-8 w-full items-center gap-2 px-3 text-left text-xs text-text transition-colors hover:bg-surface-strong focus-visible:outline-2 focus-visible:outline-inset focus-visible:outline-focus"
                key={row.key}
                onClick={() => onToggleOverlay(row.key)}
                type="button"
              >
                <span
                  className="size-2.5 rounded-[3px] border border-white/25"
                  style={{ backgroundColor: overlays[row.key] ? row.color : "transparent" }}
                />
                <span className="min-w-0 flex-1 truncate">{row.label}</span>
                {overlays[row.key] ? <Eye className="size-3.5 text-muted" /> : <EyeOff className="size-3.5 text-muted/60" />}
              </button>
            ))}
          </div>
        </div>
      </div>
    </div>
  );
}

const overlayRows: { key: OverlayKey; label: string; color: string }[] = [
  { key: "roi", label: "ROI", color: "#d7a936" },
  { key: "points", label: "Keypoints", color: "#8ee0bb" },
  { key: "bbox", label: "Bounding box", color: "#e2b94c" },
  { key: "summary", label: "Summary", color: "#edf1f4" },
];
