// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, expect, it, vi } from "vitest";
import { shanghaiToday } from "../lib/calendar";
import { defaultSettings, emptyAccount, emptyCalendar, emptyTask } from "../lib/defaults";
import { PortfolioPage } from "./PortfolioPage";
import { SchedulePage } from "./SchedulePage";
import { SettingsPage } from "./SettingsPage";
import { CommandPalette } from "../components/CommandPalette";
import { WorkbenchError } from "../lib/api";

const mocked = vi.hoisted(() => ({ call: vi.fn(), open: vi.fn(), save: vi.fn() }));
vi.mock("../lib/api", async (load) => ({ ...await load<typeof import("../lib/api")>(), call: mocked.call }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: mocked.open, save: mocked.save }));
vi.mock("@tauri-apps/api/app", () => ({ getVersion: () => Promise.resolve("0.1.4") }));
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
  Element.prototype.scrollIntoView = () => undefined;
  vi.stubGlobal("ResizeObserver", class { observe() {} unobserve() {} disconnect() {} });
});
afterEach(() => { cleanup(); vi.restoreAllMocks(); vi.resetAllMocks(); });
const snapshot = { totalMarketValue: "0", totalCost: "0", unrealizedGain: "0", realizedGain: "0", holdings: [], updatedAt: null, valuationComplete: true, missingPriceCount: 0 };

it("CSV 必须确认目标账户，而不是固定使用第一个账户", async () => {
  const accounts = ["a", "b"].map((id) => ({ ...emptyAccount(), id, name: `账户 ${id}` }));
  mocked.call.mockImplementation(async (command) => command === "get_portfolio_snapshot" ? snapshot : command === "list_investment_accounts" ? accounts : command === "import_portfolio_csv" ? { imported: 1, skipped: 0, updated: 0, errors: [] } : []);
  mocked.open.mockResolvedValue("/synthetic.csv");
  render(<PortfolioPage settings={defaultSettings} onSettingsChange={vi.fn()} />);
  await screen.findByText("账户 b");
  fireEvent.click(screen.getByRole("button", { name: "导入 CSV" }));
  expect(mocked.open).not.toHaveBeenCalled();
  fireEvent.change(screen.getByLabelText("导入到哪个账户"), { target: { value: "b" } });
  fireEvent.click(screen.getByRole("button", { name: "选择 CSV 文件并导入" }));
  await waitFor(() => expect(mocked.call).toHaveBeenCalledWith("import_portfolio_csv", { path: "/synthetic.csv", accountId: "b" }));
});

it("估值错误不妨碍加载账户和交易以便修正数据", async () => {
  mocked.call.mockImplementation(async (command) => {
    if (command === "get_portfolio_snapshot") throw new WorkbenchError({ code: "FINANCIAL_OVERFLOW", message: "金融计算超出范围" });
    if (command === "list_investment_accounts") return [{ ...emptyAccount(), id: "a", name: "仍能管理的账户" }];
    return [];
  });
  render(<PortfolioPage settings={defaultSettings} onSettingsChange={vi.fn()} />);
  expect(await screen.findByText("仍能管理的账户")).toBeTruthy();
  expect(screen.getByText("暂不可用")).toBeTruthy();
});

it("CSV 文件选择失败留在弹窗内且可再次选择", async () => {
  mocked.call.mockImplementation(async (command) => command === "get_portfolio_snapshot" ? snapshot : command === "list_investment_accounts" ? [{ ...emptyAccount(), id: "a", name: "测试账户" }] : []);
  mocked.open.mockRejectedValue(new Error("文件选择失败"));
  render(<PortfolioPage settings={defaultSettings} onSettingsChange={vi.fn()} />);
  await screen.findByText("测试账户");
  fireEvent.click(screen.getByRole("button", { name: "导入 CSV" }));
  fireEvent.click(screen.getByRole("button", { name: "选择 CSV 文件并导入" }));
  const alert = await within(screen.getByRole("dialog")).findByRole("alert");
  expect(alert.closest("dialog")).not.toBeNull();
  expect(alert.textContent).toContain("文件选择失败");
  expect(screen.getByRole("button", { name: "选择 CSV 文件并导入" }).hasAttribute("disabled")).toBe(false);
  expect(mocked.call.mock.calls.some(([command]) => command === "import_portfolio_csv")).toBe(false);
});

