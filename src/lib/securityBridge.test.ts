import { describe, expect, it } from "vitest";
import { acceptsSecurityStatus, activityReporter, lockMessage } from "./securityBridge";
import { lockEventSchema, validateCommandResponse } from "./schemas";

describe("后端锁定与页面会话边界", () => {
  it("拒绝晚到的旧会话解锁响应，允许真正的新会话", () => {
    expect(acceptsSecurityStatus(4, { sessionEpoch: 3, unlocked: true }, true)).toBe(false);
    expect(acceptsSecurityStatus(4, { sessionEpoch: 4, unlocked: true }, true)).toBe(false);
    expect(acceptsSecurityStatus(4, { sessionEpoch: 4, unlocked: false }, true)).toBe(true);
    expect(acceptsSecurityStatus(4, { sessionEpoch: 5, unlocked: true }, true)).toBe(true);
  });
  it("只发送真实可见窗口中的用户活动，不发送保活心跳", () => {
    let time = 0;
    let sent = 0;
    const report = activityReporter(() => sent++, () => time);
    report(false, true);
    report(true, false);
    expect(sent).toBe(0);
    report(true, true);
    for (time = 1; time < 1000; time++) report(true, true);
    expect(sent).toBe(1);
    report(true, true);
    expect(sent).toBe(2);
  });
  it("通知只能锁定且必须有有效会话代号", () => {
    expect(lockEventSchema.safeParse({ sessionEpoch: 2, unlocked: false, lockReason: "system" }).success).toBe(true);
    expect(lockEventSchema.safeParse({ sessionEpoch: 2, unlocked: true }).success).toBe(false);
    expect(lockEventSchema.safeParse({ sessionEpoch: -1, unlocked: false }).success).toBe(false);
    expect(lockEventSchema.safeParse({ unlocked: false }).success).toBe(false);
    expect(validateCommandResponse("security_activity", null)).toBe(null);
  });
  it("区分后端闲置锁与系统返回后重新认证", () => {
    expect(lockMessage("idle")).toContain("后端锁定");
    expect(lockMessage("systemResume")).toContain("重新解锁");
    expect(lockMessage("manual")).toBe(null);
  });
});
