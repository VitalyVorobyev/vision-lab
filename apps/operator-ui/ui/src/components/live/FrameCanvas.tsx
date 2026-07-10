import type { CSSProperties, PointerEventHandler, RefObject } from "react";

import type { FramePayload } from "../../domain/camera";

type CanvasHandlers = {
  onPointerDown: PointerEventHandler<HTMLCanvasElement>;
  onPointerMove: PointerEventHandler<HTMLCanvasElement>;
  onPointerUp: PointerEventHandler<HTMLCanvasElement>;
  onPointerCancel: PointerEventHandler<HTMLCanvasElement>;
  onPointerLeave: PointerEventHandler<HTMLCanvasElement>;
};

export function FrameCanvas({
  canvasRef,
  handlers,
  frame,
}: {
  canvasRef: RefObject<HTMLCanvasElement | null>;
  handlers?: CanvasHandlers;
  frame: FramePayload | null;
}) {
  const style: CSSProperties = frame
    ? { aspectRatio: `${frame.meta.width} / ${frame.meta.height}` }
    : { aspectRatio: "4 / 3" };

  return (
    <canvas
      aria-label="Live camera frame with ROI and detection overlays"
      className="block h-full max-h-full w-auto max-w-full cursor-crosshair bg-canvas [image-rendering:pixelated]"
      height={480}
      ref={canvasRef}
      style={style}
      width={640}
      {...handlers}
    />
  );
}
