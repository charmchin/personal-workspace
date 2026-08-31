import { describe, expect, it } from "vitest";
import { localDate, money, multiplyDecimals, number, percent } from "./format";
import { emptyTransaction } from "./defaults";

describe("本地显示格式", () => {
  it("隐藏金额时不泄露数值", () => {
    expect(money("123456.78", true)).toBe("••••••");
    expect(money("invalid")).toBe("¥0.00");
  });

  it("按中文格式显示金额和精度", () => {
    expect(money("1234.5")).toContain("1,234.50");
    expect(number("1.234567", 4)).toBe("1.2346");
  });

  it("将进度限制在 0 到 100", () => {
    expect(percent(-5)).toBe("0%");
    expect(percent(48)).toBe("48%");
    expect(percent(66.666)).toBe("67%");
    expect(percent(130)).toBe("100%");
  });

  it("无效日期保留原始值，空日期明确显示", () => {
    expect(localDate(null)).toBe("未设置");
    expect(localDate("not-a-date")).toBe("not-a-date");
  });

  it("使用十进制字符串相乘，避免浮点误差", () => {
    expect(multiplyDecimals("2500.125", "1.5821")).toBe("3955.4477625");
    expect(multiplyDecimals("0.1", "0.2")).toBe("0.02");
    expect(multiplyDecimals("invalid", "2")).toBe("0");
  });
});

describe("金融数据默认值", () => {
  it("用十进制字符串初始化交易字段", () => {
    const transaction = emptyTransaction();
    expect(transaction.quantity).toBe("0");
    expect(transaction.unitPrice).toBe("0");
    expect(transaction.amount).toBe("0");
    expect(transaction.fee).toBe("0");
    expect(transaction.tax).toBe("0");
  });
});
