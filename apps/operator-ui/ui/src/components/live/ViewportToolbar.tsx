import type { CameraState, FramePayload } from "../../domain/camera";
import { frameSizeLabel } from "../../domain/camera";
import type { VisionState } from "../../domain/vision";
import { algorithmLabel } from "../../domain/vision";

export function ViewportToolbar({
  camera,
  frame,
  vision,
}: {
  camera?: CameraState;
  frame: FramePayload | null;
  vision?: VisionState;
}) {
  return (
    <div className="flex min-h-14 flex-wrap items-center justify-between gap-3 border-b border-border px-4 py-3">
      <div>
        <h2 className="text-sm font-semibold text-text">Live Camera</h2>
        <p className="mt-1 text-xs text-muted">
          Frame {camera?.frame_id ?? 0} · {(camera?.actual_fps ?? 0).toFixed(1)} fps ·{" "}
          {algorithmLabel(vision?.selected_algorithm ?? "ChessCorners")}
        </p>
      </div>
      <div className="flex flex-wrap gap-4 text-xs text-muted">
        <span>{frameSizeLabel(frame)}</span>
        <span>{camera?.dropped_frames ?? 0} capture drops</span>
      </div>
    </div>
  );
}
