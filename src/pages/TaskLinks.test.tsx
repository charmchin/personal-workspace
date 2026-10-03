// @vitest-environment jsdom
import { cleanup, fireEvent, render, screen, waitFor, within } from "@testing-library/react";
import { afterEach, beforeAll, expect, it, vi } from "vitest";
import { TodayPage } from "./TodayPage";
import { WorkPage } from "./WorkPage";
import { SchedulePage } from "./SchedulePage";
import { RelatedTasks } from "../components/RelatedTasks";
import { defaultSettings, emptyContent, emptyGoal, emptyTask, emptyWorkLog } from "../lib/defaults";
import { shanghaiToday } from "../lib/calendar";
import type { Dashboard, Task } from "../types";

const mocked = vi.hoisted(() => ({ call: vi.fn() }));
vi.mock("../lib/api", async (load) => ({ ...await load<typeof import("../lib/api")>(), call: mocked.call }));
vi.mock("@tauri-apps/plugin-dialog", () => ({ open: vi.fn(), save: vi.fn() }));
beforeAll(() => {
  HTMLDialogElement.prototype.showModal = function () { this.setAttribute("open", ""); };
  HTMLDialogElement.prototype.close = function () { this.removeAttribute("open"); };
});
afterEach(() => { cleanup(); vi.resetAllMocks(); });
function dashboard(tasks: Task[]): Dashboard {
  return { date: "2026-10-03", tasks, pendingTaskCount: 10, completedTaskCount: 7, nextEvent: null, habits: [], workLogs: [], contentItems: [], settings: defaultSettings, portfolio: { totalMarketValue: "0", totalCost: "0", unrealizedGain: "0", realizedGain: "0", holdings: [], updatedAt: null, valuationComplete: true, missingPriceCount: 0 } };
}
function today(data: Dashboard, navigate = vi.fn()) {
  mocked.call.mockImplementation(async (command) => command === "get_dashboard" ? data : []);
  render(<TodayPage quickOpen={false} closeQuick={vi.fn()} navigate={navigate} settings={defaultSettings} onSettingsChange={vi.fn()} />);
  return navigate;
}
it("首页显示五项待办和三项完成，统计采用完整数量，完成有独立删除线类", async () => {
  const tasks = Array.from({ length: 8 }, (_, i) => ({ ...emptyTask(), id: `t${i}`, title: `行动 ${i}`, status: i < 5 ? "todo" as const : "done" as const }));
  today(dashboard(tasks));
  await screen.findByText("待办 10 项 · 今日完成 7 项");
  expect(screen.getAllByRole("checkbox")).toHaveLength(8);
  expect(screen.getByText("行动 7").closest(".task-row")?.classList.contains("is-complete")).toBe(true);
  expect(screen.getByText("还有 5 项待办，查看全部 →")).toBeTruthy();
  expect(screen.getByText("还有 4 项已完成，查看日程 →")).toBeTruthy();
  expect(screen.getAllByLabelText("正常优先级")).toHaveLength(8);
});
it("首页标题只打开编辑，复选框才完成，失败不提前改变任务状态", async () => {
  const task = { ...emptyTask(), id: "target", title: "标题点击不能完成" };
  const data = dashboard([task]);
  today(data);
  fireEvent.click(await screen.findByRole("button", { name: `编辑任务 ${task.title}` }));
  await within(screen.getByRole("dialog")).findByLabelText("关联成长目标");
  expect(mocked.call.mock.calls.some(([command]) => command === "toggle_task")).toBe(false);
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "取消" }));
  let resolve!: (value: unknown) => void;
  mocked.call.mockImplementation(async (command) => {
    if (command === "get_dashboard") return data;
    if (command === "toggle_task") return new Promise((r) => { resolve = r; });
    return [];
  });
  const checkbox = screen.getByRole("checkbox", { name: `完成任务 ${task.title}` });
  fireEvent.click(checkbox);
  expect(checkbox.hasAttribute("disabled")).toBe(true);
  expect(mocked.call).toHaveBeenCalledWith("toggle_task", { id: "target", completed: true });
  data.tasks = [{ ...task, status: "done" }];
  resolve(null);
  await screen.findByRole("checkbox", { name: `取消完成 ${task.title}` });
  expect(screen.getByText(task.title).closest(".task-row")?.classList.contains("is-complete")).toBe(true);
});
it("首页工作记录打开准确记录，不能误完成任务", async () => {
  const log = { ...emptyWorkLog(), id: "log-target", title: "合成工作日志" };
  const data = dashboard([]); data.workLogs = [log];
  const navigate = today(data);
  fireEvent.click(await screen.findByText(log.title));
  expect(navigate).toHaveBeenCalledWith("work", expect.objectContaining({ id: log.id, kind: "worklog" }));
  expect(mocked.call.mock.calls.some(([command]) => command === "upsert_task")).toBe(false);
});
it("工作记录创建任务必须保存确认，原日志关联准确，重复打开编辑原任务", async () => {
  let log = { ...emptyWorkLog(), id: "source-log", title: "原始记录", nextSteps: "完成备稿" };
  let tasks: Task[] = [];
  mocked.call.mockImplementation(async (command, args) => {
    if (command === "list_work_logs") return [log];
    if (command === "list_tasks") return tasks;
    if (command === "create_task_from_work_log") {
      const task = { ...args.task, id: "new-linked-task" }; tasks = [task]; log = { ...log, taskId: task.id }; return task;
    }
    return [];
  });
  render(<WorkPage />);
  fireEvent.click(await screen.findByRole("button", { name: "创建待办任务" }));
  expect(screen.getByDisplayValue("完成备稿")).toBeTruthy();
  fireEvent.click(within(screen.getByRole("dialog")).getByRole("button", { name: "取消" }));
  expect(mocked.call.mock.calls.some(([command]) => command === "create_task_from_work_log")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "创建待办任务" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "保存任务" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "保存任务" }));
  await screen.findByRole("button", { name: "查看关联任务" });
  expect(mocked.call).toHaveBeenCalledWith("create_task_from_work_log", { logId: "source-log", task: expect.objectContaining({ id: "", title: "完成备稿" }) });
  expect(mocked.call.mock.calls.some(([command]) => command === "upsert_work_log")).toBe(false);
  fireEvent.click(screen.getByRole("button", { name: "查看关联任务" }));
  await screen.findByRole("heading", { name: "编辑任务" });
  expect(mocked.call.mock.calls.filter(([command]) => command === "create_task_from_work_log")).toHaveLength(1);
});
it.each(["goal", "content"] as const)("%s 创建关联任务仅写任务，不篡改来源状态", async (kind) => {
  let tasks: Task[] = [];
  mocked.call.mockImplementation(async (command, args) => {
    if (command === "list_tasks") return tasks;
    if (command === "list_goals") return [{ ...emptyGoal(), id: "source", title: "来源" }];
    if (command === "list_content_items") return [{ ...emptyContent(), id: "source", title: "来源" }];
    if (command === "upsert_task") { tasks = [{ ...args.task, id: "linked" }]; return tasks[0]; }
    return [];
  });
  render(<RelatedTasks kind={kind} id="source" title="来源" />);
  fireEvent.click(screen.getByRole("button", { name: "创建关联任务" }));
  await waitFor(() => expect(screen.getByRole("button", { name: "保存任务" }).hasAttribute("disabled")).toBe(false));
  fireEvent.click(screen.getByRole("button", { name: "保存任务" }));
  await screen.findByText("已完成 0 / 1", { exact: false });
  expect(mocked.call).toHaveBeenCalledWith("upsert_task", { task: expect.objectContaining({ goalId: kind === "goal" ? "source" : null, contentId: kind === "content" ? "source" : null }) });
  expect(mocked.call.mock.calls.some(([command]) => command === "upsert_goal" || command === "upsert_content_item")).toBe(false);
});
it("日程可查看已完成任务，并从列表取消完成或删除", async () => {
  const task = { ...emptyTask(), id: "done", title: "完成后的任务", status: "done" as const };
  mocked.call.mockImplementation(async (command) => command === "list_tasks" ? [task] : []);
  render(<SchedulePage />);
  await screen.findByText("0 项待完成");
  fireEvent.click(screen.getByRole("button", { name: "已完成" }));
  expect(await screen.findByRole("button", { name: `取消完成 ${task.title}` })).toBeTruthy();
  expect(screen.getByRole("button", { name: `删除任务 ${task.title}` })).toBeTruthy();
  expect(screen.getByText(task.title).closest(".panel-task")?.classList.contains("is-complete")).toBe(true);
});
it("计划开始日可在日历呈现，即使没有截止日期", async () => {
  const task = { ...emptyTask(), id: "planned", title: "计划时间任务", dueDate: null, scheduledStart: `${shanghaiToday()}T01:00:00Z` };
  mocked.call.mockImplementation(async (command) => command === "list_tasks" ? [task] : []);
  render(<SchedulePage />);
  await screen.findByText(task.title);
  expect(screen.getByText(`□ ${task.title}`)).toBeTruthy();
});