it("ICS 文件选择失败返回页面错误而非未处理拒绝", async () => {
  mocked.call.mockResolvedValue([]);
  mocked.open.mockRejectedValue(new WorkbenchError({ code: "FILE_DIALOG", message: "无法选择日程文件" }));
  render(<SchedulePage />);
  fireEvent.click(screen.getByRole("button", { name: "导入 ICS" }));
  expect(await screen.findByText("无法选择日程文件")).toBeTruthy();
  expect(mocked.call.mock.calls.some(([command]) => command === "import_ics")).toBe(false);
});

it("日视图展示全部事件，侧栏任务可展开而不是静默截断", async () => {
  const date = shanghaiToday();
  const items = Array.from({ length: 12 }, (_, i) => ({ ...emptyCalendar(), id: `e-${i}`, title: `完整日程 ${i}`, startAt: `${date}T09:00`, endAt: `${date}T10:00` }));
  const tasks = Array.from({ length: 18 }, (_, i) => ({ ...emptyTask(), id: `t-${i}`, title: `完整任务 ${i}`, dueDate: null }));
  mocked.call.mockImplementation(async (command) => command === "list_calendar_items" ? items : command === "list_tasks" ? tasks : []);
  render(<SchedulePage />);
  await screen.findByText("完整任务 0");
  fireEvent.click(screen.getByRole("button", { name: "日" }));
  expect(screen.getAllByRole("button", { name: /完整日程 \d/ })).toHaveLength(12);
  expect(screen.queryByText("完整任务 17")).toBeNull();
  fireEvent.click(screen.getByRole("button", { name: "显示更多任务" }));
  expect(screen.getByText("完整任务 17")).toBeTruthy();
});

it("任务删除须确认，取消不写数据库，确认只删除目标 ID", async () => {
  let tasks = ["a", "b"].map((id) => ({ ...emptyTask(), id, title: `可删除任务 ${id}`, dueDate: shanghaiToday() }));
  mocked.call.mockImplementation(async (command, args) => {
    if (command === "list_tasks") return tasks;
    if (command === "delete_record") { tasks = tasks.filter((task) => task.id !== args.id); return null; }
    return [];
  });
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(false);
  render(<SchedulePage />);
  const remove = await screen.findByRole("button", { name: "删除任务 可删除任务 a" });
  fireEvent.click(remove);
  expect(mocked.call.mock.calls.some(([command]) => command === "delete_record")).toBe(false);
  confirm.mockReturnValue(true);
  fireEvent.click(remove);
  await waitFor(() => expect(screen.queryByRole("button", { name: "删除任务 可删除任务 a" })).toBeNull());
  expect(mocked.call).toHaveBeenCalledWith("delete_record", { kind: "task", id: "a" });
  expect(screen.queryByText("□ 可删除任务 a")).toBeNull();
  expect(screen.getByRole("button", { name: "删除任务 可删除任务 b" })).toBeTruthy();
});

it("编辑任务删除失败保留草稿，执行期间不能重复提交", async () => {
  const task = { ...emptyTask(), id: "saved-task", title: "保留失败草稿", dueDate: null, recurrence: "daily" as const };
  let reject!: (error: unknown) => void;
  mocked.call.mockImplementation(async (command) => {
    if (command === "list_tasks") return [task];
    if (command === "delete_record") return new Promise((_, rejectPromise) => { reject = rejectPromise; });
    return [];
  });
  const confirm = vi.spyOn(window, "confirm").mockReturnValue(true);
  render(<SchedulePage />);
  fireEvent.click(await screen.findByRole("button", { name: /保留失败草稿 无截止日期/ }));
  const dialog = screen.getByRole("dialog");
  const remove = within(dialog).getByRole("button", { name: "删除任务" });
  fireEvent.change(within(dialog).getByLabelText("任务标题"), { target: { value: "未保存的标题" } });
  fireEvent.click(remove);
  fireEvent.click(remove);
  expect(remove.hasAttribute("disabled")).toBe(true);
  expect(mocked.call.mock.calls.filter(([command]) => command === "delete_record")).toHaveLength(1);
  expect(confirm).toHaveBeenCalledWith(expect.stringContaining("已经生成的其他任务记录不会被删除"));
  reject(new WorkbenchError({ code: "TEST_DELETE", message: "删除失败请重试" }));
  expect((await within(dialog).findByRole("alert")).textContent).toContain("删除失败请重试");
  expect(within(dialog).getByDisplayValue("未保存的标题")).toBeTruthy();
  expect(remove.hasAttribute("disabled")).toBe(false);
});

