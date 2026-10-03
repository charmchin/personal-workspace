import type { CalendarItem } from "../types";

export const shanghaiToday = () => shanghaiInput(new Date().toISOString()).slice(0, 10);
// A date-only cursor for date-fns calendar arithmetic, not a stored instant.
export const shanghaiCivilDay = () => new Date(`${shanghaiToday()}T12:00:00`);

export function shanghaiInput(value?: string | null) {
  if (!value) return "";
  if (!/(?:Z|[+-]\d{2}:\d{2})$/.test(value)) return value.slice(0, 16);
  const instant = new Date(value);
  if (!Number.isFinite(instant.getTime())) return "";
  return new Date(instant.getTime() + 8 * 3600_000).toISOString().slice(0, 16);
}

export function shanghaiInstant(value: string) {
  return new Date(/(?:Z|[+-]\d{2}:\d{2})$/.test(value) ? value : `${value.length === 10 ? `${value}T00:00:00` : value}+08:00`);
}

export function calendarOccursOn(item: CalendarItem, day: Date) {
  const civil = `${day.getFullYear()}-${String(day.getMonth() + 1).padStart(2, "0")}-${String(day.getDate()).padStart(2, "0")}`;
  const dayStart = shanghaiInstant(civil).getTime();
  const dayEnd = dayStart + 86400_000;
  const original = shanghaiInstant(item.startAt).getTime();
  const end = shanghaiInstant(item.endAt).getTime();
  if (!Number.isFinite(original) || !Number.isFinite(end)) return false;
  const duration = Math.max(end - original, 1);
  if (item.recurrence === "none") return original < dayEnd && original + duration > dayStart;
  const anchor = new Date(original + 8 * 3600_000);
  const target = new Date(dayStart + 8 * 3600_000);
  target.setUTCHours(anchor.getUTCHours(), anchor.getUTCMinutes(), anchor.getUTCSeconds(), 0);
  if (item.recurrence === "weekly") {
    target.setUTCDate(target.getUTCDate() - (target.getUTCDay() - anchor.getUTCDay() + 7) % 7);
  } else if (item.recurrence === "monthly") {
    const year = target.getUTCFullYear(); const month = target.getUTCMonth();
    for (let previous = 0; previous < 13; previous++) {
      const first = new Date(Date.UTC(year, month - previous, 1));
      const candidate = new Date(Date.UTC(first.getUTCFullYear(), first.getUTCMonth(), anchor.getUTCDate(), anchor.getUTCHours(), anchor.getUTCMinutes(), anchor.getUTCSeconds()));
      if (candidate.getUTCMonth() === first.getUTCMonth() && candidate.getTime() <= target.getTime()) { target.setTime(candidate.getTime()); break; }
    }
  }
  const occurrence = target.getTime() - 8 * 3600_000;
  return occurrence >= original && occurrence < dayEnd && occurrence + duration > dayStart;
}
