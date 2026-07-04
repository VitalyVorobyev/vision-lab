import { invoke } from "@tauri-apps/api/core";

import type { FramePayload } from "../domain/camera";
import type { RectF32 } from "../domain/geometry";
import { emptySystemView, type SystemView } from "../domain/system";
import type { AlgorithmId } from "../domain/vision";
import { isTauriHost } from "./host";

export function getSystemView() {
  if (!isTauriHost()) return Promise.resolve<SystemView>(emptySystemView);
  return invoke<SystemView>("system_view");
}

export function getLatestFrame() {
  if (!isTauriHost()) return Promise.resolve<FramePayload | null>(null);
  return invoke<FramePayload | null>("latest_frame");
}

export function connectCamera() {
  if (!isTauriHost()) return Promise.resolve();
  return invoke("connect_camera");
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
