// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, expect, it, vi } from "vitest";
import { defaultSettings } from "../lib/defaults";
import { SettingsPage } from "./SettingsPage";
import type { BackupInfo } from "../types";

const mocked = vi.hoisted(() => ({ call: vi.fn(), getVersion: vi.fn() }));
vi.mock("../lib/api", async (load) => ({ ...await load<typeof import("../lib/api")>(), call: mocked.call }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: mocked.getVersion }));

afterEach(() => { cleanup(); vi.clearAllMocks(); });

function setup(fail = false, version = "0.1.4", backups: BackupInfo[] = []) {
  mocked.getVersion.mockResolvedValue(version);
  mocked.call.mockImplementation((command) => {
    if (command === "security_status") return Promise.resolve({ keychainMode: "passphrase" });
    if (command === "list_backups") return Promise.resolve(backups);
    if (command === "has_tushare_token") return Promise.resolve(false);
    if (command === "open_license_notices" && fail) return Promise.reject({ code: "LICENSE_OPEN_FAILED", message: "无法打开许可声明" });
    return Promise.resolve(null);
  });
  render(<SettingsPage settings={defaultSettings} onSettingsChange={vi.fn()} onLock={vi.fn()} />);
}

it("shows the runtime app version rather than a hard-coded source version", async () => {
  setup(false, "9.8.7");
  expect(await screen.findByText("应用版本：9.8.7")).toBeTruthy();
  expect(mocked.getVersion).toHaveBeenCalledTimes(1);
});

it("keeps the full recovery filename and accessible restore/delete actions for long snapshot names", async () => {
  const name = "pre-password-change-20261003T000000.000Z-long-recovery-point.sqlite3";
  setup(false, "0.1.4", [{ name, kind: "recovery", createdAt: "2026-10-03T00:00:00Z", sizeBytes: 200000 }]);
  expect((await screen.findByText(name)).getAttribute("title")).toBe(name);
  expect(screen.getByRole("button", { name: `恢复快照 ${name}` })).toBeTruthy();
  expect(screen.getByRole("button", { name: `删除快照 ${name}` })).toBeTruthy();
});

it("opens only the fixed bundled notice command, with no path or network arguments", async () => {
  setup();
  fireEvent.click(screen.getByRole("button", { name: "查看许可" }));
  await waitFor(() => expect(mocked.call).toHaveBeenCalledWith("open_license_notices"));
  expect(mocked.call.mock.calls.find(([command]) => command === "open_license_notices")).toHaveLength(1);
});

it("shows a recoverable error when the bundled notice cannot be opened", async () => {
  setup(true);
  fireEvent.click(screen.getByRole("button", { name: "查看许可" }));
  expect(await screen.findByText("无法打开许可声明")).toBeTruthy();
});
