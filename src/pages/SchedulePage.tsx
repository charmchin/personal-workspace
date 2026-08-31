import { useCallback, useEffect, useMemo, useState } from "react";
import { addDays, addMonths, eachDayOfInterval, endOfMonth, endOfWeek, format, isSameDay, isSameMonth, parseISO, startOfMonth, startOfWeek, subMonths } from "date-fns";
import { zhCN } from "date-fns/locale";
import { open, save } from "@tauri-apps/plugin-dialog";
import { CalendarPlus, ChevronLeft, ChevronRight, CircleCheck, Import as ImportIcon, FileOutput as ExportIcon, Plus } from "lucide-react";
import type { CalendarItem, ImportReport, Project, Task } from "../types";
import { call, WorkbenchError } from "../lib/api";
import { emptyCalendar, emptyTask } from "../lib/defaults";
import { localDate } from "../lib/format";
import { Badge, Button, Card, EmptyState, ErrorBanner, Field, Input, Modal, SectionHeader, Select, Textarea } from "../components/ui";

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

export function SchedulePage() {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [items, setItems] = useState<CalendarItem[]>([]);
  const [projects, setProjects] = useState<Project[]>([]);
  const [cursor, setCursor] = useState(new Date());
  const [view, setView] = useState<ViewMode>("month");
  const [taskDraft, setTaskDraft] = useState<Task | null>(null);
  const [eventDraft, setEventDraft] = useState<CalendarItem | null>(null);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);

  const load = useCallback(async () => {
    try {
      const [taskData, calendarData, projectData] = await Promise.all([
        call<Task[]>("list_tasks"), call<CalendarItem[]>("list_calendar_items", { start: null, end: null }), call<Project[]>("list_projects"),
      ]);
      setTasks(taskData); setItems(calendarData); setProjects(projectData); setError(null);
    } catch (value) { setError(value as WorkbenchError); }
  }, []);
  useEffect(() => { void load(); }, [load]);

  const calendarDays = useMemo(() => {
    if (view === "day") return [cursor];
    if (view === "week") return eachDayOfInterval({ start: startOfWeek(cursor, { weekStartsOn: 1 }), end: endOfWeek(cursor, { weekStartsOn: 1 }) });
    const monthStart = startOfMonth(cursor);
    return eachDayOfInterval({ start: startOfWeek(monthStart, { weekStartsOn: 1 }), end: endOfWeek(endOfMonth(monthStart), { weekStartsOn: 1 }) });
  }, [cursor, view]);

  const step = (direction: number) => {
    if (view === "month") setCursor((current) => direction > 0 ? addMonths(current, 1) : subMonths(current, 1));
    else setCursor((current) => addDays(current, direction * (view === "week" ? 7 : 1)));
  };

  async function saveTask() {
    if (!taskDraft) return;
    try { await call("upsert_task", { task: taskDraft }); setTaskDraft(null); await load(); }
    catch (value) { setError(value as WorkbenchError); }
  }
  async function saveEvent() {
    if (!eventDraft) return;
    try { await call("upsert_calendar_item", { item: eventDraft }); setEventDraft(null); await load(); }
    catch (value) { setError(value as WorkbenchError); }
  }
  async function toggleTask(task: Task) {
    try { await call("toggle_task", { id: task.id, completed: task.status !== "done" }); await load(); }
    catch (value) { setError(value as WorkbenchError); }
  }
  async function importIcs() {
    const path = await open({ multiple: false, filters: [{ name: "iCalendar", extensions: ["ics"] }] });
    if (!path) return;
    try { const report = await call<ImportReport>("import_ics", { path, source: `ics:${path}` }); setNotice(`已导入 ${report.imported} 项，更新 ${report.updated} 项，跳过 ${report.skipped} 项`); await load(); }
    catch (value) { setError(value as WorkbenchError); }
  }
  async function exportIcs() {
    const path = await save({ defaultPath: `个人工作台日程-${format(new Date(), "yyyy-MM-dd")}.ics`, filters: [{ name: "iCalendar", extensions: ["ics"] }] });
    if (!path) return;
    try { const count = await call<number>("export_ics", { path }); setNotice(`已导出 ${count} 项日程`); }
    catch (value) { setError(value as WorkbenchError); }
  }

  return <div className="page schedule-page">
    {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
    {notice && <div className="notice-banner"><CircleCheck size={17} />{notice}<button onClick={() => setNotice(null)}>×</button></div>}
    <div className="page-heading"><div><span className="eyebrow">PLAN WITH INTENTION</span><h1>日程计划</h1><p>把任务和时间放在同一张地图上，给真正重要的事留出空间。</p></div><div className="heading-actions"><Button variant="secondary" onClick={() => void importIcs()} title="从 .ics 文件读入日程"><ImportIcon size={16} />导入 ICS</Button><Button variant="secondary" onClick={() => void exportIcs()} title="将工作台日程写入 .ics 文件"><ExportIcon size={16} />导出 ICS</Button><Button onClick={() => setEventDraft(emptyCalendar())}><CalendarPlus size={16} />新建日程</Button></div></div>
    <div className="schedule-toolbar">
      <div className="date-navigation"><Button variant="ghost" size="sm" onClick={() => step(-1)}><ChevronLeft size={17} /></Button><button className="month-label" onClick={() => setCursor(new Date())}>{view === "month" ? format(cursor, "yyyy年 M月", { locale: zhCN }) : format(cursor, "M月d日", { locale: zhCN })}</button><Button variant="ghost" size="sm" onClick={() => step(1)}><ChevronRight size={17} /></Button></div>
      <div className="segmented">{(["day", "week", "month"] as ViewMode[]).map((mode) => <button className={view === mode ? "active" : ""} key={mode} onClick={() => setView(mode)}>{mode === "day" ? "日" : mode === "week" ? "周" : "月"}</button>)}</div>
    </div>
    <div className="schedule-layout">
      <Card className="calendar-card">
        {view !== "day" && <div className={`calendar-weekdays ${view === "week" ? "week" : ""}`}>{["一", "二", "三", "四", "五", "六", "日"].map((day) => <span key={day}>周{day}</span>)}</div>}
        <div className={`calendar-grid calendar-view-${view}`}>
          {calendarDays.map((day) => {
            const dayItems = items.filter((item) => recursOn(item.startAt, item.recurrence, day));
            const dayTasks = tasks.filter((task) => task.dueDate && recursOn(task.dueDate, task.recurrence, day) && task.status !== "done");
            const visibleItems = dayItems.slice(0, view === "month" ? 3 : 8);
            const visibleTasks = dayTasks.slice(0, 2);
            const hiddenCount = dayItems.length + dayTasks.length - visibleItems.length - visibleTasks.length;
            return <button key={day.toISOString()} className={`calendar-day ${!isSameMonth(day, cursor) && view === "month" ? "outside" : ""} ${isSameDay(day, new Date()) ? "today" : ""}`} onDoubleClick={() => setEventDraft({ ...emptyCalendar(), startAt: `${format(day, "yyyy-MM-dd")}T09:00`, endAt: `${format(day, "yyyy-MM-dd")}T10:00` })}><span className="day-number">{format(day, "d")}</span><div className="day-items">{visibleItems.map((item) => <span key={item.id} className={`day-event ${item.kind}`} onClick={(event) => { event.stopPropagation(); setEventDraft(item); }}>{item.allDay ? "" : localDate(item.startAt, "HH:mm ")}{item.title}</span>)}{visibleTasks.map((task) => <span key={task.id} className="day-task" onClick={(event) => { event.stopPropagation(); setTaskDraft(task); }}>□ {task.title}</span>)}{hiddenCount > 0 && <small>还有 {hiddenCount} 项</small>}</div></button>;
          })}
        </div>
      </Card>
      <Card className="task-panel">
        <SectionHeader title="任务清单" description={`${tasks.filter((item) => item.status !== "done").length} 项待完成`} action={<Button size="sm" onClick={() => setTaskDraft(emptyTask())}><Plus size={15} />任务</Button>} />
        <div className="task-panel-list">{tasks.filter((task) => task.status !== "done").slice(0, 14).map((task) => <div key={task.id} className="panel-task"><button className="round-check" onClick={() => void toggleTask(task)} aria-label="完成任务" /><button className="task-content" onClick={() => setTaskDraft(task)}><strong>{task.title}</strong><small>{task.dueDate ? localDate(task.dueDate, "M月d日") : "无截止日期"}</small></button><Badge tone={task.priority === 1 ? "red" : task.priority === 2 ? "amber" : "neutral"}>P{task.priority}</Badge></div>)}</div>
        {!tasks.some((task) => task.status !== "done") && <EmptyState compact title="任务已清空" detail="享受片刻留白，或规划下一件事。" />}
      </Card>
    </div>

    <Modal open={Boolean(taskDraft)} onClose={() => setTaskDraft(null)} title={taskDraft?.id ? "编辑任务" : "新建任务"} footer={<><Button variant="secondary" onClick={() => setTaskDraft(null)}>取消</Button><Button onClick={() => void saveTask()}>保存任务</Button></>}>
      {taskDraft && <div className="form-grid"><Field label="任务标题" className="span-2"><Input autoFocus value={taskDraft.title} onChange={(event) => setTaskDraft({ ...taskDraft, title: event.currentTarget.value })} /></Field><Field label="截止日期"><Input type="date" value={taskDraft.dueDate ?? ""} onChange={(event) => setTaskDraft({ ...taskDraft, dueDate: event.currentTarget.value || null })} /></Field><Field label="优先级"><Select value={taskDraft.priority} onChange={(event) => setTaskDraft({ ...taskDraft, priority: Number(event.currentTarget.value) as 1 | 2 | 3 })}><option value="1">P1 · 重要</option><option value="2">P2 · 正常</option><option value="3">P3 · 稍后</option></Select></Field><Field label="状态"><Select value={taskDraft.status} onChange={(event) => setTaskDraft({ ...taskDraft, status: event.currentTarget.value as Task["status"] })}><option value="todo">待办</option><option value="doing">进行中</option><option value="done">已完成</option></Select></Field><Field label="重复"><Select value={taskDraft.recurrence} onChange={(event) => setTaskDraft({ ...taskDraft, recurrence: event.currentTarget.value as Task["recurrence"] })}><option value="none">不重复</option><option value="daily">每天</option><option value="weekly">每周</option><option value="monthly">每月</option></Select></Field><Field label="项目"><Select value={taskDraft.projectId ?? ""} onChange={(event) => setTaskDraft({ ...taskDraft, projectId: event.currentTarget.value || null })}><option value="">不关联项目</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</Select></Field><Field label="说明" className="span-2"><Textarea rows={4} value={taskDraft.notes} onChange={(event) => setTaskDraft({ ...taskDraft, notes: event.currentTarget.value })} /></Field></div>}
    </Modal>
    <Modal open={Boolean(eventDraft)} onClose={() => setEventDraft(null)} title={eventDraft?.id ? "编辑日程" : "新建日程"} footer={<><Button variant="secondary" onClick={() => setEventDraft(null)}>取消</Button><Button onClick={() => void saveEvent()}>保存日程</Button></>}>
      {eventDraft && <div className="form-grid"><Field label="日程标题" className="span-2"><Input autoFocus value={eventDraft.title} onChange={(event) => setEventDraft({ ...eventDraft, title: event.currentTarget.value })} /></Field><Field label="类型"><Select value={eventDraft.kind} onChange={(event) => setEventDraft({ ...eventDraft, kind: event.currentTarget.value as CalendarItem["kind"] })}><option value="event">事件</option><option value="timeblock">时间块</option></Select></Field><Field label="重复"><Select value={eventDraft.recurrence} onChange={(event) => setEventDraft({ ...eventDraft, recurrence: event.currentTarget.value as CalendarItem["recurrence"] })}><option value="none">不重复</option><option value="daily">每天</option><option value="weekly">每周</option><option value="monthly">每月</option></Select></Field><Field label="开始时间"><Input type="datetime-local" value={eventDraft.startAt.slice(0, 16)} onChange={(event) => setEventDraft({ ...eventDraft, startAt: event.currentTarget.value })} /></Field><Field label="结束时间"><Input type="datetime-local" value={eventDraft.endAt.slice(0, 16)} onChange={(event) => setEventDraft({ ...eventDraft, endAt: event.currentTarget.value })} /></Field><Field label="项目"><Select value={eventDraft.projectId ?? ""} onChange={(event) => setEventDraft({ ...eventDraft, projectId: event.currentTarget.value || null })}><option value="">不关联项目</option>{projects.map((project) => <option key={project.id} value={project.id}>{project.name}</option>)}</Select></Field><Field label="全天"><label className="checkbox-field"><input type="checkbox" checked={eventDraft.allDay} onChange={(event) => setEventDraft({ ...eventDraft, allDay: event.currentTarget.checked })} />这是全天事件</label></Field><Field label="说明" className="span-2"><Textarea rows={4} value={eventDraft.notes} onChange={(event) => setEventDraft({ ...eventDraft, notes: event.currentTarget.value })} /></Field></div>}
    </Modal>
  </div>;
}
