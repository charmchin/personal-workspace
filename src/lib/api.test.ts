import { describe, expect, it } from "vitest";
import { WorkbenchError } from "./api";
import { validateCommandRequest, validateCommandResponse } from "./schemas";
import { emptyTask, emptyTransaction, defaultSettings } from "./defaults";

describe("统一错误边界", () => {
  it("写入前验证必填、金融类型和设置，不将精确十进制转成浮点数", () => {
    expect(() => validateCommandRequest("upsert_task", { task: emptyTask() })).toThrow();
    expect(() => validateCommandRequest("upsert_task", { task: { ...emptyTask(), title: "有效任务" } })).not.toThrow();
    const transaction = { ...emptyTransaction(), accountId: "account", instrumentId: "asset", quantity: "9007199254740993.123456789" };
    expect(() => validateCommandRequest("upsert_portfolio_transaction", { transaction })).not.toThrow();
    expect(() => validateCommandRequest("upsert_portfolio_transaction", { transaction: { ...transaction, quantity: 1 } })).toThrow();
    expect(() => validateCommandRequest("upsert_portfolio_transaction", { transaction: { ...transaction, quantity: "1e20" } })).toThrow();
    expect(() => validateCommandRequest("update_settings", { settings: { ...defaultSettings, lockMinutes: 7 } })).toThrow();
    expect(() => validateCommandRequest("update_settings", { settings: defaultSettings })).not.toThrow();
  });
  it("兼容旧备份响应并保留新格式的逐表校验字段", () => {
    const legacy = { formatVersion: 1, createdAt: "2026-10-03T00:00:00Z", appVersion: "0.1.3", recordCount: 1 };
    expect(validateCommandResponse("restore_backup", legacy)).toEqual(legacy);
    const current = { ...legacy, formatVersion: 2, schemaVersion: 1, tableRecordCounts: { projects: 1, settings: 1, tasks: 0 } };
    expect(validateCommandResponse("export_backup", current)).toEqual(current);
    expect(() => validateCommandResponse("restore_backup", { ...current, tableRecordCounts: { tasks: -1 } })).toThrow();
    expect(() => validateCommandResponse("restore_backup", { ...current, schemaVersion: "1" })).toThrow();
    expect(() => validateCommandResponse("restore_backup", { ...legacy, formatVersion: 2 })).toThrow();
    expect(() => validateCommandResponse("restore_backup", { ...legacy, formatVersion: 999 })).toThrow();
  });
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
