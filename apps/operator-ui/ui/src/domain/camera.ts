export type PixelFormat = "Gray8" | "Rgb8";

export type FrameMeta = {
  frame_id: number;
  timestamp: number;
  width: number;
  height: number;
  stride: number;
  pixel_format: PixelFormat;
};

export type FramePayload = {
  meta: FrameMeta;
  data_base64: string;
};

export type CameraState = {
  lifecycle: string;
  requested_fps: number;
  actual_fps: number;
  frame_width: number;
  frame_height: number;
  frame_id: number;
  dropped_frames: number;
  error?: string | null;
};

export function frameSizeLabel(frame: FramePayload | null): string {
  if (!frame) return "No frame";
  return `${frame.meta.width}x${frame.meta.height}`;
}
