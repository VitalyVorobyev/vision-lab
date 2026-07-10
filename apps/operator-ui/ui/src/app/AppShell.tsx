import { useState, type ReactNode, type RefObject } from "react";
import { CircleDot, Grid3X3, Radio, ScanLine, ScanSearch } from "lucide-react";

import { CameraConfigPanel } from "../components/controls/CameraConfigPanel";
import { CameraControls } from "../components/controls/CameraControls";
import { RecorderControls } from "../components/controls/RecorderControls";
import { RingGridConfigPanel } from "../components/controls/RingGridConfigPanel";
import { VisionControls } from "../components/controls/VisionControls";
import { LiveViewport } from "../components/live/LiveViewport";
import { ComponentHealth } from "../components/status/ComponentHealth";
import { EventTimeline } from "../components/timeline/EventTimeline";
import { Button } from "../components/ui/Button";
import { MetricGrid } from "../components/ui/Metric";
import { Panel } from "../components/ui/Panel";
import type { FramePayload } from "../domain/camera";
import type { RectF32 } from "../domain/geometry";
import type { OverlayKey, OverlayVisibility } from "../domain/overlays";
import type { SystemView } from "../domain/system";
import { orderedRecentEvents } from "../domain/system";
import type { AlgorithmId, RingGridTargetConfig } from "../domain/vision";
import { algorithmLabel, runnableAlgorithms } from "../domain/vision";

type CanvasHandlers = Parameters<typeof LiveViewport>[0]["canvasHandlers"];
type OperatorTab = "canvas" | "algorithm" | "camera" | "health";

export type AppShellProps = {
  view: SystemView | null;
  frame: FramePayload | null;
  canvasRef: RefObject<HTMLCanvasElement | null>;
  canvasHandlers: CanvasHandlers;
  pendingRoi: RectF32 | null;
  overlays: OverlayVisibility;
  error: string | null;
  pending: (key: string) => boolean;
  onDismissError: () => void;
  onRunChess: () => void;
  onConnectCamera: () => void;
  onRefreshCameraDevices: () => void;
  onSelectCameraDevice: (deviceId: string) => void;
  onSelectCameraFormat: (formatId: string) => void;
  onStartCamera: () => void;
  onStopCamera: () => void;
  onSetRequestedFps: (fps: number) => void;
  onSelectAlgorithm: (algorithm: AlgorithmId) => void;
  onSetRingGridTargetConfig: (config: RingGridTargetConfig) => void;
  onToggleOverlay: (key: OverlayKey) => void;
  onClearRoi: () => void;
  onCaptureTemplate: () => void;
  onStartProcessing: () => void;
  onStopProcessing: () => void;
  onStartRecording: () => void;
  onStopRecording: () => void;
};

