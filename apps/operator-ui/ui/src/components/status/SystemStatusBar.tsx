import type { CameraState } from "../../domain/camera";
import type { RecorderState } from "../../domain/recorder";
import type { VisionState } from "../../domain/vision";
import { ComponentHealth } from "./ComponentHealth";

export function SystemStatusBar({
  camera,
  vision,
  recorder,
  resyncCount,
}: {
  camera?: CameraState;
  vision?: VisionState;
  recorder?: RecorderState;
  resyncCount: number;
}) {
  return (
    <header className="grid gap-4 border border-border bg-surface px-4 py-3 md:grid-cols-[minmax(180px,1fr)_auto] md:items-center">
      <div>
        <h1 className="text-xl font-semibold text-text">Vision Lab</h1>
        <p className="mt-1 text-sm text-muted">Local acquisition, detection, and recording console</p>
      </div>
      <div className="grid gap-3 sm:grid-cols-2 md:min-w-[620px] md:grid-cols-[1fr_1fr_1fr_92px]">
        <ComponentHealth label="Camera" state={camera} />
        <ComponentHealth label="Vision" state={vision} />
        <ComponentHealth label="Recorder" state={recorder} />
        <div className="border-l-2 border-border px-3">
          <p className="text-xs text-muted">Resyncs</p>
          <strong className="mt-1 block text-sm font-semibold text-text">{resyncCount}</strong>
        </div>
      </div>
    </header>
  );
}
