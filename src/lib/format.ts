import { shanghaiInstant } from "./calendar";

const displayDate = new Intl.DateTimeFormat("zh-CN", { timeZone: "Asia/Shanghai", year: "numeric", month: "2-digit", day: "2-digit", weekday: "long", hour: "2-digit", minute: "2-digit", second: "2-digit", hourCycle: "h23" });

export function localDate(value?: string | null, pattern = "M月d日 EEEE") {
  if (!value) return "未设置";
  const parsed = shanghaiInstant(value);
  if (!Number.isFinite(parsed.getTime())) return value;
  const parts = Object.fromEntries(displayDate.formatToParts(parsed).map((part) => [part.type, part.value]));
  const tokens: Record<string, string> = { yyyy: parts.year, MM: parts.month, M: String(Number(parts.month)), dd: parts.day, d: String(Number(parts.day)), HH: parts.hour, mm: parts.minute, ss: parts.second, EEEE: parts.weekday, EEE: parts.weekday.replace("星期", "周"), E: parts.weekday.replace("星期", "周") };
  return pattern.replace(/yyyy|EEEE|EEE|MM|dd|HH|mm|ss|M|d|E/g, (token) => tokens[token]);
}

export function money(value: string | number, hidden = false) {
  if (hidden) return "••••••";
  if (typeof value === "number") return Number.isFinite(value) ? new Intl.NumberFormat("zh-CN", { style: "currency", currency: "CNY", maximumFractionDigits: 2 }).format(value) : "未知";
  const result = decimalDisplay(value, 2, true);
  return result === null ? "未知" : `${result.startsWith("-") ? "-¥" : "¥"}${result.replace(/^-/, "")}`;
}

export function number(value: string | number, digits = 2) {
  if (typeof value === "number") return Number.isFinite(value) ? new Intl.NumberFormat("zh-CN", { maximumFractionDigits: digits }).format(value) : "未知";
  return decimalDisplay(value, digits, false) ?? "未知";
}

// Formatting is exact decimal arithmetic too; Number is reserved for charts.
function decimalDisplay(value: string | number, digits: number, fixed: boolean) {
  const match = String(value).trim().match(/^([+-]?)(\d+)(?:\.(\d*))?$/);
  if (!match || !Number.isInteger(digits) || digits < 0 || digits > 28) return null;
  const fraction = match[3] ?? "";
  let units = BigInt(match[2]) * 10n ** BigInt(digits) + BigInt(fraction.slice(0, digits).padEnd(digits, "0") || "0");
  if ((fraction[digits] ?? "0") >= "5") units += 1n;
  const padded = units.toString().padStart(digits + 1, "0");
  const integer = (digits ? padded.slice(0, -digits) : padded).replace(/\B(?=(\d{3})+(?!\d))/g, ",");
  let tail = digits ? padded.slice(-digits) : "";
  if (!fixed) tail = tail.replace(/0+$/, "");
  return `${match[1] === "-" && units !== 0n ? "-" : ""}${integer}${tail ? `.${tail}` : ""}`;
}

export function percent(value: number) {
  return `${Math.round(Math.max(0, Math.min(100, value)))}%`;
}

export function multiplyDecimals(left: string, right: string) {
  const parse = (value: string) => {
    const match = value.trim().match(/^([+-]?)(\d+)(?:\.(\d*))?$/);
    if (!match) return null;
    const fraction = match[3] ?? "";
    const digits = `${match[2]}${fraction}`.replace(/^0+(?=\d)/, "");
    return { value: BigInt(`${match[1] === "-" ? "-" : ""}${digits}`), scale: fraction.length };
  };
  const a = parse(left);
  const b = parse(right);
  if (!a || !b) return "0";
  const product = a.value * b.value;
  const scale = a.scale + b.scale;
  if (scale === 0) return product.toString();
  const negative = product < 0n;
  const digits = (negative ? -product : product).toString().padStart(scale + 1, "0");
  const integer = digits.slice(0, -scale);
  const fraction = digits.slice(-scale).replace(/0+$/, "");
  return `${negative ? "-" : ""}${integer}${fraction ? `.${fraction}` : ""}`;
}

export function errorMessage(error: unknown) {
  if (error instanceof Error) return error.message;
  return String(error);
}
