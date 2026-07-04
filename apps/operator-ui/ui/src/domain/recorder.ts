export type RecorderState = {
  lifecycle: string;
  session_path?: string | null;
  recorded_frames: number;
  recorded_detections: number;
  dropped_frames: number;
  error?: string | null;
};
