type TauriWindow = Window & {
  __TAURI_INTERNALS__?: unknown;
};

export function isTauriHost(): boolean {
  return typeof window !== "undefined" && (window as TauriWindow).__TAURI_INTERNALS__ != null;
}
