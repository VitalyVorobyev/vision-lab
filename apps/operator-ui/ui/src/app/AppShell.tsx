import { useState, type RefObject } from "react";

import { CameraConfigPanel } from "../components/controls/CameraConfigPanel";
import { CameraControls } from "../components/controls/CameraControls";
import { RecorderControls } from "../components/controls/RecorderControls";
import { VisionControls } from "../components/controls/VisionControls";
import { LiveViewport } from "../components/live/LiveViewport";
import { SystemStatusBar } from "../components/status/SystemStatusBar";
import { Button } from "../components/ui/Button";
import type { FramePayload } from "../domain/camera";
import type { RectF32 } from "../domain/geometry";
import type { SystemView } from "../domain/system";
import type { AlgorithmId } from "../domain/vision";

type CanvasHandlers = Parameters<typeof LiveViewport>[0]["canvasHandlers"];

export type AppShellProps = {
  view: SystemView | null;
  frame: FramePayload | null;
  canvasRef: RefObject<HTMLCanvasElement | null>;
  canvasHandlers: CanvasHandlers;
  pendingRoi: RectF32 | null;
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
  onClearRoi,
  onCaptureTemplate,
  onStartProcessing,
  onStopProcessing,
  onStartRecording,
  onStopRecording,
}: AppShellProps) {
  const [sidebarTab, setSidebarTab] = useState<"operate" | "config">("operate");
  const camera = view?.camera.value;
  const vision = view?.vision.value;
  const recorder = view?.recorder.value;

  return (
    <main className="flex h-screen min-h-0 flex-col gap-3 overflow-hidden bg-app p-3 text-text">
      <SystemStatusBar
        camera={camera}
        recorder={recorder}
        resyncCount={view?.resync_count ?? 0}
        vision={vision}
      />

      {error ? (
        <section className="flex items-center justify-between gap-3 border border-danger/60 bg-danger/15 px-4 py-3 text-sm text-danger-text">
          <p>{error}</p>
          <Button onClick={onDismissError} variant="ghost">
            Dismiss
          </Button>
        </section>
      ) : null}

      <section className="grid min-h-0 flex-1 border border-border bg-surface md:grid-cols-[minmax(0,1fr)_380px]">
        <LiveViewport
          camera={camera}
          canvasHandlers={canvasHandlers}
          canvasRef={canvasRef}
          frame={frame}
          recorder={recorder}
          vision={vision}
        />
        <aside className="min-h-0 min-w-0 overflow-y-auto border-t border-border md:border-l md:border-t-0">
          <div className="sticky top-0 z-10 grid grid-cols-2 border-b border-border bg-surface">
            <button
              className={tabClass(sidebarTab === "operate")}
              onClick={() => setSidebarTab("operate")}
              type="button"
            >
              Operate
            </button>
            <button
              className={tabClass(sidebarTab === "config")}
              onClick={() => setSidebarTab("config")}
              type="button"
            >
              Config
            </button>
          </div>
          {sidebarTab === "operate" ? (
            <>
              <CameraControls
                camera={camera}
                onConnect={onConnectCamera}
                onSetRequestedFps={onSetRequestedFps}
                onStart={onStartCamera}
                onStop={onStopCamera}
                pending={pending}
              />
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
              <RecorderControls
                onStartRecording={onStartRecording}
                onStopRecording={onStopRecording}
                pending={pending}
                recorder={recorder}
              />
            </>
          ) : (
            <CameraConfigPanel
              camera={camera}
              onRefreshDevices={onRefreshCameraDevices}
              onSelectDevice={onSelectCameraDevice}
              onSelectFormat={onSelectCameraFormat}
              pending={pending}
            />
          )}
        </aside>
      </section>
    </main>
  );
}

function tabClass(active: boolean) {
  return [
    "min-h-10 border-b px-3 text-sm font-medium transition-colors",
    active
      ? "border-accent bg-surface-strong text-text"
      : "border-transparent text-muted hover:bg-surface-hover hover:text-text",
  ].join(" ");
}
