import type { PointerEventHandler, RefObject } from "react";

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
}: {
  canvasRef: RefObject<HTMLCanvasElement | null>;
  handlers: CanvasHandlers;
}) {
  return (
    <canvas
      aria-label="Live camera frame with ROI and detection overlays"
      className="h-full min-h-[360px] w-full cursor-crosshair bg-canvas object-contain [image-rendering:pixelated]"
      height={480}
      ref={canvasRef}
      width={640}
      {...handlers}
    />
  );
}
