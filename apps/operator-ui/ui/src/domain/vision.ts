import type { RectF32 } from "./geometry";
import type { PointF32 } from "./geometry";

export type AlgorithmId =
  | "TemplateNcc"
  | "EdgeModelMatch"
  | "RadialSymmetry"
  | "RingGridTarget"
  | "ChessCorners"
  | "CalibrationTarget";

export type Detection = {
  frame_id: number;
  object_id: string;
  confidence: number;
  bbox?: RectF32 | null;
  points: PointF32[];
  method: AlgorithmId;
  latency_us: number;
  diagnostics?: string | null;
};

export type VisionState = {
  lifecycle: string;
  selected_algorithm: AlgorithmId;
  roi?: RectF32 | null;
  has_template: boolean;
  input_fps: number;
  processing_fps: number;
  mean_latency_ms: number;
  dropped_input_frames: number;
  last_detection?: Detection | null;
  error?: string | null;
};

export const algorithms: AlgorithmId[] = [
  "ChessCorners",
  "TemplateNcc",
  "EdgeModelMatch",
  "RadialSymmetry",
  "RingGridTarget",
  "CalibrationTarget",
];

export function algorithmLabel(algorithm: AlgorithmId): string {
  switch (algorithm) {
    case "TemplateNcc":
      return "Template NCC";
    case "EdgeModelMatch":
      return "Edge model";
    case "RadialSymmetry":
      return "Radial symmetry";
    case "RingGridTarget":
      return "Ring grid";
    case "ChessCorners":
      return "Chess corners";
    case "CalibrationTarget":
      return "Calibration target";
  }
}

export function detectionConfidenceLabel(detection: Detection | null | undefined): string {
  if (!detection) return "None";
  return `${(detection.confidence * 100).toFixed(1)}%`;
}
