import { describe, expect, it } from "vitest";
import { WorkbenchError } from "./api";
import { validateCommandResponse } from "./schemas";

describe("统一错误边界", () => {
  it("保留恢复提示并兼容没有提示的安全状态", () => {
    const status = {
      initialized: true, unlocked: true,
      sessionEpoch: 1,
      databasePath: "/tmp/workbench.sqlite3", keychainMode: "passphrase",
    };
    expect(validateCommandResponse("security_status", status)).toEqual(status);
    expect(validateCommandResponse("security_unlock_with_password", {
      ...status, recoveryNotice: "已安全回滚到恢复前数据库",
    })).toEqual({ ...status, recoveryNotice: "已安全回滚到恢复前数据库" });
    expect(() => validateCommandResponse("security_status", {
      ...status, recoveryNotice: 123,
    })).toThrow();
  });
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
      sessionEpoch: 1,
      unlocked: "yes",
      databasePath: "/tmp/workbench.sqlite3",
      keychainMode: "passphrase",
    })).toThrow();
  });
});
