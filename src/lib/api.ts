import { invoke } from "@tauri-apps/api/core";
import type { CommandError } from "../types";
import { ZodError } from "zod";
import { validateCommandResponse } from "./schemas";

export class WorkbenchError extends Error {
  code: string;
  recovery?: string | null;

  constructor(error: CommandError | string | unknown) {
    const normalized = normalizeError(error);
    super(normalized.message);
    this.name = "WorkbenchError";
    this.code = normalized.code;
    this.recovery = normalized.recovery;
  }
}

function normalizeError(error: unknown): CommandError {
  if (typeof error === "string") {
    try {
      const parsed = JSON.parse(error) as Partial<CommandError>;
      if (parsed.message) return { code: parsed.code ?? "UNKNOWN", message: parsed.message, recovery: parsed.recovery };
    } catch {
      return { code: "UNKNOWN", message: error };
    }
  }
  if (typeof error === "object" && error && "message" in error) {
    const value = error as Partial<CommandError>;
    return { code: value.code ?? "UNKNOWN", message: String(value.message), recovery: value.recovery };
  }
  return { code: "UNKNOWN", message: "发生未知错误" };
}

export async function call<T>(command: string, args?: Record<string, unknown>): Promise<T> {
  try {
    const response = await invoke<unknown>(command, args);
    try {
      return validateCommandResponse(command, response) as T;
    } catch (error) {
      if (error instanceof ZodError) {
        throw new WorkbenchError({
          code: "INVALID_BACKEND_RESPONSE",
          message: `本地后端返回的 ${command} 数据格式无效`,
          recovery: "请锁定并重新打开工作台；如果持续出现，请先保留数据目录再排查版本一致性。",
        });
      }
      throw error;
    }
  } catch (error) {
    throw new WorkbenchError(error);
  }
}
