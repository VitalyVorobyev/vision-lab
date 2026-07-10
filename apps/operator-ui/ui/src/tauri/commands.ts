import { invoke } from "@tauri-apps/api/core";

import type { FramePayload } from "../domain/camera";
import type { RectF32 } from "../domain/geometry";
import type { RecordedFrame, RecordedSession } from "../domain/replay";
import { emptySystemView, type SystemView } from "../domain/system";
import type { AlgorithmId, RingGridTargetConfig } from "../domain/vision";
import { isTauriHost } from "./host";

export function getSystemView() {
  if (!isTauriHost()) return Promise.resolve<SystemView>(emptySystemView);
  return invoke<SystemView>("system_view");
}

export function getLatestFrame() {
  if (!isTauriHost()) return Promise.resolve<FramePayload | null>(null);
  return invoke<FramePayload | null>("latest_frame");
}

export function getLatestReplayFrame() {
  if (!isTauriHost()) return Promise.resolve<FramePayload | null>(null);
  return invoke<FramePayload | null>("latest_replay_frame");
}

export function getRecordedSessions() {
  if (!isTauriHost()) return Promise.resolve<RecordedSession[]>([]);
  return invoke<RecordedSession[]>("recorded_sessions");
}

export function getRecordedSessionFrames(sessionId: string) {
  if (!isTauriHost()) return Promise.resolve<RecordedFrame[]>([]);
  return invoke<RecordedFrame[]>("recorded_session_frames", { sessionId });
}

export function selectRecordedFrame(sessionId: string, frameId: number) {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("select_recorded_frame", { sessionId, frameId });
}

export function connectCamera() {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("connect_camera");
}

export function refreshCameraDevices() {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("refresh_camera_devices");
}

export function selectCameraDevice(deviceId: string) {
  if (!isTauriHost()) return Promise.resolve(deviceId);
  return invoke("select_camera_device", { deviceId });
}

export function selectCameraFormat(formatId: string) {
  if (!isTauriHost()) return Promise.resolve(formatId);
  return invoke("select_camera_format", { formatId });
}

export function startCamera() {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("start_camera");
}

export function stopCamera() {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("stop_camera");
}

export function setRequestedFps(fps: number) {
  if (!isTauriHost()) return Promise.resolve(fps);
  return invoke("set_requested_fps", { fps });
}

export function selectAlgorithm(algorithm: AlgorithmId) {
  if (!isTauriHost()) return Promise.resolve(algorithm);
  return invoke("select_algorithm", { algorithm });
}

export function setRingGridTargetConfig(config: RingGridTargetConfig) {
  if (!isTauriHost()) return Promise.resolve(config);
  return invoke("set_ringgrid_target_config", { config });
}

export function setRoi(roi: RectF32 | null) {
  if (!isTauriHost()) return Promise.resolve(roi);
  return invoke("set_roi", { roi });
}

export function captureTemplate() {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("capture_template");
}

export function startProcessing() {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("start_processing");
}

export function stopProcessing() {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("stop_processing");
}

export function startRecording(maxFps: number) {
  if (!isTauriHost()) return Promise.resolve(maxFps);
  return invoke("start_recording", { maxFps });
}

export function stopRecording() {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("stop_recording");
}

export function errorMessage(error: unknown): string {
  return error instanceof Error ? error.message : String(error);
}
