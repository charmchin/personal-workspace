import { format, isValid, parseISO } from "date-fns";
import { zhCN } from "date-fns/locale";

export function localDate(value?: string | null, pattern = "M月d日 EEEE") {
  if (!value) return "未设置";
  const parsed = parseISO(value);
  return isValid(parsed) ? format(parsed, pattern, { locale: zhCN }) : value;
}

export function money(value: string | number, hidden = false) {
  if (hidden) return "••••••";
  const number = Number(value);
  if (!Number.isFinite(number)) return "¥0.00";
  return new Intl.NumberFormat("zh-CN", { style: "currency", currency: "CNY", maximumFractionDigits: 2 }).format(number);
}

export function number(value: string | number, digits = 2) {
  const parsed = Number(value);
  if (!Number.isFinite(parsed)) return "0";
  return new Intl.NumberFormat("zh-CN", { maximumFractionDigits: digits }).format(parsed);
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
