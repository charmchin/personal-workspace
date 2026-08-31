import { describe, expect, it } from "vitest";
import { BACKUP_PASSWORD_MIN_LENGTH, LOCAL_PASSWORD_MIN_LENGTH, passwordCharacterCount } from "./security";

describe("工作台口令长度", () => {
  it("最低长度为六个字符，并按 Unicode 字符计数", () => {
    expect(LOCAL_PASSWORD_MIN_LENGTH).toBe(6);
    expect(BACKUP_PASSWORD_MIN_LENGTH).toBe(10);
    expect(passwordCharacterCount("安全口令12")).toBe(6);
    expect(passwordCharacterCount("🔐🔐🔐🔐🔐🔐")).toBe(6);
  });
});