it("新建任务没有删除入口，保存过的已完成任务可通过编辑删除", async () => {
  const task = { ...emptyTask(), id: "done-task", title: "已完成任务", status: "done" as const };
  let deleted = false;
  mocked.call.mockImplementation(async (command) => {
    if (command === "list_tasks") return deleted ? [] : [task];
    if (command === "delete_record") { deleted = true; return null; }
    return [];
  });
  vi.spyOn(window, "confirm").mockReturnValue(true);
  const page = render(<SchedulePage />);
  fireEvent.click(screen.getByRole("button", { name: "任务" }));
  expect(within(screen.getByRole("dialog")).queryByRole("button", { name: "删除任务" })).toBeNull();
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "取消" }));
  page.rerender(<SchedulePage searchTarget={{ id: "done-task", kind: "task", title: "已完成任务", subtitle: "" }} />);
  const remove = await screen.findByRole("button", { name: "删除任务" });
  fireEvent.click(remove);
  await waitFor(() => expect(screen.queryByRole("dialog")).toBeNull());
  expect(mocked.call).toHaveBeenCalledWith("delete_record", { kind: "task", id: "done-task" });
});

it("设置保存期间禁用其他修改，失败后回到已保存值", async () => {
  let reject!: (error: unknown) => void;
  mocked.call.mockImplementation(async (command) => {
    if (command === "security_status") return { keychainMode: "passphrase" };
    if (command === "has_tushare_token") return false;
    if (command === "update_settings") return new Promise((_, rejectPromise) => { reject = rejectPromise; });
    return [];
  });
  render(<SettingsPage settings={defaultSettings} onSettingsChange={vi.fn()} onLock={vi.fn()} />);
  fireEvent.change(screen.getByDisplayValue("跟随系统"), { target: { value: "light" } });
  expect(screen.getByRole("switch", { name: "默认隐藏金额" }).hasAttribute("disabled")).toBe(true);
  reject(new WorkbenchError({ code: "TEST", message: "保存失败" }));
  await screen.findByText("保存失败");
  expect(screen.getByDisplayValue("跟随系统")).toBeTruthy();
});

it("晚返回的旧搜索不会覆盖新查询，结果选择保留记录 ID", async () => {
  const pending = new Map<string, (value: unknown) => void>();
  mocked.call.mockImplementation((_command, { query }) => new Promise((resolve) => pending.set(query, resolve)));
  const navigate = vi.fn();
  render(<CommandPalette open onClose={vi.fn()} navigate={navigate} quickAdd={vi.fn()} />);
  const input = screen.getByPlaceholderText("搜索任务、项目、内容或证券…");
  fireEvent.change(input, { target: { value: "old" } });
  await waitFor(() => expect(pending.has("old")).toBe(true));
  fireEvent.change(input, { target: { value: "new" } });
  await waitFor(() => expect(pending.has("new")).toBe(true));
  const latest = { id: "new-id", kind: "habit", title: "new 阅读", subtitle: "daily" };
  pending.get("new")!([latest]); await screen.findByText("new 阅读");
  pending.get("old")!([{ id: "old-id", kind: "task", title: "old 任务", subtitle: "old" }]);
  await waitFor(() => expect(screen.queryByText("old 任务")).toBeNull());
  fireEvent.click(screen.getByText("new 阅读"));
  expect(navigate).toHaveBeenCalledWith("growth", latest);
});
