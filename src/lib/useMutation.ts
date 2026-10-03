import { useCallback, useRef, useState } from "react";
import { WorkbenchError } from "./api";

export function useMutation(onError: (error: WorkbenchError | null) => void) {
  const gate = useRef(false);
  const [busy, setBusy] = useState(false);
  const mutate = useCallback(async (operation: () => Promise<void>) => {
    if (gate.current) return false;
    gate.current = true; setBusy(true); onError(null);
    try { await operation(); return true; }
    catch (value) { onError(value instanceof WorkbenchError ? value : new WorkbenchError(value)); return false; }
    finally { gate.current = false; setBusy(false); }
  }, [onError]);
  return { busy, mutate };
}
