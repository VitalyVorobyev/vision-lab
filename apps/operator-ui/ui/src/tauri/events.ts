import { listen } from "@tauri-apps/api/event";

import type { FramePayload } from "../domain/camera";
import type { SystemView } from "../domain/system";
import { isTauriHost } from "./host";

export function subscribeSystemView(onEvent: (view: SystemView) => void) {
  if (!isTauriHost()) return Promise.resolve(() => undefined);
  return listen<SystemView>("system-view", (event) => onEvent(event.payload));
}

export function subscribeLatestFrame(onEvent: (frame: FramePayload) => void) {
  if (!isTauriHost()) return Promise.resolve(() => undefined);
  return listen<FramePayload>("frame", (event) => onEvent(event.payload));
}
