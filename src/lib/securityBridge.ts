import { useEffect } from "react";
import { listen } from "@tauri-apps/api/event";
import type { SecurityStatus } from "../types";
import { call, WorkbenchError } from "./api";
import { lockEventSchema } from "./schemas";

export interface LockEvent { sessionEpoch: number; unlocked: false; lockReason?: string | null }

export function acceptsSecurityStatus(epoch: number, incoming: Pick<SecurityStatus, "sessionEpoch" | "unlocked">, alreadyLocked: boolean): boolean {
  return incoming.sessionEpoch > epoch || (incoming.sessionEpoch === epoch && !(alreadyLocked && incoming.unlocked));
}

export function activityReporter(send: () => void, now: () => number = () => performance.now()) {
  let last = -Infinity;
  return (trusted: boolean, visible: boolean) => {
    const time = now();
    if (!trusted || !visible || time - last < 1000) return;
    last = time;
    send();
  };
}

export function lockMessage(reason?: string | null): string | null {
  if (reason === "idle") return "闲置时间已到，工作台已由本地后端锁定。";
  if (reason === "system" || reason === "systemResume") return "系统锁屏、休眠或会话切换后，工作台需要重新解锁。";
  if (reason === "hidden") return "窗口已隐藏，工作台已锁定。";
  return null;
}

export function useSecurityBridge({ unlocked, onStatus, onLocked, onFailure, onHidden }: {
  unlocked: boolean;
  onStatus: (status: SecurityStatus) => void;
  onLocked: (event: LockEvent) => void;
  onFailure: (error: WorkbenchError) => void;
  onHidden: () => void;
}) {
  useEffect(() => {
    let disposed = false;
    let unlisten: (() => void) | undefined;
    let checking = false;
    const reconcile = async () => {
      if (checking || disposed) return;
      checking = true;
      try {
        const next = await call<SecurityStatus>("security_status");
        if (!disposed) onStatus(next);
      } catch (error) {
        if (!disposed) onFailure(error as WorkbenchError);
      } finally { checking = false; }
    };
    void listen<unknown>("workbench:locked", ({ payload }) => {
      const parsed = lockEventSchema.safeParse(payload);
      if (!disposed && parsed.success) onLocked(parsed.data);
    }).then((stop) => {
      if (disposed) stop(); else { unlisten = stop; void reconcile(); }
    }).catch((error) => { if (!disposed) onFailure(new WorkbenchError(error)); });
    // Polling only checks state: it must never extend the backend idle deadline.
    const interval = unlocked ? window.setInterval(() => void reconcile(), 1000) : undefined;
    const visibility = () => { if (document.hidden && unlocked) onHidden(); else void reconcile(); };
    const requestCheck = () => void reconcile();
    window.addEventListener("focus", requestCheck);
    document.addEventListener("visibilitychange", visibility);
    window.addEventListener("workbench:security-check", requestCheck);
    return () => {
      disposed = true;
      unlisten?.();
      if (interval !== undefined) window.clearInterval(interval);
      window.removeEventListener("focus", requestCheck);
      document.removeEventListener("visibilitychange", visibility);
      window.removeEventListener("workbench:security-check", requestCheck);
    };
  }, [unlocked, onStatus, onLocked, onFailure, onHidden]);

  useEffect(() => {
    if (!unlocked) return;
    const report = activityReporter(() => { void call("security_activity").catch(() => undefined); });
    const handler = (event: Event) => report(event.isTrusted, !document.hidden);
    const events = ["pointerdown", "pointermove", "keydown", "wheel", "touchstart"] as const;
    events.forEach((event) => window.addEventListener(event, handler, { passive: true }));
    return () => events.forEach((event) => window.removeEventListener(event, handler));
  }, [unlocked]);
}