it("编辑保留目标和内容关联，重复任务不能直接保存为完成", async () => {
  const task = { ...emptyTask(), id: "repeat", title: "重复行动", goalId: "goal", contentId: "content" };
  mocked.call.mockImplementation(async (command, args) => {
    if (command === "list_tasks") return [task];
    if (command === "list_goals") return [{ ...emptyGoal(), id: "goal", title: "成长目标" }];
    if (command === "list_content_items") return [{ ...emptyContent(), id: "content", title: "自媒体内容" }];
    if (command === "upsert_task") return args.task;
    return [];
  });
  render(<SchedulePage searchTarget={{ kind: "task", id: task.id, title: task.title, subtitle: "" }} />);
  const dialog = await screen.findByRole("dialog");
  await waitFor(() => expect(within(dialog).getByRole("button", { name: "保存任务" }).hasAttribute("disabled")).toBe(false));
  expect((within(dialog).getByLabelText("关联成长目标") as HTMLSelectElement).value).toBe("goal");
  expect((within(dialog).getByLabelText("关联自媒体内容") as HTMLSelectElement).value).toBe("content");
  fireEvent.change(within(dialog).getByLabelText("状态"), { target: { value: "done" } });
  fireEvent.change(within(dialog).getByLabelText("重复"), { target: { value: "daily" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "保存任务" }));
  expect((await within(dialog).findByRole("alert")).textContent).toContain("重复任务请从任务清单勾选完成");
  expect(mocked.call.mock.calls.some(([command]) => command === "upsert_task")).toBe(false);
  fireEvent.change(within(dialog).getByLabelText("状态"), { target: { value: "todo" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "保存任务" }));
  await waitFor(() => expect(mocked.call).toHaveBeenCalledWith("upsert_task", { task: expect.objectContaining({ goalId: "goal", contentId: "content", status: "todo", recurrence: "daily" }) }));
});

it("快速捕获工作不虚增工时，也不自动创建任务", async () => {
  const data = dashboard([]);
  mocked.call.mockImplementation(async (command) => command === "get_dashboard" ? data : []);
  render(<TodayPage quickOpen closeQuick={vi.fn()} navigate={vi.fn()} settings={defaultSettings} onSettingsChange={vi.fn()} />);
  fireEvent.click(await screen.findByRole("button", { name: /工作/ }));
  const dialog = screen.getByRole("dialog");
  fireEvent.change(within(dialog).getByLabelText("标题"), { target: { value: "备稿" } });
  fireEvent.click(within(dialog).getByRole("button", { name: "保存记录" }));
  await waitFor(() => expect(mocked.call).toHaveBeenCalledWith("upsert_work_log", { log: expect.objectContaining({ title: "备稿", minutes: 0, taskId: null }) }));
  expect(mocked.call.mock.calls.some(([command]) => command === "upsert_task")).toBe(false);
});
