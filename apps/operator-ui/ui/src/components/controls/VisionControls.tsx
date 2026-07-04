import { CircleDot, Pause, Radio, ScanLine } from "lucide-react";

import type { RectF32 } from "../../domain/geometry";
import type { AlgorithmId, VisionState } from "../../domain/vision";
import { algorithmLabel, algorithms } from "../../domain/vision";
import { Button } from "../ui/Button";
import { MetricGrid } from "../ui/Metric";
import { Panel } from "../ui/Panel";
import { Select } from "../ui/Select";
import { Toolbar } from "../ui/Toolbar";

export function VisionControls({
  vision,
  pendingRoi,
  pending,
  onSelectAlgorithm,
  onClearRoi,
  onCaptureTemplate,
  onStartProcessing,
  onStopProcessing,
}: {
  vision?: VisionState;
  pendingRoi: RectF32 | null;
  pending: (key: string) => boolean;
  onSelectAlgorithm: (algorithm: AlgorithmId) => void;
  onClearRoi: () => void;
  onCaptureTemplate: () => void;
  onStartProcessing: () => void;
  onStopProcessing: () => void;
}) {
  const hasRoi = pendingRoi !== null || vision?.roi != null;

  return (
    <Panel eyebrow="Detect" title="Vision">
      <div className="grid gap-3">
        <Select
          label="Algorithm"
          onChange={(event) => onSelectAlgorithm(event.currentTarget.value as AlgorithmId)}
          value={vision?.selected_algorithm ?? "TemplateNcc"}
        >
          {algorithms.map((algorithm) => (
            <option key={algorithm} value={algorithm}>
              {algorithmLabel(algorithm)}
            </option>
          ))}
        </Select>
        <Toolbar>
          <Button busy={pending("clear-roi")} icon={<ScanLine />} onClick={onClearRoi}>
            Clear ROI
          </Button>
          <Button
            busy={pending("capture-template")}
            disabled={!hasRoi}
            icon={<CircleDot />}
            onClick={onCaptureTemplate}
          >
            Template
          </Button>
        </Toolbar>
        <Toolbar>
          <Button
            busy={pending("start-processing")}
            icon={<Radio />}
            onClick={onStartProcessing}
            variant="primary"
          >
            Process
          </Button>
          <Button busy={pending("stop-processing")} icon={<Pause />} onClick={onStopProcessing}>
            Pause
          </Button>
        </Toolbar>
        <MetricGrid
          items={[
            {
              label: "Template",
              tone: vision?.has_template ? "good" : "warn",
              value: vision?.has_template ? "Ready" : "Missing",
            },
            { label: "Processing", value: `${(vision?.processing_fps ?? 0).toFixed(1)} fps` },
            { label: "Latency", value: `${(vision?.mean_latency_ms ?? 0).toFixed(2)} ms` },
            {
              label: "Dropped",
              tone: (vision?.dropped_input_frames ?? 0) > 0 ? "warn" : "neutral",
              value: String(vision?.dropped_input_frames ?? 0),
            },
          ]}
        />
        {!hasRoi ? <p className="text-xs text-muted">Draw an ROI on the live frame to capture a template.</p> : null}
      </div>
    </Panel>
  );
}
