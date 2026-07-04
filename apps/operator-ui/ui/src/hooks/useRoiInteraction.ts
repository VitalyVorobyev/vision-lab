import type { PointerEvent } from "react";
import { useCallback, useRef, useState } from "react";

import type { FrameMeta, FramePayload } from "../domain/camera";
import type { PointF32, RectF32 } from "../domain/geometry";
import { isUsableRect, rectFromPoints } from "../domain/geometry";

type RoiCommit = (roi: RectF32) => void;

export function useRoiInteraction(frame: FramePayload | null, onCommit: RoiCommit) {
  const [pendingRoi, setPendingRoi] = useState<RectF32 | null>(null);
  const dragStart = useRef<PointF32 | null>(null);

  const onPointerDown = useCallback(
    (event: PointerEvent<HTMLCanvasElement>) => {
      if (!frame) return;
      const point = canvasToImage(event.currentTarget, event, frame.meta);
      dragStart.current = point;
      setPendingRoi({ x: point.x, y: point.y, width: 0, height: 0 });
      event.currentTarget.setPointerCapture(event.pointerId);
    },
    [frame],
  );

  const onPointerMove = useCallback(
    (event: PointerEvent<HTMLCanvasElement>) => {
      if (!frame || !dragStart.current) return;
      const point = canvasToImage(event.currentTarget, event, frame.meta);
      setPendingRoi(rectFromPoints(dragStart.current, point));
    },
    [frame],
  );

  const finishDrag = useCallback(
    (event: PointerEvent<HTMLCanvasElement>) => {
      if (event.currentTarget.hasPointerCapture(event.pointerId)) {
        event.currentTarget.releasePointerCapture(event.pointerId);
      }
      dragStart.current = null;
      setPendingRoi((current) => {
        if (isUsableRect(current)) onCommit(current);
        return null;
      });
    },
    [onCommit],
  );

  const clearPendingRoi = useCallback(() => {
    dragStart.current = null;
    setPendingRoi(null);
  }, []);

  return {
    pendingRoi,
    clearPendingRoi,
    canvasHandlers: {
      onPointerDown,
      onPointerMove,
      onPointerUp: finishDrag,
      onPointerCancel: finishDrag,
      onPointerLeave: finishDrag,
    },
  };
}

function canvasToImage(
  canvas: HTMLCanvasElement,
  event: PointerEvent<HTMLCanvasElement>,
  meta: FrameMeta,
): PointF32 {
  const rect = canvas.getBoundingClientRect();
  return {
    x: ((event.clientX - rect.left) / rect.width) * meta.width,
    y: ((event.clientY - rect.top) / rect.height) * meta.height,
  };
}
