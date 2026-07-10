import { CircleDot, Pause, Radio, ScanLine, ScanSearch } from "lucide-react";

import type { RectF32 } from "../../domain/geometry";
import type { AlgorithmId, VisionState } from "../../domain/vision";
import { algorithmLabel, runnableAlgorithms } from "../../domain/vision";
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
  onOpenAlgorithmConfig,
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
  onOpenAlgorithmConfig?: () => void;
  onClearRoi: () => void;
  onCaptureTemplate: () => void;
  onRunChess: () => void;
  onStartProcessing: () => void;
  onStopProcessing: () => void;
}) {
  const hasRoi = pendingRoi !== null || vision?.roi != null;
  const selectedAlgorithm = vision?.selected_algorithm ?? "ChessCorners";
  const isTemplate = selectedAlgorithm === "TemplateNcc";
  const isRingGrid = selectedAlgorithm === "RingGridTarget";
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
          {runnableAlgorithms.map((algorithm) => (
            <option key={algorithm} value={algorithm}>
              {algorithmLabel(algorithm)}
            </option>
          ))}
        </Select>
        {isRingGrid ? (
          <div className="border border-border bg-surface-muted px-3 py-2 text-xs">
            <p className="text-muted">Coded hex target</p>
            <p className="mt-1 font-mono text-text">
              {vision?.ringgrid_target.rows ?? 15} rows · {vision?.ringgrid_target.long_row_cols ?? 14} columns · {vision?.ringgrid_target.pitch_mm ?? 8} mm
            </p>
            {onOpenAlgorithmConfig ? (
              <Button className="mt-2 w-full" onClick={onOpenAlgorithmConfig} variant="ghost">
                Open full configuration
              </Button>
            ) : null}
          </div>
        ) : null}
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
