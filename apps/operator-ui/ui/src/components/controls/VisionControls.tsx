import { CircleDot, Pause, Radio, ScanLine, ScanSearch } from "lucide-react";

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
  onRunChess,
  onStartProcessing,
  onStopProcessing,
}: {
  vision?: VisionState;
  pendingRoi: RectF32 | null;
  pending: (key: string) => boolean;
  onSelectAlgorithm: (algorithm: AlgorithmId) => void;
  onClearRoi: () => void;
  onCaptureTemplate: () => void;
  onRunChess: () => void;
  onStartProcessing: () => void;
  onStopProcessing: () => void;
}) {
  const hasRoi = pendingRoi !== null || vision?.roi != null;
  const selectedAlgorithm = vision?.selected_algorithm ?? "ChessCorners";
  const isTemplate = selectedAlgorithm === "TemplateNcc";
  const detectorMetric = isTemplate
    ? ({
        label: "Template",
        tone: vision?.has_template ? "good" : "warn",
        value: vision?.has_template ? "Ready" : "Missing",
      } as const)
    : ({
        label: "Detector",
        tone: "good",
        value: "Ready",
      } as const);

  return (
    <Panel eyebrow="Detect" title="Vision">
      <div className="grid gap-3">
        <Button
          busy={pending("run-chess")}
          className="w-full"
          icon={<ScanSearch />}
          onClick={onRunChess}
          variant="primary"
        >
          Run ChESS
        </Button>
        <Select
          label="Algorithm"
          onChange={(event) => onSelectAlgorithm(event.currentTarget.value as AlgorithmId)}
          value={selectedAlgorithm}
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
            disabled={!hasRoi || !isTemplate}
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
          >
            Process
          </Button>
          <Button busy={pending("stop-processing")} icon={<Pause />} onClick={onStopProcessing}>
            Pause
          </Button>
        </Toolbar>
        <MetricGrid
          items={[
            detectorMetric,
            { label: "Processing", value: `${(vision?.processing_fps ?? 0).toFixed(1)} fps` },
            { label: "Latency", value: `${(vision?.mean_latency_ms ?? 0).toFixed(2)} ms` },
            {
              label: "Dropped",
              tone: (vision?.dropped_input_frames ?? 0) > 0 ? "warn" : "neutral",
              value: String(vision?.dropped_input_frames ?? 0),
            },
          ]}
        />
        {isTemplate && !hasRoi ? (
          <p className="text-xs text-muted">Draw an ROI on the live frame to capture a template.</p>
        ) : null}
      </div>
    </Panel>
  );
}
