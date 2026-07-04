import { invoke } from "@tauri-apps/api/core";
import { listen } from "@tauri-apps/api/event";
import {
  Camera,
  CircleDot,
  Disc,
  Pause,
  Play,
  Radio,
  ScanLine,
  Square,
  StopCircle,
} from "lucide-react";
import React, { useCallback, useEffect, useMemo, useRef, useState } from "react";

type PixelFormat = "Gray8" | "Rgb8";
type AlgorithmId =
  | "TemplateNcc"
  | "EdgeModelMatch"
  | "RadialSymmetry"
  | "RingGridTarget"
  | "ChessCorners"
  | "CalibrationTarget";

type RectF32 = { x: number; y: number; width: number; height: number };
type FrameMeta = {
  frame_id: number;
  timestamp: number;
  width: number;
  height: number;
  stride: number;
  pixel_format: PixelFormat;
};
type FramePayload = { meta: FrameMeta; data_base64: string };
type Versioned<T> = { revision: number; value: T };
type CameraState = {
  lifecycle: string;
  requested_fps: number;
  actual_fps: number;
  frame_width: number;
  frame_height: number;
  frame_id: number;
  dropped_frames: number;
  error?: string | null;
};
type Detection = {
  frame_id: number;
  confidence: number;
  bbox?: RectF32 | null;
  method: AlgorithmId;
  latency_us: number;
};
type VisionState = {
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
type RecorderState = {
  lifecycle: string;
  session_path?: string | null;
  recorded_frames: number;
  recorded_detections: number;
  dropped_frames: number;
  error?: string | null;
};
type EventSummary = {
  sequence: number;
  summary: string;
  correlation_id?: string | null;
  source: { component: { component_type: string; component_name: string } };
};
type SystemView = {
  camera: Versioned<CameraState>;
  vision: Versioned<VisionState>;
  recorder: Versioned<RecorderState>;
  recent_events: EventSummary[];
  resync_count: number;
};

const algorithms: AlgorithmId[] = [
  "TemplateNcc",
  "EdgeModelMatch",
  "RadialSymmetry",
  "RingGridTarget",
  "ChessCorners",
  "CalibrationTarget",
];

export function App() {
  const [view, setView] = useState<SystemView | null>(null);
  const [frame, setFrame] = useState<FramePayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const [pendingRoi, setPendingRoi] = useState<RectF32 | null>(null);
  const canvasRef = useRef<HTMLCanvasElement | null>(null);
  const dragStart = useRef<{ x: number; y: number } | null>(null);

  useEffect(() => {
    invoke<SystemView>("system_view").then(setView).catch(showError(setError));
    invoke<FramePayload | null>("latest_frame").then(setFrame).catch(showError(setError));
    const unlistenSystem = listen<SystemView>("system-view", (event) => setView(event.payload));
    const unlistenFrame = listen<FramePayload>("frame", (event) => setFrame(event.payload));
    return () => {
      unlistenSystem.then((unlisten) => unlisten());
      unlistenFrame.then((unlisten) => unlisten());
    };
  }, []);

  const camera = view?.camera.value;
  const vision = view?.vision.value;
  const recorder = view?.recorder.value;
  const activeRoi = pendingRoi ?? vision?.roi ?? null;

  useEffect(() => {
    drawFrame(canvasRef.current, frame, activeRoi, vision?.last_detection?.bbox ?? null);
  }, [frame, activeRoi, vision?.last_detection]);

  const run = useCallback((command: string, args?: Record<string, unknown>) => {
    setError(null);
    invoke(command, args).catch(showError(setError));
  }, []);

  const onPointerDown = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (!frame) return;
    const point = canvasToImage(event.currentTarget, event, frame.meta);
    dragStart.current = point;
    setPendingRoi({ x: point.x, y: point.y, width: 0, height: 0 });
  };

  const onPointerMove = (event: React.PointerEvent<HTMLCanvasElement>) => {
    if (!frame || !dragStart.current) return;
    const point = canvasToImage(event.currentTarget, event, frame.meta);
    setPendingRoi(rectFromPoints(dragStart.current, point));
  };

  const onPointerUp = () => {
    dragStart.current = null;
    if (pendingRoi && pendingRoi.width >= 2 && pendingRoi.height >= 2) {
      run("set_roi", { roi: pendingRoi });
    }
  };

  const timeline = useMemo(() => [...(view?.recent_events ?? [])].reverse().slice(0, 18), [view]);

  return (
    <main className="app">
      <header className="topbar">
        <div>
          <h1>Vision Lab</h1>
          <p>Communication-first local vision prototype</p>
        </div>
        <div className="topbar__metrics">
          <Status label="Camera" value={camera?.lifecycle ?? "Offline"} />
          <Status label="Vision" value={vision?.lifecycle ?? "Offline"} />
          <Status label="Recorder" value={recorder?.lifecycle ?? "Offline"} />
          <Status label="Resyncs" value={String(view?.resync_count ?? 0)} />
        </div>
      </header>

      {error && <div className="error">{error}</div>}

      <section className="workspace">
        <section className="viewport">
          <div className="viewport__head">
            <div>
              <h2>Live Camera</h2>
              <span>
                frame {camera?.frame_id ?? 0} · {camera?.actual_fps.toFixed(1) ?? "0.0"} fps
              </span>
            </div>
            <div className="inline-stats">
              <span>{frame ? `${frame.meta.width}x${frame.meta.height}` : "No frame"}</span>
              <span>{camera?.dropped_frames ?? 0} dropped</span>
            </div>
          </div>
          <canvas
            ref={canvasRef}
            className="frame-canvas"
            width={frame?.meta.width ?? 320}
            height={frame?.meta.height ?? 240}
            onPointerDown={onPointerDown}
            onPointerMove={onPointerMove}
            onPointerUp={onPointerUp}
            onPointerLeave={onPointerUp}
          />
        </section>

        <aside className="side">
          <section className="panel">
            <h2>Camera</h2>
            <div className="button-row">
              <IconButton icon={<Camera />} label="Connect" onClick={() => run("connect_camera")} />
              <IconButton icon={<Play />} label="Start" onClick={() => run("start_camera")} />
              <IconButton icon={<StopCircle />} label="Stop" onClick={() => run("stop_camera")} />
            </div>
            <label className="field">
              <span>Requested FPS</span>
              <input
                type="number"
                min={1}
                max={120}
                defaultValue={camera?.requested_fps ?? 30}
                onBlur={(event) =>
                  run("set_requested_fps", { fps: Number(event.currentTarget.value) })
                }
              />
            </label>
          </section>

          <section className="panel">
            <h2>Vision</h2>
            <label className="field">
              <span>Algorithm</span>
              <select
                value={vision?.selected_algorithm ?? "TemplateNcc"}
                onChange={(event) =>
                  run("select_algorithm", { algorithm: event.currentTarget.value })
                }
              >
                {algorithms.map((algorithm) => (
                  <option key={algorithm} value={algorithm}>
                    {algorithm}
                  </option>
                ))}
              </select>
            </label>
            <div className="button-row">
              <IconButton
                icon={<ScanLine />}
                label="Clear ROI"
                onClick={() => run("set_roi", { roi: null })}
              />
              <IconButton
                icon={<CircleDot />}
                label="Template"
                onClick={() => run("capture_template")}
              />
            </div>
            <div className="button-row">
              <IconButton icon={<Radio />} label="Process" onClick={() => run("start_processing")} />
              <IconButton icon={<Pause />} label="Pause" onClick={() => run("stop_processing")} />
            </div>
            <MetricGrid
              items={[
                ["Template", vision?.has_template ? "ready" : "missing"],
                ["Processing", `${vision?.processing_fps.toFixed(1) ?? "0.0"} fps`],
                ["Latency", `${vision?.mean_latency_ms.toFixed(2) ?? "0.00"} ms`],
                ["Dropped", String(vision?.dropped_input_frames ?? 0)],
              ]}
            />
          </section>

          <section className="panel">
            <h2>Recorder</h2>
            <div className="button-row">
              <IconButton
                icon={<Disc />}
                label="Record"
                onClick={() => run("start_recording", { maxFps: 8 })}
              />
              <IconButton icon={<Square />} label="Stop" onClick={() => run("stop_recording")} />
            </div>
            <MetricGrid
              items={[
                ["Frames", String(recorder?.recorded_frames ?? 0)],
                ["Detections", String(recorder?.recorded_detections ?? 0)],
                ["Dropped", String(recorder?.dropped_frames ?? 0)],
              ]}
            />
            <p className="path">{recorder?.session_path ?? "No active session"}</p>
          </section>
        </aside>
      </section>

      <section className="timeline">
        <h2>Event Timeline</h2>
        <div className="timeline__list">
          {timeline.map((event) => (
            <div
              className="timeline__row"
              key={`${event.source.component.component_name}-${event.sequence}-${event.summary}`}
            >
              <span>{event.source.component.component_type}</span>
              <strong>#{event.sequence}</strong>
              <p>{event.summary}</p>
            </div>
          ))}
        </div>
      </section>
    </main>
  );
}

