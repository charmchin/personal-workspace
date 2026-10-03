import { useEffect, useRef } from "react";
import type { SearchResult } from "../types";

export function useSearchTarget<T extends { id: string }>(target: SearchResult | null | undefined, kind: string, records: T[], open: (record: T) => void) {
  const consumed = useRef<SearchResult | null>(null);
  useEffect(() => {
    if (!target || target.kind !== kind || consumed.current === target) return;
    const record = records.find((item) => item.id === target.id);
    if (record) { consumed.current = target; open(record); }
  }, [target, kind, records, open]);
}
