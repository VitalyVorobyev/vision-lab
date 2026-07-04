import type { RefObject } from "react";

import { CameraControls } from "../components/controls/CameraControls";
import { RecorderControls } from "../components/controls/RecorderControls";
import { VisionControls } from "../components/controls/VisionControls";
import { LiveViewport } from "../components/live/LiveViewport";
import { SystemStatusBar } from "../components/status/SystemStatusBar";
import { EventTimeline } from "../components/timeline/EventTimeline";
import { Button } from "../components/ui/Button";
import type { FramePayload } from "../domain/camera";
import type { RectF32 } from "../domain/geometry";
import type { SystemView } from "../domain/system";
import { orderedRecentEvents } from "../domain/system";
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
  onConnectCamera: () => void;
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
  onConnectCamera,
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
  const camera = view?.camera.value;
  const vision = view?.vision.value;
  const recorder = view?.recorder.value;

  return (
    <main className="flex min-h-screen flex-col gap-4 bg-app p-4 text-text">
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

      <section className="grid flex-1 border border-border bg-surface lg:grid-cols-[minmax(0,1fr)_380px]">
        <LiveViewport
          camera={camera}
          canvasHandlers={canvasHandlers}
          canvasRef={canvasRef}
          frame={frame}
          recorder={recorder}
          vision={vision}
        />
        <aside className="min-w-0 border-t border-border lg:border-l lg:border-t-0">
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
        </aside>
      </section>

      <EventTimeline events={orderedRecentEvents(view)} />
    </main>
  );
}
