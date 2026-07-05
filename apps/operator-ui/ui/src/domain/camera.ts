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

export type CameraPermissionStatus =
  | "Unknown"
  | "NotDetermined"
  | "Authorized"
  | "Denied"
  | "Restricted";

export type CameraPosition = "Unknown" | "Front" | "Back" | "External";

export type CameraTransport = "Unknown" | "BuiltIn" | "Continuity" | "Usb" | "Virtual";

export type CameraFormatInfo = {
  id: string;
  width: number;
  height: number;
  pixel_format: PixelFormat;
  min_fps: number;
  max_fps: number;
};

export type CameraDeviceInfo = {
  id: string;
  display_name: string;
  model_id?: string | null;
  manufacturer?: string | null;
  position: CameraPosition;
  transport: CameraTransport;
  is_default: boolean;
  formats: CameraFormatInfo[];
};

export type CameraState = {
  lifecycle: string;
  available_devices: CameraDeviceInfo[];
  active_device_id?: string | null;
  active_format_id?: string | null;
  permission_status: CameraPermissionStatus;
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

export function activeCameraDevice(camera?: CameraState) {
  return camera?.available_devices.find((device) => device.id === camera.active_device_id) ?? null;
}

export function activeCameraFormat(camera?: CameraState) {
  const device = activeCameraDevice(camera);
  return device?.formats.find((format) => format.id === camera?.active_format_id) ?? null;
}