export function AppShell({
  view,
  frame,
  canvasRef,
  canvasHandlers,
  pendingRoi,
  overlays,
  error,
  pending,
  onDismissError,
  onRunChess,
  onConnectCamera,
  onRefreshCameraDevices,
  onSelectCameraDevice,
  onSelectCameraFormat,
  onStartCamera,
  onStopCamera,
  onSetRequestedFps,
  onSelectAlgorithm,
  onSetRingGridTargetConfig,
  onToggleOverlay,
  onClearRoi,
  onCaptureTemplate,
  onStartProcessing,
  onStopProcessing,
  onStartRecording,
  onStopRecording,
}: AppShellProps) {
  const [activeTab, setActiveTab] = useState<OperatorTab>("canvas");
  const camera = view?.camera.value;
  const vision = view?.vision.value;
  const recorder = view?.recorder.value;
  const events = orderedRecentEvents(view, 40);

  return (
    <main className="flex h-screen min-h-0 flex-col gap-3 overflow-hidden bg-app p-3 text-text">
      <header className="grid min-h-14 gap-3 rounded-[9px] border border-border bg-surface px-3 md:grid-cols-[auto_minmax(0,1fr)_auto] md:items-stretch">
        <div className="flex min-w-[150px] items-center gap-3 py-2">
          <span className="size-3 rotate-45 rounded-[2px] bg-accent" />
          <div>
            <h1 className="text-sm font-bold text-text">Vision Lab</h1>
            <p className="text-xs text-muted">Operator console</p>
          </div>
        </div>
        <nav className="flex min-w-0 overflow-x-auto" aria-label="Operator sections">
          {tabs.map((tab) => (
            <button
              className={topTabClass(activeTab === tab.id)}
              key={tab.id}
              onClick={() => setActiveTab(tab.id)}
              type="button"
            >
              {tab.label}
            </button>
          ))}
        </nav>
        <div className="grid min-w-0 gap-2 py-2 sm:grid-cols-2 lg:min-w-[560px] lg:grid-cols-[1fr_1fr_1fr_82px]">
          <ComponentHealth label="Camera" state={camera} />
          <ComponentHealth label="Vision" state={vision} />
          <ComponentHealth label="Recorder" state={recorder} />
          <div className="border-l-2 border-border px-3">
            <p className="text-xs text-muted">Resyncs</p>
            <strong className="mt-1 block font-mono text-sm font-semibold text-text">
              {view?.resync_count ?? 0}
            </strong>
          </div>
        </div>
      </header>

      {error ? (
        <section className="flex items-center justify-between gap-3 rounded-[9px] border border-danger/60 bg-danger/15 px-4 py-3 text-sm text-danger-text">
          <p>{error}</p>
          <Button onClick={onDismissError} variant="ghost">
            Dismiss
          </Button>
        </section>
      ) : null}

      <section className="min-h-0 flex-1 overflow-hidden rounded-[9px] border border-border bg-surface">
        {activeTab === "canvas" ? (
          <CanvasWorkspace
            camera={camera}
            canvasHandlers={canvasHandlers}
            canvasRef={canvasRef}
            frame={frame}
            onCaptureTemplate={onCaptureTemplate}
            onClearRoi={onClearRoi}
            onConnectCamera={onConnectCamera}
            onOpenAlgorithmConfig={() => setActiveTab("algorithm")}
            onRunChess={onRunChess}
            onSelectAlgorithm={onSelectAlgorithm}
            onSetRequestedFps={onSetRequestedFps}
            onStartCamera={onStartCamera}
            onStartProcessing={onStartProcessing}
            onStartRecording={onStartRecording}
            onStopCamera={onStopCamera}
            onStopProcessing={onStopProcessing}
            onStopRecording={onStopRecording}
            onToggleOverlay={onToggleOverlay}
            overlays={overlays}
            pending={pending}
            pendingRoi={pendingRoi}
            recorder={recorder}
            vision={vision}
          />
        ) : null}
        {activeTab === "algorithm" ? (
          <AlgorithmWorkspace
            onCaptureTemplate={onCaptureTemplate}
            onClearRoi={onClearRoi}
            onRunChess={onRunChess}
            onSelectAlgorithm={onSelectAlgorithm}
            onSetRingGridTargetConfig={onSetRingGridTargetConfig}
            onStartProcessing={onStartProcessing}
            onStopProcessing={onStopProcessing}
            pending={pending}
            pendingRoi={pendingRoi}
            vision={vision}
          />
        ) : null}
        {activeTab === "camera" ? (
          <CameraWorkspace
            camera={camera}
            onConnectCamera={onConnectCamera}
            onRefreshCameraDevices={onRefreshCameraDevices}
            onSelectCameraDevice={onSelectCameraDevice}
            onSelectCameraFormat={onSelectCameraFormat}
            onSetRequestedFps={onSetRequestedFps}
            onStartCamera={onStartCamera}
            onStopCamera={onStopCamera}
            pending={pending}
          />
        ) : null}
        {activeTab === "health" ? (
          <HealthWorkspace
            camera={camera}
            events={events}
            recorder={recorder}
            resyncCount={view?.resync_count ?? 0}
            vision={vision}
          />
        ) : null}
      </section>
    </main>
  );
}

