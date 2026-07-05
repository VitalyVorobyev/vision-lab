import type { RefObject } from "react";
import { useEffect } from "react";

import type { FramePayload } from "../domain/camera";
import type { RectF32 } from "../domain/geometry";
import type { Detection } from "../domain/vision";

type FrameCanvasInput = {
  canvasRef: RefObject<HTMLCanvasElement | null>;
  frame: FramePayload | null;
  roi: RectF32 | null;
  detection: Detection | null;
};

export function useFrameCanvas({ canvasRef, frame, roi, detection }: FrameCanvasInput) {
  useEffect(() => {
    drawFrame(canvasRef.current, frame, roi, detection);
  }, [canvasRef, frame, roi, detection]);
}

function drawFrame(
  canvas: HTMLCanvasElement | null,
  frame: FramePayload | null,
  roi: RectF32 | null,
  detection: Detection | null,
) {
  if (!canvas) return;
  const context = canvas.getContext("2d");
  if (!context) return;

  if (!frame) {
    canvas.width = 640;
    canvas.height = 480;
    context.fillStyle = "#0d1116";
    context.fillRect(0, 0, canvas.width, canvas.height);
    drawEmptyState(context, canvas);
    return;
  }

  canvas.width = frame.meta.width;
  canvas.height = frame.meta.height;
  const bytes = Uint8Array.from(atob(frame.data_base64), (char) => char.charCodeAt(0));
  const image = context.createImageData(frame.meta.width, frame.meta.height);

  if (frame.meta.pixel_format === "Rgb8") {
    drawRgb(bytes, image);
  } else {
    drawGray(bytes, image);
  }

  context.putImageData(image, 0, 0);
  if (roi) drawRect(context, roi, "#d7a936");
  if (detection) drawDetection(context, detection);
}

function drawGray(bytes: Uint8Array, image: ImageData) {
  const pixels = image.width * image.height;
  for (let index = 0; index < pixels; index += 1) {
    const gray = bytes[index] ?? 0;
    const offset = index * 4;
    image.data[offset] = gray;
    image.data[offset + 1] = gray;
    image.data[offset + 2] = gray;
    image.data[offset + 3] = 255;
  }
}

function drawRgb(bytes: Uint8Array, image: ImageData) {
  const pixels = image.width * image.height;
  for (let index = 0; index < pixels; index += 1) {
    const sourceOffset = index * 3;
    const targetOffset = index * 4;
    image.data[targetOffset] = bytes[sourceOffset] ?? 0;
    image.data[targetOffset + 1] = bytes[sourceOffset + 1] ?? 0;
    image.data[targetOffset + 2] = bytes[sourceOffset + 2] ?? 0;
    image.data[targetOffset + 3] = 255;
  }
}

function drawRect(context: CanvasRenderingContext2D, rect: RectF32, color: string) {
  context.strokeStyle = color;
  context.lineWidth = Math.max(2, Math.round(context.canvas.width / 400));
  context.strokeRect(rect.x, rect.y, rect.width, rect.height);
}

function drawDetection(context: CanvasRenderingContext2D, detection: Detection) {
  if (detection.bbox) drawRect(context, detection.bbox, "#4fb286");
  const radius = Math.max(2, Math.round(context.canvas.width / 220));
  context.fillStyle = "#8ee0bb";
  context.strokeStyle = "#0d1116";
  context.lineWidth = Math.max(1, Math.round(radius / 2));
  for (const point of detection.points ?? []) {
    context.beginPath();
    context.arc(point.x, point.y, radius, 0, Math.PI * 2);
    context.fill();
    context.stroke();
  }
}

function drawEmptyState(context: CanvasRenderingContext2D, canvas: HTMLCanvasElement) {
  context.fillStyle = "#687482";
  context.font = "16px ui-sans-serif, system-ui, sans-serif";
  context.textAlign = "center";
  context.fillText("Waiting for camera frames", canvas.width / 2, canvas.height / 2);
}
