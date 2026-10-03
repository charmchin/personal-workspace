import { useCallback, useEffect, useMemo, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { endOfWeek, isWithinInterval, parseISO, startOfWeek } from "date-fns";
import { AlertTriangle, ArrowRight, CheckCircle2, Clock3, FileText, FolderKanban, Gauge, Paperclip, Plus, Trash2 } from "lucide-react";
import type { Project, WorkLog, SearchResult, Task } from "../types";
import { TaskEditor } from "../components/TaskEditor";
import { call, WorkbenchError } from "../lib/api";
import { useLocalDay } from "../lib/useLocalDay";
import { useSearchTarget } from "../lib/useSearchTarget";
import { useMutation } from "../lib/useMutation";
import { emptyProject, emptyTask, emptyWorkLog } from "../lib/defaults";
import { localDate, number } from "../lib/format";
import { Badge, Button, Card, EmptyState, ErrorBanner, Field, Input, Modal, SectionHeader, Select, Textarea } from "../components/ui";

export function WorkPage({ searchTarget }: { searchTarget?: SearchResult | null }) {
  const [projects, setProjects] = useState<Project[]>([]);
  const [logs, setLogs] = useState<WorkLog[]>([]);
  const [tasks, setTasks] = useState<Task[]>([]);
  const [taskDraft, setTaskDraft] = useState<Task | null>(null);
  const [taskSource, setTaskSource] = useState<string | null>(null);
  const [projectDraft, setProjectDraft] = useState<Project | null>(null);
  const [logDraft, setLogDraft] = useState<WorkLog | null>(null);
  const [filterProject, setFilterProject] = useState("");
  const [error, setError] = useState<WorkbenchError | null>(null);
  const { busy, mutate } = useMutation(setError);
  const load = useCallback(async () => { try { const [projectData, logData, taskData] = await Promise.all([call<Project[]>("list_projects"), call<WorkLog[]>("list_work_logs"), call<Task[]>("list_tasks")]); setProjects(projectData); setLogs(logData); setTasks(taskData); setError(null); } catch (value) { setError(value as WorkbenchError); } }, []);
  useEffect(() => { void load(); }, [load]);
  const [logLimit, setLogLimit] = useState(30);
  useSearchTarget(searchTarget, "project", projects, setProjectDraft);
  useSearchTarget(searchTarget, "worklog", logs, setLogDraft);
  const visibleLogs = filterProject ? logs.filter((log) => log.projectId === filterProject) : logs;
  const date = useLocalDay();
  const weekLogs = useMemo(() => {
    const today = new Date(`${date}T12:00:00`);
    const interval = { start: startOfWeek(today, { weekStartsOn: 1 }), end: endOfWeek(today, { weekStartsOn: 1 }) };
    return logs.filter((log) => { try { return isWithinInterval(parseISO(log.logDate), interval); } catch { return false; } });
  }, [logs, date]);
  async function saveProject() { if (!projectDraft) return; await mutate(async () => { await call("upsert_project", { project: projectDraft }); setProjectDraft(null); await load(); }); }
  async function saveLog() { if (!logDraft) return; await mutate(async () => { await call("upsert_work_log", { log: logDraft }); setLogDraft(null); await load(); }); }
  async function chooseAttachment() { await mutate(async () => { const path = await open({ multiple: false }); if (path && logDraft) setLogDraft({ ...logDraft, attachmentPath: path }); }); }
  async function removeLog(log: WorkLog) { if (!window.confirm(`确认删除“${log.title}”？`)) return; try { await call("delete_record", { kind: "worklog", id: log.id }); await load(); } catch (value) { setError(value as WorkbenchError); } }
  function prepareTask(log: WorkLog) {
    const saved = logs.find((item) => item.id === log.id);
    if (!saved) return;
    const existing = saved.taskId ? tasks.find((task) => task.id === saved.taskId) : null;
    if (saved.taskId && !existing) { setError(new WorkbenchError({ code: "NOT_FOUND", message: "关联任务已变化，请重新打开工作记录后再试" })); return; }
    setTaskSource(saved.id);
    setTaskDraft(existing ?? { ...emptyTask(), title: (saved.nextSteps.trim() || saved.title).slice(0, 240), projectId: saved.projectId, notes: `来源工作记录：${saved.title}` });
  }
  async function taskChanged(saved?: Task) {
    setLogDraft((current) => current && current.id === taskSource ? { ...current, taskId: saved?.id ?? null } : current);
    await load();
  }

  return <div className="page work-page">
    {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
    <div className="page-heading"><div><span className="eyebrow">DOCUMENT THE WORK</span><h1>工作记录</h1><p>用结果、问题与下一步建立连续的项目记忆。</p></div><div className="heading-actions"><Button variant="secondary" onClick={() => setProjectDraft(emptyProject())}><FolderKanban size={16} />新建项目</Button><Button onClick={() => setLogDraft({ ...emptyWorkLog(), projectId: filterProject || null })}><Plus size={16} />记录工作</Button></div></div>
    <div className="work-stats">
      <Card><span className="metric-icon green"><Clock3 size={19} /></span><div><small>本周记录时间</small><strong>{number(weekLogs.reduce((sum, log) => sum + log.minutes, 0) / 60, 1)} 小时</strong></div></Card>
      <Card><span className="metric-icon amber"><FileText size={19} /></span><div><small>本周日志</small><strong>{weekLogs.length} 篇</strong></div></Card>
      <Card><span className="metric-icon blue"><Gauge size={19} /></span><div><small>平均精力</small><strong>{weekLogs.length ? number(weekLogs.reduce((sum, log) => sum + log.energy, 0) / weekLogs.length, 1) : "—"} / 5</strong></div></Card>
    </div>
    <div className="work-layout">
      <aside className="project-rail"><SectionHeader title="项目" action={<button onClick={() => setProjectDraft(emptyProject())}><Plus size={15} /></button>} /><button className={!filterProject ? "active" : ""} onClick={() => setFilterProject("")}><span className="project-color all" />全部记录<small>{logs.length}</small></button>{projects.map((project) => <button key={project.id} className={filterProject === project.id ? "active" : ""} onClick={() => setFilterProject(project.id)} onDoubleClick={() => setProjectDraft(project)}><span className="project-color" style={{ background: project.color }} />{project.name}<small>{logs.filter((log) => log.projectId === project.id).length}</small></button>)}</aside>
      <section className="log-timeline module-section"><SectionHeader title={filterProject ? projects.find((project) => project.id === filterProject)?.name ?? "项目记录" : "最近记录"} description="按日期倒序排列，双击项目可编辑。" />{visibleLogs.length ? visibleLogs.slice(0, logLimit).map((log) => <Card key={log.id} className="work-log-card"><div className="log-date"><strong>{localDate(log.logDate, "dd")}</strong><span>{localDate(log.logDate, "M月")}</span></div><div className="log-body"><div className="log-title-row"><button onClick={() => setLogDraft(log)}><h3>{log.title}</h3></button><div><Badge tone="neutral">{log.minutes} 分钟</Badge><Badge tone={log.energy >= 4 ? "green" : log.energy <= 2 ? "red" : "amber"}>精力 {log.energy}/5</Badge></div></div><div className="log-structured"><div><span><CheckCircle2 size={15} />完成</span><p>{log.completed || "未填写"}</p></div><div><span><AlertTriangle size={15} />问题</span><p>{log.blockers || "暂无阻塞"}</p></div><div><span><ArrowRight size={15} />下一步</span><p>{log.nextSteps || "未填写"}</p></div></div>{log.markdown && <pre className="markdown-preview">{log.markdown}</pre>}<footer>{log.attachmentPath && <button onClick={() => void call("open_saved_file", { path: log.attachmentPath! }).catch((value) => setError(value as WorkbenchError))}><Paperclip size={14} />打开附件</button>}<button disabled={busy} onClick={() => prepareTask(log)}>{log.taskId ? "查看关联任务" : "创建待办任务"}</button><button className="danger" onClick={() => void removeLog(log)}><Trash2 size={14} />删除</button></footer></div></Card>) : <EmptyState title="还没有工作记录" detail="完成一段工作后，写下结果、问题与下一步。" action={<Button onClick={() => setLogDraft({ ...emptyWorkLog(), projectId: filterProject || null })}>写第一条记录</Button>} />}</section>
    </div>

    <TaskEditor task={taskDraft} onClose={() => setTaskDraft(null)} onChanged={taskChanged} description={taskDraft?.id ? "编辑关联任务不会自动改写工作记录。" : "基于已保存的工作记录创建待办；原记录保留，保存时同时建立关联。"} save={taskDraft?.id ? undefined : (task) => call<Task>("create_task_from_work_log", { logId: taskSource, task })} />
    {visibleLogs.length > logLimit && <Button variant="ghost" onClick={() => setLogLimit((value) => value + 30)}>显示更多日志</Button>}
    <Modal busy={busy} error={error} open={Boolean(projectDraft)} onClose={() => setProjectDraft(null)} title={projectDraft?.id ? "编辑项目" : "新建项目"} footer={<><Button variant="secondary" onClick={() => setProjectDraft(null)}>取消</Button><Button onClick={() => void saveProject()}>保存项目</Button></>}>
      {projectDraft && <div className="form-grid"><Field label="项目名称" className="span-2"><Input autoFocus value={projectDraft.name} onChange={(event) => setProjectDraft({ ...projectDraft, name: event.currentTarget.value })} /></Field><Field label="领域"><Select value={projectDraft.area} onChange={(event) => setProjectDraft({ ...projectDraft, area: event.currentTarget.value })}><option value="work">工作</option><option value="media">自媒体</option><option value="growth">成长</option><option value="personal">个人</option></Select></Field><Field label="状态"><Select value={projectDraft.status} onChange={(event) => setProjectDraft({ ...projectDraft, status: event.currentTarget.value as Project["status"] })}><option value="active">进行中</option><option value="paused">暂停</option><option value="completed">已完成</option></Select></Field><Field label="颜色"><Input type="color" value={projectDraft.color} onChange={(event) => setProjectDraft({ ...projectDraft, color: event.currentTarget.value })} /></Field><Field label="说明" className="span-2"><Textarea rows={5} value={projectDraft.notes} onChange={(event) => setProjectDraft({ ...projectDraft, notes: event.currentTarget.value })} /></Field></div>}
    </Modal>
    <Modal busy={busy} error={error} open={Boolean(logDraft)} onClose={() => setLogDraft(null)} title={logDraft?.id ? "编辑工作记录" : "记录一段工作"} size="lg" footer={<>{logDraft?.id && <Button variant="secondary" onClick={() => prepareTask(logDraft)}>{logDraft.taskId ? "查看关联任务" : "创建待办任务"}</Button>}<Button variant="secondary" onClick={() => setLogDraft(null)}>取消</Button><Button onClick={() => void saveLog()}>保存记录</Button></>}>
      {logDraft && <div className="form-grid"><Field label="标题" className="span-2"><Input autoFocus value={logDraft.title} onChange={(event) => setLogDraft({ ...logDraft, title: event.currentTarget.value })} /></Field><Field label="日期"><Input type="date" value={logDraft.logDate} onChange={(event) => setLogDraft({ ...logDraft, logDate: event.currentTarget.value })} /></Field><Field label="项目"><Select value={logDraft.projectId ?? ""} onChange={(event) => setLogDraft({ ...logDraft, projectId: event.currentTarget.value || null })}><option value="">不关联项目</option>{projects.map((project) => <option value={project.id} key={project.id}>{project.name}</option>)}</Select></Field><Field label="关联任务" hint="关联不等于完成；删除任务不会删除工作记录。"><Select value={logDraft.taskId ?? ""} onChange={(event) => setLogDraft({ ...logDraft, taskId: event.currentTarget.value || null })}><option value="">不关联任务</option>{tasks.map((task) => <option key={task.id} value={task.id}>{task.title}{task.status === "done" ? "（已完成）" : ""}</option>)}</Select></Field><Field label="耗时（分钟）" hint="默认 0；请填写实际耗时，不会自动计时。"><Input type="number" min="0" step="5" value={logDraft.minutes} onChange={(event) => setLogDraft({ ...logDraft, minutes: Number(event.currentTarget.value) })} /></Field><Field label="精力状态"><Select value={logDraft.energy} onChange={(event) => setLogDraft({ ...logDraft, energy: Number(event.currentTarget.value) })}><option value="1">1 · 很低</option><option value="2">2 · 偏低</option><option value="3">3 · 正常</option><option value="4">4 · 良好</option><option value="5">5 · 专注</option></Select></Field><Field label="完成了什么" className="span-2"><Textarea rows={3} value={logDraft.completed} onChange={(event) => setLogDraft({ ...logDraft, completed: event.currentTarget.value })} /></Field><Field label="遇到的问题"><Textarea rows={3} value={logDraft.blockers} onChange={(event) => setLogDraft({ ...logDraft, blockers: event.currentTarget.value })} /></Field><Field label="下一步"><Textarea rows={3} value={logDraft.nextSteps} onChange={(event) => setLogDraft({ ...logDraft, nextSteps: event.currentTarget.value })} /></Field><Field label="Markdown 笔记" className="span-2"><Textarea rows={7} value={logDraft.markdown} onChange={(event) => setLogDraft({ ...logDraft, markdown: event.currentTarget.value })} placeholder="## 补充记录&#10;- 决策&#10;- 细节&#10;- 相关链接" /></Field><Field label="本机附件" className="span-2"><div className="file-picker"><Input readOnly value={logDraft.attachmentPath ?? ""} placeholder="不复制文件，仅保存路径引用" /><Button type="button" variant="secondary" onClick={() => void chooseAttachment()}>选择文件</Button></div></Field></div>}
    </Modal>
  </div>;
}