function CanvasWorkspace({
  camera,
  vision,
  recorder,
  frame,
  canvasRef,
  canvasHandlers,
  pendingRoi,
  overlays,
  pending,
  onToggleOverlay,
  onSelectAlgorithm,
  onOpenAlgorithmConfig,
  onRunChess,
  onConnectCamera,
  onStartCamera,
  onStopCamera,
  onSetRequestedFps,
  onClearRoi,
  onCaptureTemplate,
  onStartProcessing,
  onStopProcessing,
  onStartRecording,
  onStopRecording,
}: CanvasWorkspaceProps) {
  const selectedAlgorithm = runnableSelection(vision?.selected_algorithm);

  return (
    <div className="grid h-full min-h-0 grid-cols-[64px_minmax(0,1fr)] lg:grid-cols-[66px_minmax(0,1fr)_340px]">
      <ToolRail selectedAlgorithm={selectedAlgorithm} onSelectAlgorithm={onSelectAlgorithm} />
      <LiveViewport
        camera={camera}
        canvasHandlers={canvasHandlers}
        canvasRef={canvasRef}
        frame={frame}
        onToggleOverlay={onToggleOverlay}
        overlays={overlays}
        recorder={recorder}
        vision={vision}
      />
      <aside className="hidden min-h-0 overflow-y-auto border-l border-border bg-surface lg:block">
        <VisionControls
          onCaptureTemplate={onCaptureTemplate}
          onClearRoi={onClearRoi}
          onRunChess={onRunChess}
          onSelectAlgorithm={onSelectAlgorithm}
          onOpenAlgorithmConfig={onOpenAlgorithmConfig}
          onStartProcessing={onStartProcessing}
          onStopProcessing={onStopProcessing}
          pending={pending}
          pendingRoi={pendingRoi}
          vision={vision}
        />
        <CameraControls
          camera={camera}
          onConnect={onConnectCamera}
          onSetRequestedFps={onSetRequestedFps}
          onStart={onStartCamera}
          onStop={onStopCamera}
          pending={pending}
        />
        <RecorderControls
          onStartRecording={onStartRecording}
          onStopRecording={onStopRecording}
          pending={pending}
          recorder={recorder}
        />
      </aside>
    </div>
  );
}

function AlgorithmWorkspace({
  vision,
  pendingRoi,
  pending,
  onSelectAlgorithm,
  onSetRingGridTargetConfig,
  onClearRoi,
  onCaptureTemplate,
  onRunChess,
  onStartProcessing,
  onStopProcessing,
}: AlgorithmWorkspaceProps) {
  const selectedAlgorithm = runnableSelection(vision?.selected_algorithm);

  return (
    <div className="grid h-full min-h-0 lg:grid-cols-[240px_minmax(0,1fr)]">
      <aside className="min-h-0 overflow-y-auto border-b border-border bg-surface-muted p-2 lg:border-b-0 lg:border-r">
        <ToolList selectedAlgorithm={selectedAlgorithm} onSelectAlgorithm={onSelectAlgorithm} />
      </aside>
      <div className="min-h-0 overflow-y-auto">
        <div className="mx-auto grid max-w-5xl gap-3 p-3 lg:grid-cols-[380px_minmax(0,1fr)]">
          <VisionControls
            onCaptureTemplate={onCaptureTemplate}
            onClearRoi={onClearRoi}
            onRunChess={onRunChess}
            onSelectAlgorithm={onSelectAlgorithm}
            onStartProcessing={onStartProcessing}
            onStopProcessing={onStopProcessing}
            pending={pending}
            pendingRoi={pendingRoi}
            vision={vision}
          />
          <div className="grid content-start gap-3">
            {selectedAlgorithm === "RingGridTarget" ? (
              <RingGridConfigPanel
                key={JSON.stringify(vision?.ringgrid_target)}
                onApply={onSetRingGridTargetConfig}
                pending={pending}
                vision={vision}
              />
            ) : null}
            <Panel eyebrow="State" title={algorithmLabel(selectedAlgorithm)}>
              <MetricGrid
                items={[
                  { label: "Lifecycle", value: vision?.lifecycle ?? "Offline" },
                  {
                    label: "Input",
                    value: `${(vision?.input_fps ?? 0).toFixed(1)} fps`,
                  },
                  {
                    label: "Processing",
                    value: `${(vision?.processing_fps ?? 0).toFixed(1)} fps`,
                  },
                  {
                    label: "Latency",
                    value: `${(vision?.mean_latency_ms ?? 0).toFixed(2)} ms`,
                  },
                  {
                    label: "Dropped",
                    tone: (vision?.dropped_input_frames ?? 0) > 0 ? "warn" : "neutral",
                    value: String(vision?.dropped_input_frames ?? 0),
                  },
                  {
                    label: "Points",
                    value: String(vision?.last_detection?.points.length ?? 0),
                  },
                ]}
              />
            </Panel>
          </div>
        </div>
      </div>
    </div>
  );
}

function CameraWorkspace({
  camera,
  pending,
  onConnectCamera,
  onRefreshCameraDevices,
  onSelectCameraDevice,
  onSelectCameraFormat,
  onStartCamera,
  onStopCamera,
  onSetRequestedFps,
}: CameraWorkspaceProps) {
  return (
    <div className="h-full min-h-0 overflow-y-auto">
      <div className="mx-auto grid max-w-6xl gap-3 p-3 lg:grid-cols-[360px_minmax(0,1fr)]">
        <CameraControls
          camera={camera}
          onConnect={onConnectCamera}
          onSetRequestedFps={onSetRequestedFps}
          onStart={onStartCamera}
          onStop={onStopCamera}
          pending={pending}
        />
        <CameraConfigPanel
          camera={camera}
          onRefreshDevices={onRefreshCameraDevices}
          onSelectDevice={onSelectCameraDevice}
          onSelectFormat={onSelectCameraFormat}
          pending={pending}
        />
      </div>
    </div>
  );
}

