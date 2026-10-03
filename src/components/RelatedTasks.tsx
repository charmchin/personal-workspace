import { useCallback, useEffect, useRef, useState } from "react";
import type { Task } from "../types";
import { call, WorkbenchError } from "../lib/api";
import { emptyTask } from "../lib/defaults";
import { TaskEditor, TaskPriority } from "./TaskEditor";
import { Button, ErrorBanner } from "./ui";

export function RelatedTasks({ kind, id, title }: { kind: "goal" | "content"; id: string; title: string }) {
  const [tasks, setTasks] = useState<Task[]>([]);
  const [draft, setDraft] = useState<Task | null>(null);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const requestVersion = useRef(0);
  const load = useCallback(async () => {
    const version = ++requestVersion.current;
    try { const all = await call<Task[]>("list_tasks"); if (version === requestVersion.current) { setTasks(all.filter((t) => (kind === "goal" ? t.goalId : t.contentId) === id)); setError(null); } }
    catch (value) { if (version === requestVersion.current) setError(value as WorkbenchError); }
  }, [id, kind]);
  useEffect(() => { setTasks([]); setDraft(null); void load(); return () => { requestVersion.current++; }; }, [load]);
  return <section className="related-tasks span-2" aria-label="关联任务">
    <div className="related-tasks-heading"><div><strong>关联任务</strong><small>已完成 {tasks.filter((t) => t.status === "done").length} / {tasks.length} · 不自动更改{kind === "goal" ? "目标进度" : "内容阶段"}</small></div><Button size="sm" variant="secondary" onClick={() => setDraft({ ...emptyTask(), title: `${kind === "goal" ? "推进" : "备稿"}：${title}`.slice(0, 240), goalId: kind === "goal" ? id : null, contentId: kind === "content" ? id : null })}>创建关联任务</Button></div>
    {error && <ErrorBanner message={error.message} recovery={error.recovery} />}
    {tasks.length ? <div className="related-task-list">{tasks.map((t) => <button key={t.id} className={t.status === "done" ? "is-complete" : ""} onClick={() => setDraft(t)}><span className="task-title">{t.title}</span><small>{t.status === "done" ? "已完成" : t.status === "doing" ? "进行中" : "待办"}</small><TaskPriority priority={t.priority} /></button>)}</div> : <p className="muted">暂无关联行动；创建并保存任务后可在今日和日程中跟进。</p>}
    <TaskEditor task={draft} onClose={() => setDraft(null)} onChanged={load} />
  </section>;
}
