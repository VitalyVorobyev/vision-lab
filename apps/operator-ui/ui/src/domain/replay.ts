import type { FrameMeta } from "./camera";

export type RecordedSession = {
  id: string;
  created_at_ms: number;
  frame_count: number;
  detection_count: number;
};

export type RecordedFrame = {
  meta: FrameMeta;
};

export function sessionLabel(session: RecordedSession): string {
  return new Date(session.created_at_ms).toLocaleString();
}
