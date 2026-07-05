import type { PointerEventHandler, RefObject } from "react";

import type { CameraState, FramePayload } from "../../domain/camera";
import type { OverlayKey, OverlayVisibility } from "../../domain/overlays";
import type { RecorderState } from "../../domain/recorder";
import type { VisionState } from "../../domain/vision";
import { FrameCanvas } from "./FrameCanvas";
import { OverlayLayer } from "./OverlayLayer";
import { ViewportToolbar } from "./ViewportToolbar";

type CanvasHandlers = {
  onPointerDown: PointerEventHandler<HTMLCanvasElement>;
  onPointerMove: PointerEventHandler<HTMLCanvasElement>;
  onPointerUp: PointerEventHandler<HTMLCanvasElement>;
  onPointerCancel: PointerEventHandler<HTMLCanvasElement>;
  onPointerLeave: PointerEventHandler<HTMLCanvasElement>;
};

export function LiveViewport({
  camera,
  vision,
  recorder,
  frame,
  canvasRef,
  canvasHandlers,
  overlays,
  onToggleOverlay,
}: {
  camera?: CameraState;
  vision?: VisionState;
  recorder?: RecorderState;
  frame: FramePayload | null;
  canvasRef: RefObject<HTMLCanvasElement | null>;
  canvasHandlers: CanvasHandlers;
  overlays: OverlayVisibility;
  onToggleOverlay: (key: OverlayKey) => void;
}) {
  return (
    <section className="flex min-h-0 min-w-0 flex-col bg-surface">
      <ViewportToolbar camera={camera} frame={frame} vision={vision} />
      <div className="relative flex min-h-0 flex-1 items-center justify-center overflow-hidden bg-canvas p-3">
        <FrameCanvas canvasRef={canvasRef} frame={frame} handlers={canvasHandlers} />
        <OverlayLayer
          detection={vision?.last_detection}
          hasFrame={frame !== null}
          onToggleOverlay={onToggleOverlay}
          overlays={overlays}
          recording={recorder?.lifecycle.toLowerCase() === "recording"}
        />
      </div>
    </section>
  );
}
