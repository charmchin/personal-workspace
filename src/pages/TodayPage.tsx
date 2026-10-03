import { useCallback, useEffect, useState } from "react";
import { ArrowRight, CalendarClock, Check, CircleDollarSign, Eye, EyeOff, Flame, ListChecks, NotebookPen, Plus, Sparkles } from "lucide-react";
import { z } from "zod";
import type { AppSettings, Dashboard, PageKey, SearchResult, Task } from "../types";
import { TaskEditor, TaskPriority } from "../components/TaskEditor";
import { call, WorkbenchError } from "../lib/api";
import { useMutation } from "../lib/useMutation";
import { emptyContent, emptyLearning, emptyTask, emptyWorkLog } from "../lib/defaults";
import { localDate, money } from "../lib/format";
import { useLocalDay } from "../lib/useLocalDay";
import { Button, Card, EmptyState, ErrorBanner, Field, Input, LoadingBlock, Modal, SectionHeader, Textarea } from "../components/ui";

const captureSchema = z.object({ title: z.string().trim().min(1, "请输入标题").max(240, "标题过长") });
type CaptureKind = "task" | "content" | "worklog" | "learning";

const contentStatus: Record<string, string> = { idea: "灵感", planning: "待策划", creating: "制作中", ready: "待发布", published: "已发布", review: "待复盘", archived: "已归档" };