function HealthWorkspace({ camera, vision, recorder, events, resyncCount }: HealthWorkspaceProps) {
  return (
    <div className="h-full min-h-0 overflow-y-auto">
      <div className="grid gap-3 p-3 xl:grid-cols-[minmax(0,1fr)_420px]">
        <div className="grid gap-3">
          <Panel eyebrow="Health" title="Components">
            <div className="grid gap-3 md:grid-cols-3">
              <ComponentHealth label="Camera" state={camera} />
              <ComponentHealth label="Vision" state={vision} />
              <ComponentHealth label="Recorder" state={recorder} />
            </div>
          </Panel>
          <Panel eyebrow="Counters" title="System">
            <MetricGrid
              items={[
                { label: "Resyncs", value: String(resyncCount) },
                { label: "Frame", value: String(camera?.frame_id ?? 0) },
                {
                  label: "Capture drops",
                  tone: (camera?.dropped_frames ?? 0) > 0 ? "warn" : "neutral",
                  value: String(camera?.dropped_frames ?? 0),
                },
                {
                  label: "Vision drops",
                  tone: (vision?.dropped_input_frames ?? 0) > 0 ? "warn" : "neutral",
                  value: String(vision?.dropped_input_frames ?? 0),
                },
                { label: "Recorded frames", value: String(recorder?.recorded_frames ?? 0) },
                {
                  label: "Recorder drops",
                  tone: (recorder?.dropped_frames ?? 0) > 0 ? "warn" : "neutral",
                  value: String(recorder?.dropped_frames ?? 0),
                },
              ]}
            />
          </Panel>
        </div>
        <EventTimeline events={events} />
      </div>
    </div>
  );
}

function ToolRail({
  selectedAlgorithm,
  onSelectAlgorithm,
}: {
  selectedAlgorithm: AlgorithmId;
  onSelectAlgorithm: (algorithm: AlgorithmId) => void;
}) {
  return (
    <aside className="flex min-h-0 flex-col gap-1 border-r border-border bg-surface-muted py-2">
      {toolItems.map((tool) => {
        const active = selectedAlgorithm === tool.algorithm;
        return (
          <button
            aria-label={`Select ${tool.label}`}
            aria-pressed={active}
            className={railButtonClass(active)}
            key={tool.algorithm}
            onClick={() => onSelectAlgorithm(tool.algorithm)}
            title={tool.label}
            type="button"
          >
            <span className="grid size-5 place-items-center [&_svg]:size-5">{tool.icon}</span>
            <span className="max-w-full truncate text-[9px] font-medium">{tool.shortLabel}</span>
          </button>
        );
      })}
    </aside>
  );
}

function ToolList({
  selectedAlgorithm,
  onSelectAlgorithm,
}: {
  selectedAlgorithm: AlgorithmId;
  onSelectAlgorithm: (algorithm: AlgorithmId) => void;
}) {
  return (
    <div className="grid gap-1">
      <p className="px-3 py-2 text-[10px] font-semibold uppercase tracking-[0.07em] text-muted">
        Runnable tools
      </p>
      {toolItems.map((tool) => {
        const active = selectedAlgorithm === tool.algorithm;
        return (
          <button
            aria-pressed={active}
            className={toolListClass(active)}
            key={tool.algorithm}
            onClick={() => onSelectAlgorithm(tool.algorithm)}
            type="button"
          >
            <span className="grid size-7 place-items-center rounded-md border border-border bg-canvas text-accent-text [&_svg]:size-4">
              {tool.icon}
            </span>
            <span className="min-w-0 flex-1 text-left">
              <span className="block truncate text-sm font-medium">{tool.label}</span>
              <span className="block truncate text-xs text-muted">{tool.family}</span>
            </span>
          </button>
        );
      })}
    </div>
  );
}

function runnableSelection(algorithm: AlgorithmId | undefined): AlgorithmId {
  return algorithm && runnableAlgorithms.includes(algorithm) ? algorithm : "ChessCorners";
}

