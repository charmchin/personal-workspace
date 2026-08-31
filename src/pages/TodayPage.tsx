import { useCallback, useEffect, useState } from "react";
import { format } from "date-fns";
import { zhCN } from "date-fns/locale";
import { ArrowRight, CalendarClock, Check, CircleDollarSign, Eye, EyeOff, Flame, ListChecks, NotebookPen, Plus, Sparkles } from "lucide-react";
import { z } from "zod";
import type { AppSettings, Dashboard, PageKey } from "../types";
import { call, WorkbenchError } from "../lib/api";
import { emptyContent, emptyLearning, emptyTask, emptyWorkLog } from "../lib/defaults";
import { localDate, money } from "../lib/format";
import { useLocalDay } from "../lib/useLocalDay";
import { Badge, Button, Card, EmptyState, ErrorBanner, Field, Input, LoadingBlock, Modal, SectionHeader, Select, Textarea } from "../components/ui";

const captureSchema = z.object({ title: z.string().trim().min(1, "请输入标题").max(240, "标题过长") });
type CaptureKind = "task" | "content" | "worklog" | "learning";

const contentStatus: Record<string, string> = { idea: "灵感", planning: "待策划", creating: "制作中", ready: "待发布", published: "已发布", review: "待复盘", archived: "已归档" };