export function TodayPage({ quickOpen, closeQuick, navigate, settings, onSettingsChange }: { quickOpen: boolean; closeQuick: () => void; navigate: (page: PageKey, target?: SearchResult) => void; settings: AppSettings; onSettingsChange: (settings: AppSettings) => void }) {
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const [captureKind, setCaptureKind] = useState<CaptureKind>("task");
  const [title, setTitle] = useState("");
  const [detail, setDetail] = useState("");
  const [taskDraft, setTaskDraft] = useState<Task | null>(null);
  const { busy: saving, mutate } = useMutation(setError);
  const date = useLocalDay();

  const load = useCallback(async () => {
    try { setDashboard(await call<Dashboard>("get_dashboard", { date })); setError(null); }
    catch (value) { setError(value as WorkbenchError); }
  }, [date]);
  useEffect(() => { void load(); }, [load]);

  async function toggleAmountVisibility() {
    await mutate(async () => {
      const updated = await call<AppSettings>("update_settings", { settings: { ...settings, amountsHidden: !settings.amountsHidden } });
      onSettingsChange(updated);
      setDashboard((current) => current ? { ...current, settings: updated } : current);
    });
  }

  async function saveCapture() {
    const parsed = captureSchema.safeParse({ title });
    if (!parsed.success) { setError(new WorkbenchError({ code: "VALIDATION_ERROR", message: parsed.error.issues[0].message })); return; }
    await mutate(async () => {
      if (captureKind === "task") await call("upsert_task", { task: { ...emptyTask(), title, notes: detail } });
      if (captureKind === "content") await call("upsert_content_item", { item: { ...emptyContent(), title, notes: detail } });
      if (captureKind === "worklog") await call("upsert_work_log", { log: { ...emptyWorkLog(), title, markdown: detail } });
      if (captureKind === "learning") await call("upsert_learning_item", { item: { ...emptyLearning(), title, notes: detail } });
      setTitle(""); setDetail(""); closeQuick(); await load();
    });
  }

  async function toggleTask(id: string, completed: boolean) {
    await mutate(async () => { await call("toggle_task", { id, completed }); await load(); });
  }

  async function toggleHabit(id: string, checked: boolean) {
    await mutate(async () => { await call("check_habit", { habitId: id, date, checked }); await load(); });
  }

  if (!dashboard && !error) return <LoadingBlock rows={7} />;
  const pending = dashboard?.tasks.filter((t) => t.status !== "done") ?? [];
  const completed = dashboard?.tasks.filter((t) => t.status === "done") ?? [];
  const pendingCount = dashboard?.pendingTaskCount ?? pending.length;
  const completedCount = dashboard?.completedTaskCount ?? completed.length;
  function taskRows(tasks: Task[], focus: boolean) {
    return <div className="task-list">{tasks.map((task, index) => <div className={`task-row ${task.status === "done" ? "is-complete" : ""}`} key={task.id}>
      <label className="task-checkbox"><input type="checkbox" aria-label={`${task.status === "done" ? "取消完成" : "完成任务"} ${task.title}`} disabled={saving} checked={task.status === "done"} onChange={(e) => void toggleTask(task.id, e.currentTarget.checked)} /><span className="custom-check"><Check size={13} /></span></label>
      <button className="task-details" disabled={saving} onClick={() => setTaskDraft(task)} aria-label={`编辑任务 ${task.title}`}><strong>{focus && index < 3 && <span className="focus-number">0{index + 1}</span>}<span className="task-title">{task.title}</span></strong><small>{task.status === "done" ? "已完成" : task.status === "doing" ? "进行中" : task.dueDate ? `截止 ${localDate(task.dueDate, "M月d日")}` : "没有截止日期"}</small></button>
      <TaskPriority priority={task.priority} />
    </div>)}</div>;
  }
  return (
    <div className="page today-page">
      {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
      <div className="today-heading">
        <div><span className="eyebrow">TODAY · {localDate(date, "yyyy.MM.dd")}</span><h1>{localDate(date, "EEEE")}，把重要的事向前推一点</h1><p>先完成最关键的三件事，再处理其余输入。</p></div>
        <Button onClick={() => document.dispatchEvent(new CustomEvent("workbench:quick-add"))}><Plus size={17} />快速记录</Button>
      </div>
      {dashboard && <>
        <section className="today-grid">
          <Card className="focus-card span-2">
            <SectionHeader eyebrow="FOCUS" title="今日重点" description={`待办 ${pendingCount} 项 · 今日完成 ${completedCount} 项`} action={<Button variant="ghost" size="sm" onClick={() => navigate("schedule")}>查看日程 <ArrowRight size={14} /></Button>} />
            <p className="focus-hint">显示到期、逾期、未设截止日期及计划今天开始的任务；工作记录不自动变成任务。</p>
            {pending.length ? taskRows(pending, true) : <EmptyState compact title="今天还没有待办任务" detail="记录一件最值得完成的事。" action={<Button size="sm" onClick={() => document.dispatchEvent(new CustomEvent("workbench:quick-add"))}>添加任务</Button>} />}
            {pendingCount > pending.length && <button className="text-link" onClick={() => navigate("schedule")}>还有 {pendingCount - pending.length} 项待办，查看全部 →</button>}
            {completed.length > 0 && <div className="completed-task-section"><h3>今日已完成</h3>{taskRows(completed, false)}{completedCount > completed.length && <button className="text-link" onClick={() => navigate("schedule", { id: "done", kind: "task-list", title: "已完成任务", subtitle: "" })}>还有 {completedCount - completed.length} 项已完成，查看日程 →</button>}</div>}
          </Card>
          <Card className="next-card">
            <div className="metric-icon amber"><CalendarClock size={20} /></div><span className="card-kicker">下一项日程</span>
            {dashboard.nextEvent ? <><h3>{dashboard.nextEvent.title}</h3><p className="big-time">{localDate(dashboard.nextEvent.startAt, "HH:mm")}</p><small>{localDate(dashboard.nextEvent.startAt)}</small></> : <><h3>日程留白</h3><p className="muted">今天暂时没有安排</p></>}
          </Card>
          <Card className="portfolio-card">
            <div className="card-top"><div className="metric-icon green"><CircleDollarSign size={20} /></div><button className="inline-icon" onClick={() => void toggleAmountVisibility()} aria-label={settings.amountsHidden ? "显示金额" : "隐藏金额"}>{settings.amountsHidden ? <EyeOff size={17} /> : <Eye size={17} />}</button></div>
            <span className="card-kicker">投资组合</span><h3 className="money-value">{dashboard.portfolioError ? "暂不可用" : money(dashboard.portfolio.totalMarketValue, settings.amountsHidden)}</h3><p className={Number(dashboard.portfolio.unrealizedGain) >= 0 ? "gain" : "loss"}>{dashboard.portfolioError ?? (dashboard.portfolio.valuationComplete === false ? "价格不完整，仅展示已知市值小计" : `未实现 ${money(dashboard.portfolio.unrealizedGain, settings.amountsHidden)}`)}</p><small>{dashboard.portfolio.updatedAt ? `数据更新至 ${dashboard.portfolio.updatedAt}` : "尚未录入持仓"}</small>
          </Card>
        </section>

        <section className="today-lower-grid">
          <Card>
            <SectionHeader title="习惯节奏" action={<Button size="sm" variant="ghost" onClick={() => navigate("growth")}>管理</Button>} />
            {dashboard.habits.some((habit) => habit.active) ? <div className="habit-mini-list">{dashboard.habits.filter((habit) => habit.active).slice(0, 5).map((habit) => <button key={habit.id} className={`habit-mini ${habit.checkedToday ? "checked" : ""}`} onClick={() => void toggleHabit(habit.id, !habit.checkedToday)}><span style={{ background: habit.color }}>{habit.checkedToday ? <Check size={14} /> : null}</span><div><strong>{habit.name}</strong><small><Flame size={12} /> 连续 {habit.streak} 天</small></div></button>)}</div> : <EmptyState compact title="还没有习惯" detail="从一个可持续的小动作开始。" />}
          </Card>
          <Card>
            <SectionHeader title="内容推进" action={<Button size="sm" variant="ghost" onClick={() => navigate("media")}>看板</Button>} />
            {dashboard.contentItems.length ? <div className="simple-list">{dashboard.contentItems.map((item) => <button key={item.id} onClick={() => navigate("media", { id: item.id, kind: "content", title: item.title, subtitle: "" })}><span className="list-icon"><Sparkles size={15} /></span><div><strong>{item.title}</strong><small>{item.platform} · {contentStatus[item.status]}</small></div>{item.publishAt && <time>{localDate(item.publishAt, "M/d")}</time>}</button>)}</div> : <EmptyState compact title="内容池是空的" detail="把刚冒出来的想法先收下来。" />}
          </Card>
          <Card>
            <SectionHeader title="今日工作记录" action={<Button size="sm" variant="ghost" onClick={() => navigate("work")}>全部记录</Button>} />
            {dashboard.workLogs.length ? <div className="simple-list">{dashboard.workLogs.map((log) => <button key={log.id} onClick={() => navigate("work", { id: log.id, kind: "worklog", title: log.title, subtitle: "" })}><span className="list-icon"><NotebookPen size={15} /></span><div><strong>{log.title}</strong><small>{log.completed || "尚未填写完成事项"}</small></div><time>{log.minutes}m</time></button>)}</div> : <EmptyState compact title="今天尚未记录" detail="完成一段工作后，留下结果和下一步。" />}
          </Card>
        </section>
      </>}

      <TaskEditor task={taskDraft} onClose={() => setTaskDraft(null)} onChanged={load} />
      <Modal busy={saving} error={error} open={quickOpen} onClose={closeQuick} title="快速记录" description="先捕获，再到对应模块继续整理。" footer={<><Button variant="secondary" onClick={closeQuick}>取消</Button><Button onClick={() => void saveCapture()} disabled={saving}>{saving ? "保存中…" : "保存记录"}</Button></>}>
        <div className="capture-kinds">
          {([ ["task", ListChecks, "任务"], ["content", Sparkles, "灵感"], ["worklog", NotebookPen, "工作"], ["learning", Plus, "学习"] ] as const).map(([kind, Icon, label]) => <button key={kind} className={captureKind === kind ? "active" : ""} onClick={() => setCaptureKind(kind)}><Icon size={17} />{label}</button>)}
        </div>
        <div className="form-grid one-column">
          <Field label="标题"><Input autoFocus value={title} onChange={(event) => setTitle(event.currentTarget.value)} placeholder={captureKind === "content" ? "刚想到的选题…" : "需要记住什么？"} /></Field>
          <Field label="补充说明"><Textarea rows={5} value={detail} onChange={(event) => setDetail(event.currentTarget.value)} placeholder="背景、下一步或随手备注（可选）" /></Field>
          <p className="capture-guidance">{captureKind === "task" ? "任务：需要去做的行动，默认今天到期；可在日程编辑优先级和关联。" : captureKind === "worklog" ? "工作记录：保存结果、问题和下一步，不自动创建待办；之后可明确创建关联任务。耗时需自行填写。" : captureKind === "content" ? "灵感：进入自媒体内容池，可在内容编辑窗口创建关联任务。" : "学习：保存学习项目，不自动变成待办。"}</p>
        </div>
      </Modal>
    </div>
  );
}
