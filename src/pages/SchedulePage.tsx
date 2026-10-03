import { useCallback, useEffect, useMemo, useState } from "react";
import { addDays, addMonths, eachDayOfInterval, endOfMonth, endOfWeek, format, isSameDay, isSameMonth, parseISO, startOfMonth, startOfWeek, subMonths } from "date-fns";
import { zhCN } from "date-fns/locale";
import { open, save } from "@tauri-apps/plugin-dialog";
import { CalendarPlus, ChevronLeft, ChevronRight, CircleCheck, Import as ImportIcon, FileOutput as ExportIcon, Plus, Trash2 } from "lucide-react";
import type { CalendarItem, ImportReport, Project, Task, SearchResult } from "../types";
import { call, WorkbenchError } from "../lib/api";
import { useSearchTarget } from "../lib/useSearchTarget";
import { TaskEditor, TaskPriority } from "../components/TaskEditor";
import { useMutation } from "../lib/useMutation";
import { emptyCalendar, emptyTask } from "../lib/defaults";
import { calendarOccursOn, shanghaiInput, shanghaiCivilDay, shanghaiToday } from "../lib/calendar";
import { localDate } from "../lib/format";
import { Button, Card, EmptyState, ErrorBanner, Field, Input, Modal, SectionHeader, Select, Textarea } from "../components/ui";

type ViewMode = "day" | "week" | "month";

function recursOn(startValue: string, recurrence: "none" | "daily" | "weekly" | "monthly", day: Date) {
  const start = parseISO(startValue);
  const startDay = new Date(start.getFullYear(), start.getMonth(), start.getDate());
  const targetDay = new Date(day.getFullYear(), day.getMonth(), day.getDate());
  if (targetDay < startDay) return false;
  if (recurrence === "none") return isSameDay(start, day);
  if (recurrence === "daily") return true;
  if (recurrence === "weekly") return start.getDay() === day.getDay();
  return start.getDate() === day.getDate();
}

