import type { CameraState } from "./camera";
import type { RecorderState } from "./recorder";
import type { VisionState } from "./vision";

export type Versioned<T> = {
  revision: number;
  value: T;
};

export type EventSummary = {
  sequence: number;
  summary: string;
  correlation_id?: string | null;
  source: {
    component: {
      component_type: string;
      component_name: string;
    };
  };
};

export type SystemView = {
  camera: Versioned<CameraState>;
  vision: Versioned<VisionState>;
  recorder: Versioned<RecorderState>;
  recent_events: EventSummary[];
  resync_count: number;
};

export const emptySystemView: SystemView = {
  camera: {
    revision: 0,
    value: {
      lifecycle: "Offline",
      available_devices: [],
      active_device_id: null,
      active_format_id: null,
      permission_status: "Unknown",
      requested_fps: 30,
      actual_fps: 0,
      frame_width: 0,
      frame_height: 0,
      frame_id: 0,
      dropped_frames: 0,
      error: null,
    },
  },
  vision: {
    revision: 0,
    value: {
      lifecycle: "Offline",
      selected_algorithm: "ChessCorners",
      roi: null,
      has_template: false,
      input_fps: 0,
      processing_fps: 0,
      mean_latency_ms: 0,
      dropped_input_frames: 0,
      last_detection: null,
      error: null,
    },
  },
  recorder: {
    revision: 0,
    value: {
      lifecycle: "Offline",
      session_path: null,
      recorded_frames: 0,
      recorded_detections: 0,
      dropped_frames: 0,
      error: null,
    },
  },
  recent_events: [],
  resync_count: 0,
};

export function orderedRecentEvents(view: SystemView | null, limit = 18): EventSummary[] {
  return [...(view?.recent_events ?? [])].reverse().slice(0, limit);
}

export function componentHealth(state: { lifecycle?: string; error?: string | null } | undefined) {
  if (!state?.lifecycle) return "offline";
  if (state.error) return "error";
  return state.lifecycle.toLowerCase();
}