function topTabClass(active: boolean) {
  return [
    "inline-flex min-h-14 items-center border-b-2 px-3 text-sm font-medium transition-colors",
    "focus-visible:outline-2 focus-visible:outline-inset focus-visible:outline-focus",
    active
      ? "border-accent text-text"
      : "border-transparent text-muted hover:bg-surface-strong hover:text-text",
  ].join(" ");
}

function railButtonClass(active: boolean) {
  return [
    "flex min-h-[58px] flex-col items-center justify-center gap-1 border-l-2 px-1 transition-colors",
    "focus-visible:outline-2 focus-visible:outline-inset focus-visible:outline-focus",
    active
      ? "border-accent bg-surface text-accent-text"
      : "border-transparent text-muted hover:bg-surface hover:text-text",
  ].join(" ");
}

function toolListClass(active: boolean) {
  return [
    "flex min-h-12 items-center gap-3 px-3 py-2 transition-colors",
    "focus-visible:outline-2 focus-visible:outline-inset focus-visible:outline-focus",
    active
      ? "bg-accent/10 text-accent-text shadow-[inset_2px_0_0_var(--color-accent)]"
      : "text-text hover:bg-surface-strong",
  ].join(" ");
}

const tabs: { id: OperatorTab; label: string }[] = [
  { id: "canvas", label: "Canvas" },
  { id: "algorithm", label: "Algorithm" },
  { id: "camera", label: "Camera" },
  { id: "health", label: "Logs & health" },
];

const toolItems: {
  algorithm: AlgorithmId;
  label: string;
  shortLabel: string;
  family: string;
  icon: ReactNode;
}[] = [
  {
    algorithm: "ChessCorners",
    family: "Corners",
    icon: <ScanSearch />,
    label: "Chess corners",
    shortLabel: "Corners",
  },
  {
    algorithm: "RadialSymmetry",
    family: "Circles",
    icon: <CircleDot />,
    label: "Radial symmetry",
    shortLabel: "Circles",
  },
  {
    algorithm: "CalibrationTarget",
    family: "Calibration",
    icon: <ScanLine />,
    label: "Calibration target",
    shortLabel: "Calib",
  },
  {
    algorithm: "RingGridTarget",
    family: "Calibration",
    icon: <Grid3X3 />,
    label: "RingGrid target",
    shortLabel: "RingGrid",
  },
  {
    algorithm: "TemplateNcc",
    family: "Pattern matching",
    icon: <Radio />,
    label: "Template NCC",
    shortLabel: "Pattern",
  },
];

type CanvasWorkspaceProps = Pick<
  AppShellProps,
  | "canvasRef"
  | "canvasHandlers"
  | "frame"
  | "pendingRoi"
  | "overlays"
  | "pending"
  | "onToggleOverlay"
  | "onSelectAlgorithm"
  | "onRunChess"
  | "onConnectCamera"
  | "onStartCamera"
  | "onStopCamera"
  | "onSetRequestedFps"
  | "onClearRoi"
  | "onCaptureTemplate"
  | "onStartProcessing"
  | "onStopProcessing"
  | "onStartRecording"
  | "onStopRecording"
> & {
  camera: SystemView["camera"]["value"] | undefined;
  vision: SystemView["vision"]["value"] | undefined;
  recorder: SystemView["recorder"]["value"] | undefined;
  onOpenAlgorithmConfig: () => void;
};

type AlgorithmWorkspaceProps = Pick<
  AppShellProps,
  | "pendingRoi"
  | "pending"
  | "onSelectAlgorithm"
  | "onSetRingGridTargetConfig"
  | "onClearRoi"
  | "onCaptureTemplate"
  | "onRunChess"
  | "onStartProcessing"
  | "onStopProcessing"
> & {
  vision: SystemView["vision"]["value"] | undefined;
};

type CameraWorkspaceProps = Pick<
  AppShellProps,
  | "pending"
  | "onConnectCamera"
  | "onRefreshCameraDevices"
  | "onSelectCameraDevice"
  | "onSelectCameraFormat"
  | "onStartCamera"
  | "onStopCamera"
  | "onSetRequestedFps"
> & {
  camera: SystemView["camera"]["value"] | undefined;
};

type HealthWorkspaceProps = {
  camera: SystemView["camera"]["value"] | undefined;
  vision: SystemView["vision"]["value"] | undefined;
  recorder: SystemView["recorder"]["value"] | undefined;
  events: ReturnType<typeof orderedRecentEvents>;
  resyncCount: number;
};