export function SchedulePage({ searchTarget }: { searchTarget?: SearchResult | null }) {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [items, setItems] = useState<CalendarItem[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [cursor, setCursor] = useState(shanghaiCivilDay);
  const [view, setView] = useState<ViewMode>("month");
  const [taskDraft, setTaskDraft] = useState<Task | null>(null);
  const [eventDraft, setEventDraft] = useState<CalendarItem | null>(null);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const { busy, mutate } = useMutation(setError);
  const [taskLimit, setTaskLimit] = useState(14);
  const [taskFilter, setTaskFilter] = useState<"pending" | "done" | "all">("pending");
  const visibleTasks = tasks.filter((task) => taskFilter === "all" || (taskFilter === "done" ? task.status === "done" : task.status !== "done"));
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [taskData, calendarData, projectData] = await Promise.all([
        call<Task[]>("list_tasks"), call<CalendarItem[]>("list_calendar_items", { start: null, end: null }), call<Project[]>("list_projects"),
      ]);
      setTasks(taskData); setItems(calendarData); setProjects(projectData);
    } catch (value) { setError(value as WorkbenchError); }
  }, []);
  useEffect(() => { void load(); }, [load]);

  const calendarDays = useMemo(() => {
    if (view === "day") return [cursor];
    if (view === "week") return eachDayOfInterval({ start: startOfWeek(cursor, { weekStartsOn: 1 }), end: endOfWeek(cursor, { weekStartsOn: 1 }) });
    const monthStart = startOfMonth(cursor);
    return eachDayOfInterval({ start: startOfWeek(monthStart, { weekStartsOn: 1 }), end: endOfWeek(endOfMonth(monthStart), { weekStartsOn: 1 }) });
  }, [cursor, view]);

  useSearchTarget(searchTarget, "task", tasks, setTaskDraft);
  useSearchTarget(searchTarget, "calendar", items, setEventDraft);
  useEffect(() => {
    if (searchTarget?.kind === "task-list" && searchTarget.id === "done") { setTaskFilter("done"); setTaskLimit(14); }
  }, [searchTarget]);
  const step = (direction: number) => {
    if (view === "month") setCursor((current) => direction > 0 ? addMonths(current, 1) : subMonths(current, 1));
    else setCursor((current) => addDays(current, direction * (view === "week" ? 7 : 1)));
  };

  async function deleteTask(task: Task) {
    if (busy || !task.id) return;
    const recurrenceNotice = task.recurrence === "none" ? "" : "删除此重复任务后，它将不再出现在未来日程中；已经生成的其他任务记录不会被删除。";
    if (!window.confirm(`确认删除任务“${task.title}”？删除后无法恢复。${recurrenceNotice}`)) return;
    await mutate(async () => {
      await call("delete_record", { kind: "task", id: task.id });
      setTasks((current) => current.filter((item) => item.id !== task.id));
      setTaskDraft((current) => current?.id === task.id ? null : current);
      await load();
    });
  }
  async function saveEvent() {
    if (!eventDraft) return;
    await mutate(async () => { await call("upsert_calendar_item", { item: eventDraft }); setEventDraft(null); await load(); });
  }
  async function toggleTask(task: Task) {
    await mutate(async () => { await call("toggle_task", { id: task.id, completed: task.status !== "done" }); await load(); });
  }
  async function importIcs() {
    try {
      const path = await open({ multiple: false, filters: [{ name: "iCalendar", extensions: ["ics"] }] });
      if (!path) return;
      const report = await call<ImportReport>("import_ics", { path, source: `ics:${path}` }); setNotice(`已导入 ${report.imported} 项，更新 ${report.updated} 项，跳过 ${report.skipped} 项${report.errors[0] ? ` · ${report.errors[0]}` : ""}`); await load();
    }
    catch (value) { setError(value as WorkbenchError); }
  }
  async function exportIcs() {
    try {
      const path = await save({ defaultPath: `个人工作台日程-${shanghaiToday()}.ics`, filters: [{ name: "iCalendar", extensions: ["ics"] }] });
      if (!path) return;
      const count = await call<number>("export_ics", { path }); setNotice(`已导出 ${count} 项日程`);
    }
    catch (value) { setError(value as WorkbenchError); }
  }

  return <div className="page schedule-page">
    {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
    {notice && <div className="notice-banner"><CircleCheck size={17} />{notice}<button onClick={() => setNotice(null)}>×</button></div>}
    <div className="page-heading"><div><span className="eyebrow">PLAN WITH INTENTION</span><h1>日程计划</h1><p>把任务和时间放在同一张地图上，给真正重要的事留出空间。</p></div><div className="heading-actions"><Button variant="secondary" onClick={() => void importIcs()} title="从 .ics 文件读入日程"><ImportIcon size={16} />导入 ICS</Button><Button variant="secondary" onClick={() => void exportIcs()} title="将工作台日程写入 .ics 文件"><ExportIcon size={16} />导出 ICS</Button><Button onClick={() => setEventDraft(emptyCalendar())}><CalendarPlus size={16} />新建日程</Button></div></div>
    <div className="schedule-toolbar">
      <div className="date-navigation"><Button variant="ghost" size="sm" onClick={() => step(-1)}><ChevronLeft size={17} /></Button><button className="month-label" onClick={() => setCursor(shanghaiCivilDay())}>{view === "month" ? format(cursor, "yyyy年 M月", { locale: zhCN }) : format(cursor, "M月d日", { locale: zhCN })}</button><Button variant="ghost" size="sm" onClick={() => step(1)}><ChevronRight size={17} /></Button></div>
      <div className="segmented">{(["day", "week", "month"] as ViewMode[]).map((mode) => <button className={view === mode ? "active" : ""} key={mode} onClick={() => setView(mode)}>{mode === "day" ? "日" : mode === "week" ? "周" : "月"}</button>)}</div>
    </div>
    <div className="schedule-layout">
      <Card className="calendar-card">
        {view !== "day" && <div className={`calendar-weekdays ${view === "week" ? "week" : ""}`}>{["一", "二", "三", "四", "五", "六", "日"].map((day) => <span key={day}>周{day}</span>)}</div>}
        <div className={`calendar-grid calendar-view-${view}`}>
          {calendarDays.map((day) => {
            const dayItems = items.filter((item) => calendarOccursOn(item, day)).sort((a, b) => Number(b.allDay) - Number(a.allDay) || shanghaiInput(a.startAt).slice(11).localeCompare(shanghaiInput(b.startAt).slice(11)));
            const dayTasks = tasks.filter((task) => task.status !== "done" && (
              (task.dueDate && recursOn(task.dueDate, task.recurrence, day)) ||
              (task.scheduledStart && recursOn(shanghaiInput(task.scheduledStart).slice(0, 10), task.recurrence, day))
            ));
            const visibleItems = view === "month" ? dayItems.slice(0, 3) : dayItems;
            const visibleTasks = view === "month" ? dayTasks.slice(0, 2) : dayTasks;
            const hiddenCount = dayItems.length + dayTasks.length - visibleItems.length - visibleTasks.length;
            return <div key={day.toISOString()} className={`calendar-day ${!isSameMonth(day, cursor) && view === "month" ? "outside" : ""} ${isSameDay(day, shanghaiCivilDay()) ? "today" : ""}`} onDoubleClick={() => setEventDraft({ ...emptyCalendar(), startAt: `${format(day, "yyyy-MM-dd")}T09:00`, endAt: `${format(day, "yyyy-MM-dd")}T10:00` })}><button className="day-number" aria-label={`查看 ${format(day, "yyyy-MM-dd")} 的完整日程`} onClick={() => { setCursor(day); setView("day"); }}>{format(day, "d")}</button><div className="day-items">{visibleItems.map((item) => <button key={item.id} title={item.title} className={`day-event ${item.kind}`} onClick={(event) => { event.stopPropagation(); setEventDraft(item); }}>{item.allDay ? "全天 " : localDate(item.startAt, "HH:mm ")}{item.title}</button>)}{visibleTasks.map((task) => <button key={task.id} className="day-task" onClick={(event) => { event.stopPropagation(); setTaskDraft(task); }}>□ {task.title}</button>)}{hiddenCount > 0 && <button className="text-link" onClick={(event) => { event.stopPropagation(); setCursor(day); setView("day"); }}>查看其余 {hiddenCount} 项 →</button>}</div></div>;
          })}
        </div>
      </Card>
      <Card className="task-panel">
        <SectionHeader title="任务清单" description={`${tasks.filter((item) => item.status !== "done").length} 项待完成`} action={<Button size="sm" onClick={() => setTaskDraft(emptyTask())}><Plus size={15} />任务</Button>} />
        <div className="segmented task-status-filter" aria-label="筛选任务状态">{(["pending", "done", "all"] as const).map((filter) => <button key={filter} className={taskFilter === filter ? "active" : ""} onClick={() => { setTaskFilter(filter); setTaskLimit(14); }}>{filter === "pending" ? "待办" : filter === "done" ? "已完成" : "全部"}</button>)}</div>
        <div className="task-panel-list">{visibleTasks.slice(0, taskLimit).map((task) => <div key={task.id} className={`panel-task ${task.status === "done" ? "is-complete" : ""}`}><button className={`round-check ${task.status === "done" ? "checked" : ""}`} disabled={busy} onClick={() => void toggleTask(task)} aria-label={`${task.status === "done" ? "取消完成" : "完成任务"} ${task.title}`}>{task.status === "done" && <CircleCheck size={16} />}</button><button className="task-content" disabled={busy} onClick={() => setTaskDraft(task)}><strong className="task-title">{task.title}</strong><small>{task.dueDate ? localDate(task.dueDate, "M月d日") : "无截止日期"}</small></button><TaskPriority priority={task.priority} /><button className="subtle-delete task-delete" disabled={busy} title="删除任务" aria-label={`删除任务 ${task.title}`} onClick={() => void deleteTask(task)}><Trash2 size={16} /></button></div>)}</div>
        {visibleTasks.length > taskLimit && <Button variant="ghost" onClick={() => setTaskLimit((value) => value + 30)}>显示更多任务</Button>}
        {!visibleTasks.length && <EmptyState compact title={taskFilter === "done" ? "还没有已完成任务" : "任务已清空"} detail="可以切换状态查看其他任务，或规划下一件事。" />}
      </Card>
    </div>

    <TaskEditor task={taskDraft} onClose={() => setTaskDraft(null)} onChanged={load} />
    <Modal busy={busy} error={error} open={Boolean(eventDraft)} onClose={() => setEventDraft(null)} title={eventDraft?.id ? "编辑日程" : "新建日程"} footer={<><Button variant="secondary" onClick={() => setEventDraft(null)}>取消</Button><Button onClick={() => void saveEvent()}>保存日程</Button></>}>
      {eventDraft && <div className="form-grid"><Field label="日程标题" className="span-2"><Input autoFocus value={eventDraft.title} onChange={(event) => setEventDraft({ ...eventDraft, title: event.currentTarget.value })} /></Field><Field label="类型"><Select value={eventDraft.kind} onChange={(event) => setEventDraft({ ...eventDraft, kind: event.currentTarget.value as CalendarItem["kind"] })}><option value="event">事件</option><option value="timeblock">时间块</option></Select></Field><Field label="重复"><Select value={eventDraft.recurrence} onChange={(event) => setEventDraft({ ...eventDraft, recurrence: event.currentTarget.value as CalendarItem["recurrence"] })}><option value="none">不重复</option><option value="daily">每天</option><option value="weekly">每周</option><option value="monthly">每月</option></Select></Field><Field label="开始时间"><Input type="datetime-local" value={shanghaiInput(eventDraft.startAt)} onChange={(event) => setEventDraft({ ...eventDraft, startAt: event.currentTarget.value })} /></Field><Field label="结束时间" hint={eventDraft.allDay ? "全天日程的结束日期不包含当天" : undefined}><Input type="datetime-local" value={shanghaiInput(eventDraft.endAt)} onChange={(event) => setEventDraft({ ...eventDraft, endAt: event.currentTarget.value })} /></Field><Field label="项目"><Select value={eventDraft.projectId ?? ""} onChange={(event) => setEventDraft({ ...eventDraft, projectId: event.currentTarget.value || null })}><option value="">不关联项目</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</Select></Field><Field label="全天"><label className="checkbox-field"><input type="checkbox" checked={eventDraft.allDay} onChange={(event) => setEventDraft({ ...eventDraft, allDay: event.currentTarget.checked })} />这是全天事件</label></Field><Field label="说明" className="span-2"><Textarea rows={4} value={eventDraft.notes} onChange={(event) => setEventDraft({ ...eventDraft, notes: event.currentTarget.value })} /></Field></div>}
    </Modal>
  </div>;
}
