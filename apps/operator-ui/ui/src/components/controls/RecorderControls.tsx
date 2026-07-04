import { Disc, Square } from "lucide-react";

import type { RecorderState } from "../../domain/recorder";
import { Button } from "../ui/Button";
import { MetricGrid } from "../ui/Metric";
import { Panel } from "../ui/Panel";
import { Toolbar } from "../ui/Toolbar";

export function RecorderControls({
  recorder,
  pending,
  onStartRecording,
  onStopRecording,
}: {
  recorder?: RecorderState;
  pending: (key: string) => boolean;
  onStartRecording: () => void;
  onStopRecording: () => void;
}) {
  return (
    <Panel eyebrow="Record" title="Recorder">
      <div className="grid gap-3">
        <Toolbar>
          <Button
            busy={pending("start-recording")}
            icon={<Disc />}
            onClick={onStartRecording}
            variant="primary"
          >
            Record
          </Button>
          <Button busy={pending("stop-recording")} icon={<Square />} onClick={onStopRecording}>
            Stop
          </Button>
        </Toolbar>
        <MetricGrid
          items={[
            { label: "Frames", value: String(recorder?.recorded_frames ?? 0) },
            { label: "Detections", value: String(recorder?.recorded_detections ?? 0) },
            {
              label: "Dropped",
              tone: (recorder?.dropped_frames ?? 0) > 0 ? "warn" : "neutral",
              value: String(recorder?.dropped_frames ?? 0),
            },
            { label: "State", value: recorder?.lifecycle ?? "Offline" },
          ]}
        />
        <p className="[overflow-wrap:anywhere] text-xs leading-5 text-muted">
          {recorder?.session_path ?? "No active recording session."}
        </p>
      </div>
    </Panel>
  );
}
