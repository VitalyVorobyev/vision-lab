import type { CameraState } from "../../domain/camera";
import type { RecorderState } from "../../domain/recorder";
import { componentHealth } from "../../domain/system";
import type { VisionState } from "../../domain/vision";
import { StatusPill } from "../ui/StatusPill";

type HealthState = CameraState | VisionState | RecorderState | undefined;

export function ComponentHealth({ label, state }: { label: string; state: HealthState }) {
  const health = componentHealth(state);
  const tone = health === "error" ? "danger" : health === "offline" ? "neutral" : "good";

  return (
    <div className="min-w-0 border-l-2 border-accent px-3">
      <p className="text-xs text-muted">{label}</p>
      <div className="mt-1 flex items-center gap-2">
        <strong className="truncate text-sm font-semibold text-text">
          {state?.lifecycle ?? "Offline"}
        </strong>
        <StatusPill label={health === "error" ? "Check" : "OK"} tone={tone} />
      </div>
    </div>
  );
}
