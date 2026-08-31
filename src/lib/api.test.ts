import { describe, expect, it } from "vitest";
import { WorkbenchError } from "./api";
import { validateCommandResponse } from "./schemas";

describe("统一错误边界", () => {
  it("解析 Rust 返回的结构化错误", () => {
    const error = new WorkbenchError(JSON.stringify({
      code: "DATABASE_LOCKED",
      message: "工作台已锁定",
      recovery: "请先解锁",
    }));
    expect(error.code).toBe("DATABASE_LOCKED");
    expect(error.message).toBe("工作台已锁定");
    expect(error.recovery).toBe("请先解锁");
  });

  it("保留普通错误消息", () => {
    const error = new WorkbenchError(new Error("测试失败"));
    expect(error.code).toBe("UNKNOWN");
    expect(error.message).toBe("测试失败");
  });

  it("拒绝后端返回的错误结构", () => {
    expect(() => validateCommandResponse("security_status", {
      initialized: true,
      unlocked: "yes",
      databasePath: "/tmp/workbench.sqlite3",
      keychainMode: "passphrase",
    })).toThrow();
  });
});
