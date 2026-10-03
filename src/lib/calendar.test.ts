import { expect, it } from "vitest";
import { emptyCalendar } from "./defaults";
import { calendarOccursOn, shanghaiInput } from "./calendar";

it("UTC 午夜附近按上海日期显示，并保留旧的本地时间输入", () => {
  expect(shanghaiInput("2026-08-29T16:30:00Z")).toBe("2026-08-30T00:30");
  expect(shanghaiInput("2026-08-30T09:30:00+08:00")).toBe("2026-08-30T09:30");
  expect(shanghaiInput("2026-08-30T09:30")).toBe("2026-08-30T09:30");
});

it("跨午夜日程在两天均可见，结束边界不包含下一天", () => {
  const item = { ...emptyCalendar(), startAt: "2026-08-29T15:00:00Z", endAt: "2026-08-29T17:00:00Z" };
  expect(calendarOccursOn(item, new Date(2026, 7, 29))).toBe(true);
  expect(calendarOccursOn(item, new Date(2026, 7, 30))).toBe(true);
  expect(calendarOccursOn(item, new Date(2026, 7, 31))).toBe(false);
  expect(calendarOccursOn({ ...item, allDay: true, startAt: "2026-08-29T00:00:00+08:00", endAt: "2026-08-30T00:00:00+08:00" }, new Date(2026, 7, 30))).toBe(false);
});

it("每周跨日重复、月底跳月和首次开始时间正确", () => {
  const weekly = { ...emptyCalendar(), startAt: "2026-08-03T23:00", endAt: "2026-08-04T01:00", recurrence: "weekly" as const };
  expect(calendarOccursOn(weekly, new Date(2026, 7, 11))).toBe(true);
  expect(calendarOccursOn(weekly, new Date(2026, 7, 12))).toBe(false);
  expect(calendarOccursOn(weekly, new Date(2026, 6, 28))).toBe(false);
  const monthly = { ...weekly, startAt: "2026-01-31T10:00", endAt: "2026-01-31T11:00", recurrence: "monthly" as const };
  expect(calendarOccursOn(monthly, new Date(2026, 1, 28))).toBe(false);
  expect(calendarOccursOn(monthly, new Date(2026, 2, 31))).toBe(true);
});
