import { Camera, Play, StopCircle } from "lucide-react";

import type { CameraState } from "../../domain/camera";
import { Button } from "../ui/Button";
import { MetricGrid } from "../ui/Metric";
import { Panel } from "../ui/Panel";
import { Toolbar } from "../ui/Toolbar";

export function CameraControls({
  camera,
  pending,
  onConnect,
  onStart,
  onStop,
  onSetRequestedFps,
}: {
  camera?: CameraState;
  pending: (key: string) => boolean;
  onConnect: () => void;
  onStart: () => void;
  onStop: () => void;
  onSetRequestedFps: (fps: number) => void;
}) {
  return (
    <Panel eyebrow="Acquire" title="Camera">
      <div className="grid gap-3">
        <Toolbar>
          <Button busy={pending("connect-camera")} icon={<Camera />} onClick={onConnect}>
            Connect
          </Button>
          <Button busy={pending("start-camera")} icon={<Play />} onClick={onStart} variant="primary">
            Start
          </Button>
          <Button busy={pending("stop-camera")} icon={<StopCircle />} onClick={onStop}>
            Stop
          </Button>
        </Toolbar>
        <label className="grid gap-1.5">
          <span className="text-xs font-medium text-muted">Requested FPS</span>
          <input
            className="min-h-9 rounded-md border border-border bg-canvas px-3 text-sm text-text focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-focus"
            defaultValue={camera?.requested_fps ?? 30}
            key={camera?.requested_fps ?? 30}
            max={120}
            min={1}
            onBlur={(event) => onSetRequestedFps(Number(event.currentTarget.value))}
            type="number"
          />
        </label>
        <MetricGrid
          items={[
            { label: "Actual FPS", value: `${(camera?.actual_fps ?? 0).toFixed(1)}` },
            { label: "Frame", value: String(camera?.frame_id ?? 0) },
            { label: "Size", value: `${camera?.frame_width ?? 0}x${camera?.frame_height ?? 0}` },
            {
              label: "Capture drops",
              tone: (camera?.dropped_frames ?? 0) > 0 ? "warn" : "neutral",
              value: String(camera?.dropped_frames ?? 0),
            },
          ]}
        />
      </div>
    </Panel>
  );
}
