import { useCallback, useState } from "react";

import { errorMessage } from "../tauri/commands";

type CommandAction = () => Promise<unknown>;

export function useCommandStatus() {
  const [pendingByKey, setPendingByKey] = useState<Record<string, boolean>>({});
  const [error, setError] = useState<string | null>(null);

  const execute = useCallback(async (key: string, action: CommandAction) => {
    setError(null);
    setPendingByKey((current) => ({ ...current, [key]: true }));
    try {
      await action();
    } catch (caught) {
      setError(errorMessage(caught));
    } finally {
      setPendingByKey((current) => ({ ...current, [key]: false }));
    }
  }, []);

  const isPending = useCallback((key: string) => pendingByKey[key] === true, [pendingByKey]);
  const clearError = useCallback(() => setError(null), []);

  return { execute, isPending, error, clearError };
}
