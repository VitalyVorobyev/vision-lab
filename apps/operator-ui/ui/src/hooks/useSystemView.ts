import { useCallback, useEffect, useState } from "react";

import type { SystemView } from "../domain/system";
import { errorMessage, getSystemView } from "../tauri/commands";
import { subscribeSystemView } from "../tauri/events";

export function useSystemView() {
  const [view, setView] = useState<SystemView | null>(null);
  const [error, setError] = useState<string | null>(null);
  const clearError = useCallback(() => setError(null), []);

  const refresh = useCallback(async () => {
    try {
      setError(null);
      setView(await getSystemView());
    } catch (caught) {
      setError(errorMessage(caught));
    }
  }, []);

  useEffect(() => {
    let active = true;
    getSystemView()
      .then((nextView) => {
        if (active) setView(nextView);
      })
      .catch((caught) => {
        if (active) setError(errorMessage(caught));
      });

    const unlisten = subscribeSystemView((nextView) => {
      if (active) setView(nextView);
    });

    return () => {
      active = false;
      unlisten.then((dispose) => dispose());
    };
  }, []);

  return { view, error, refresh, clearError };
}
