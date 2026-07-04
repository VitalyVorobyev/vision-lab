import { useCallback, useEffect, useState } from "react";

import type { FramePayload } from "../domain/camera";
import { errorMessage, getLatestFrame } from "../tauri/commands";
import { subscribeLatestFrame } from "../tauri/events";

export function useLatestFrame() {
  const [frame, setFrame] = useState<FramePayload | null>(null);
  const [error, setError] = useState<string | null>(null);
  const clearError = useCallback(() => setError(null), []);

  const refresh = useCallback(async () => {
    try {
      setError(null);
      setFrame(await getLatestFrame());
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }, []);

  useEffect(() => {
    let active = true;
    getLatestFrame()
      .then((nextFrame) => {
        if (active) setFrame(nextFrame);
      })
      .catch((caught) => {
        if (active) setError(errorMessage(caught));
      });

    const unlisten = subscribeLatestFrame((nextFrame) => {
      if (active) setFrame(nextFrame);
    });

    return () => {
      active = false;
      unlisten.then((dispose) => dispose());
    };
  }, []);

  return { frame, error, refresh, clearError };
}