function Status({ label, value }: { label: string; value: string }) {
  return (
    <div className="status">
      <span>{label}</span>
      <strong>{value}</strong>
    </div>
  );
}

function MetricGrid({ items }: { items: [string, string][] }) {
  return (
    <div className="metric-grid">
      {items.map(([label, value]) => (
        <div key={label}>
          <span>{label}</span>
          <strong>{value}</strong>
        </div>
      ))}
    </div>
  );
}

function IconButton({
  icon,
  label,
  onClick,
}: {
  icon: React.ReactNode;
  label: string;
  onClick: () => void;
}) {
  return (
    <button type="button" onClick={onClick}>
      {icon}
      <span>{label}</span>
    </button>
  );
}

function drawFrame(
  canvas: HTMLCanvasElement | null,
  frame: FramePayload | null,
  roi: RectF32 | null,
  detection: RectF32 | null,
) {
  if (!canvas) return;
  const ctx = canvas.getContext("2d");
  if (!ctx) return;
  ctx.clearRect(0, 0, canvas.width, canvas.height);
  if (!frame) {
    ctx.fillStyle = "#101418";
    ctx.fillRect(0, 0, canvas.width, canvas.height);
    return;
  }
  canvas.width = frame.meta.width;
  canvas.height = frame.meta.height;
  const bytes = Uint8Array.from(atob(frame.data_base64), (char) => char.charCodeAt(0));
  const image = ctx.createImageData(frame.meta.width, frame.meta.height);
  for (let i = 0; i < frame.meta.width * frame.meta.height; i += 1) {
    const gray = bytes[i] ?? 0;
    const offset = i * 4;
    image.data[offset] = gray;
    image.data[offset + 1] = gray;
    image.data[offset + 2] = gray;
    image.data[offset + 3] = 255;
  }
  ctx.putImageData(image, 0, 0);
  if (roi) drawRect(ctx, roi, "#d9b44a");
  if (detection) drawRect(ctx, detection, "#4fb286");
}

function drawRect(ctx: CanvasRenderingContext2D, rect: RectF32, color: string) {
  ctx.strokeStyle = color;
  ctx.lineWidth = 2;
  ctx.strokeRect(rect.x, rect.y, rect.width, rect.height);
}

function canvasToImage(canvas: HTMLCanvasElement, event: React.PointerEvent, meta: FrameMeta) {
  const rect = canvas.getBoundingClientRect();
  return {
    x: ((event.clientX - rect.left) / rect.width) * meta.width,
    y: ((event.clientY - rect.top) / rect.height) * meta.height,
  };
}

function rectFromPoints(a: { x: number; y: number }, b: { x: number; y: number }): RectF32 {
  return {
    x: Math.min(a.x, b.x),
    y: Math.min(a.y, b.y),
    width: Math.abs(a.x - b.x),
    height: Math.abs(a.y - b.y),
  };
}

function showError(setError: (value: string) => void) {
  return (error: unknown) => setError(error instanceof Error ? error.message : String(error));
}
