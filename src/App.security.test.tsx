// @vitest-environment jsdom
import { act, cleanup, fireEvent, render, screen, waitFor } from "@testing-library/react";
import { afterEach, describe, expect, it, vi } from "vitest";
import type { SecurityStatus } from "./types";
import { defaultSettings } from "./lib/defaults";
import App from "./App";

const mocked = vi.hoisted(() => ({ call: vi.fn(), listen: vi.fn() }));
vi.mock("./lib/api", async (load) => ({ ...await load<typeof import("./lib/api")>(), call: mocked.call }));
vi.mock("@tauri-apps/api/event", () => ({ listen: mocked.listen }));
vi.mock("./pages/TodayPage", () => ({ TodayPage: () => <div data-testid="private-data">私密工作记录</div> }));

let notify: (event: { payload: unknown }) => void = () => {};
function setup(status: SecurityStatus) {
  mocked.listen.mockImplementation((_name, callback) => { notify = callback; return Promise.resolve(() => {}); });
  mocked.call.mockImplementation((command) => {
    if (command === "security_status") return Promise.resolve(status);
    if (command === "get_settings") return Promise.resolve(defaultSettings);
    return Promise.resolve(null);
  });
}
const base: SecurityStatus = { initialized: true, unlocked: true, databasePath: "/tmp/isolated.sqlite3", keychainMode: "passphrase", sessionEpoch: 1 };

afterEach(() => { cleanup(); vi.clearAllMocks(); });

describe("原生锁定后的页面清理", () => {
  it("连续 Enter 不会并发提交口令认证，失败后可以重试", async () => {
    setup({ ...base, unlocked: false, sessionEpoch: 0 });
    let reject!: (error: unknown) => void;
    const pending = new Promise<SecurityStatus>((_done, fail) => { reject = fail; });
    const initial = mocked.call.getMockImplementation()!;
    mocked.call.mockImplementation((command, args) => command === "security_unlock_with_password" ? pending : initial(command, args));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "解锁工作台" }));
    const password = screen.getByLabelText("工作台口令");
    fireEvent.change(password, { target: { value: "123456" } });
    fireEvent.keyDown(password, { key: "Enter" });
    fireEvent.keyDown(password, { key: "Enter" });
    expect(mocked.call.mock.calls.filter(([command]) => command === "security_unlock_with_password")).toHaveLength(1);
    await act(async () => reject({ code: "TEST", message: "认证失败" }));
    expect(screen.getByText("认证失败")).toBeTruthy();
    fireEvent.keyDown(password, { key: "Enter" });
    await waitFor(() => expect(mocked.call.mock.calls.filter(([command]) => command === "security_unlock_with_password")).toHaveLength(2));
  });

  it("立即卸载私密模块，并拒绝随后返回的旧状态", async () => {
    setup(base);
    render(<App />);
    await screen.findByTestId("private-data");
    act(() => notify({ payload: { sessionEpoch: 2, unlocked: false, lockReason: "system" } }));
    expect(screen.queryByTestId("private-data")).toBeNull();
    expect(screen.getByText("系统锁屏、休眠或会话切换后，工作台需要重新解锁。")).toBeTruthy();
    fireEvent.focus(window);
    await waitFor(() => expect(mocked.call).toHaveBeenCalledWith("security_status"));
    expect(screen.queryByTestId("private-data")).toBeNull();
  });

  it("解锁等待期间收到锁屏通知，不接受旧认证结果且清空口令", async () => {
    setup({ ...base, unlocked: false, sessionEpoch: 0 });
    let resolve!: (status: SecurityStatus) => void;
    const pending = new Promise<SecurityStatus>((done) => { resolve = done; });
    const initial = mocked.call.getMockImplementation()!;
    mocked.call.mockImplementation((command, args) => command === "security_unlock_with_password" ? pending : initial(command, args));
    render(<App />);
    fireEvent.click(await screen.findByRole("button", { name: "解锁工作台" }));
    const password = document.querySelector<HTMLInputElement>('input[type="password"]')!;
    fireEvent.change(password, { target: { value: "123456" } });
    fireEvent.click(screen.getByRole("button", { name: "解锁工作台" }));
    await waitFor(() => expect(mocked.call).toHaveBeenCalledWith("security_unlock_with_password", { password: "123456" }));
    act(() => notify({ payload: { sessionEpoch: 2, unlocked: false, lockReason: "system" } }));
    expect(document.querySelector('input[type="password"]')).toBeNull();
    await act(async () => resolve(base));
    expect(screen.queryByTestId("private-data")).toBeNull();
    expect(mocked.call).not.toHaveBeenCalledWith("get_settings");
  });
});