export function TodayPage({ quickOpen, closeQuick, navigate, settings, onSettingsChange }: { quickOpen: boolean; closeQuick: () => void; navigate: (page: PageKey) => void; settings: AppSettings; onSettingsChange: (settings: AppSettings) => void }) {
  const [dashboard, setDashboard] = useState<Dashboard | null>(null);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const [captureKind, setCaptureKind] = useState<CaptureKind>("task");
  const [title, setTitle] = useState("");
  const [detail, setDetail] = useState("");
  const [saving, setSaving] = useState(false);
  const date = useLocalDay();

  const load = useCallback(async () => {
    try { setDashboard(await call<Dashboard>("get_dashboard", { date })); setError(null); }
    catch (value) { setError(value as WorkbenchError); }
  }, [date]);
  useEffect(() => { void load(); }, [load]);

  async function toggleAmountVisibility() {
    const updated = await call<AppSettings>("update_settings", { settings: { ...settings, amountsHidden: !settings.amountsHidden } });
    onSettingsChange(updated);
    setDashboard((current) => current ? { ...current, settings: updated } : current);
  }

  async function saveCapture() {
    const parsed = captureSchema.safeParse({ title });
    if (!parsed.success) { setError(new WorkbenchError({ code: "VALIDATION_ERROR", message: parsed.error.issues[0].message })); return; }
    setSaving(true);
    try {
      if (captureKind === "task") await call("upsert_task", { task: { ...emptyTask(), title, notes: detail } });
      if (captureKind === "content") await call("upsert_content_item", { item: { ...emptyContent(), title, notes: detail } });
      if (captureKind === "worklog") await call("upsert_work_log", { log: { ...emptyWorkLog(), title, markdown: detail } });
      if (captureKind === "learning") await call("upsert_learning_item", { item: { ...emptyLearning(), title, notes: detail } });
      setTitle(""); setDetail(""); closeQuick(); await load();
    } catch (value) { setError(value as WorkbenchError); }
    finally { setSaving(false); }
  }

  async function toggleTask(id: string, completed: boolean) {
    try { await call("toggle_task", { id, completed }); await load(); }
    catch (value) { setError(value as WorkbenchError); }
  }

  async function toggleHabit(id: string, checked: boolean) {
    try { await call("check_habit", { habitId: id, date, checked }); await load(); }
    catch (value) { setError(value as WorkbenchError); }
  }

  if (!dashboard && !error) return <LoadingBlock rows={7} />;
  return (
    <div className="page today-page">
      {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
      <div className="today-heading">
        <div><span className="eyebrow">TODAY · {format(new Date(), "yyyy.MM.dd")}</span><h1>{format(new Date(), "EEEE", { locale: zhCN })}，把重要的事向前推一点</h1><p>先完成最关键的三件事，再处理其余输入。</p></div>
        <Button onClick={() => document.dispatchEvent(new CustomEvent("workbench:quick-add"))}><Plus size={17} />快速记录</Button>
      </div>
      {dashboard && <>
        <section className="today-grid">
          <Card className="focus-card span-2">
            <SectionHeader eyebrow="FOCUS" title="今日重点" description={`${dashboard.tasks.filter((item) => item.status === "done").length}/${dashboard.tasks.length} 已完成`} action={<Button variant="ghost" size="sm" onClick={() => navigate("schedule")}>查看日程 <ArrowRight size={14} /></Button>} />
            {dashboard.tasks.length ? <div className="task-list">{dashboard.tasks.slice(0, 5).map((task, index) => <label className="task-row" key={task.id}><input type="checkbox" checked={task.status === "done"} onChange={(event) => void toggleTask(task.id, event.currentTarget.checked)} /><span className="custom-check"><Check size={13} /></span><div><strong>{index < 3 && <span className="focus-number">0{index + 1}</span>}{task.title}</strong><small>{task.dueDate ? `截止 ${localDate(task.dueDate, "M月d日")}` : "没有截止日期"}{task.notes ? ` · ${task.notes}` : ""}</small></div><Badge tone={task.priority === 1 ? "red" : task.priority === 2 ? "amber" : "neutral"}>P{task.priority}</Badge></label>)}</div> : <EmptyState compact title="今天还没有任务" detail="记录一件最值得完成的事。" action={<Button size="sm" onClick={() => document.dispatchEvent(new CustomEvent("workbench:quick-add"))}>添加任务</Button>} />}
          </Card>
          <Card className="next-card">
            <div className="metric-icon amber"><CalendarClock size={20} /></div><span className="card-kicker">下一项日程</span>
            {dashboard.nextEvent ? <><h3>{dashboard.nextEvent.title}</h3><p className="big-time">{localDate(dashboard.nextEvent.startAt, "HH:mm")}</p><small>{localDate(dashboard.nextEvent.startAt)}</small></> : <><h3>日程留白</h3><p className="muted">今天暂时没有安排</p></>}
          </Card>
          <Card className="portfolio-card">
            <div className="card-top"><div className="metric-icon green"><CircleDollarSign size={20} /></div><button className="inline-icon" onClick={() => void toggleAmountVisibility()} aria-label={settings.amountsHidden ? "显示金额" : "隐藏金额"}>{settings.amountsHidden ? <EyeOff size={17} /> : <Eye size={17} />}</button></div>
            <span className="card-kicker">投资组合</span><h3 className="money-value">{money(dashboard.portfolio.totalMarketValue, settings.amountsHidden)}</h3><p className={Number(dashboard.portfolio.unrealizedGain) >= 0 ? "gain" : "loss"}>未实现 {money(dashboard.portfolio.unrealizedGain, settings.amountsHidden)}</p><small>{dashboard.portfolio.updatedAt ? `数据更新至 ${dashboard.portfolio.updatedAt}` : "尚未录入持仓"}</small>
          </Card>
        </section>

        <section className="today-lower-grid">
          <Card>
            <SectionHeader title="习惯节奏" action={<Button size="sm" variant="ghost" onClick={() => navigate("growth")}>管理</Button>} />
            {dashboard.habits.length ? <div className="habit-mini-list">{dashboard.habits.filter((habit) => habit.active).slice(0, 5).map((habit) => <button key={habit.id} className={`habit-mini ${habit.checkedToday ? "checked" : ""}`} onClick={() => void toggleHabit(habit.id, !habit.checkedToday)}><span style={{ background: habit.color }}>{habit.checkedToday ? <Check size={14} /> : null}</span><div><strong>{habit.name}</strong><small><Flame size={12} /> 连续 {habit.streak} 天</small></div></button>)}</div> : <EmptyState compact title="还没有习惯" detail="从一个可持续的小动作开始。" />}
          </Card>
          <Card>
            <SectionHeader title="内容推进" action={<Button size="sm" variant="ghost" onClick={() => navigate("media")}>看板</Button>} />
            {dashboard.contentItems.length ? <div className="simple-list">{dashboard.contentItems.map((item) => <button key={item.id} onClick={() => navigate("media")}><span className="list-icon"><Sparkles size={15} /></span><div><strong>{item.title}</strong><small>{item.platform} · {contentStatus[item.status]}</small></div>{item.publishAt && <time>{localDate(item.publishAt, "M/d")}</time>}</button>)}</div> : <EmptyState compact title="内容池是空的" detail="把刚冒出来的想法先收下来。" />}
          </Card>
          <Card>
            <SectionHeader title="今日工作记录" action={<Button size="sm" variant="ghost" onClick={() => navigate("work")}>全部记录</Button>} />
            {dashboard.workLogs.length ? <div className="simple-list">{dashboard.workLogs.map((log) => <button key={log.id} onClick={() => navigate("work")}><span className="list-icon"><NotebookPen size={15} /></span><div><strong>{log.title}</strong><small>{log.completed || "尚未填写完成事项"}</small></div><time>{log.minutes}m</time></button>)}</div> : <EmptyState compact title="今天尚未记录" detail="完成一段工作后，留下结果和下一步。" />}
          </Card>
        </section>
      </>}

      <Modal open={quickOpen} onClose={closeQuick} title="快速记录" description="先捕获，再到对应模块继续整理。" footer={<><Button variant="secondary" onClick={closeQuick}>取消</Button><Button onClick={() => void saveCapture()} disabled={saving}>{saving ? "保存中…" : "保存记录"}</Button></>}>
        <div className="capture-kinds">
          {([ ["task", ListChecks, "任务"], ["content", Sparkles, "灵感"], ["worklog", NotebookPen, "工作"], ["learning", Plus, "学习"] ] as const).map(([kind, Icon, label]) => <button key={kind} className={captureKind === kind ? "active" : ""} onClick={() => setCaptureKind(kind)}><Icon size={17} />{label}</button>)}
        </div>
        <div className="form-grid one-column">
          <Field label="标题"><Input autoFocus value={title} onChange={(event) => setTitle(event.currentTarget.value)} placeholder={captureKind === "content" ? "刚想到的选题…" : "需要记住什么？"} /></Field>
          <Field label="补充说明"><Textarea rows={5} value={detail} onChange={(event) => setDetail(event.currentTarget.value)} placeholder="背景、下一步或随手备注（可选）" /></Field>
          {captureKind === "task" && <Field label="记录后"><Select disabled><option>进入今日任务</option></Select></Field>}
        </div>
      </Modal>
    </div>
  );
}
