import { useEffect, useState } from "react";
import { Trash2 } from "lucide-react";
import type { ContentItem, Goal, Project, Task } from "../types";
import { call, WorkbenchError } from "../lib/api";
import { useMutation } from "../lib/useMutation";
import { shanghaiInput } from "../lib/calendar";
import { Button, Field, Input, Modal, Select, Textarea } from "./ui";

export function TaskPriority({ priority }: { priority: Task["priority"] }) {
  const label = priority === 1 ? "重要" : priority === 2 ? "正常" : "稍后";
  return <span className={`badge badge-${priority === 1 ? "red" : priority === 2 ? "amber" : "neutral"}`} title={`P${priority} · ${label}优先级`} aria-label={`${label}优先级`}>{label}</span>;
}

export function TaskEditor({ task, onClose, onChanged, save, description }: { task: Task | null; onClose: () => void; onChanged: (task?: Task) => void | Promise<void>; save?: (task: Task) => Promise<Task>; description?: string }) {
  const [draft, setDraft] = useState<Task | null>(null);
  const [projects, setProjects] = useState<Project[]>([]);
  const [goals, setGoals] = useState<Goal[]>([]);
  const [contents, setContents] = useState<ContentItem[]>([]);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const [referencesLoaded, setReferencesLoaded] = useState(false);
  const { busy, mutate } = useMutation(setError);
  useEffect(() => {
    setDraft(task); setError(null); setReferencesLoaded(false);
    if (!task) return;
    let active = true;
    Promise.all([call<Project[]>("list_projects"), call<Goal[]>("list_goals"), call<ContentItem[]>("list_content_items")]).then(([p, g, c]) => {
      if (active) { setProjects(p); setGoals(g); setContents(c); setReferencesLoaded(true); }
    }).catch((value) => { if (active) setError(value as WorkbenchError); });
    return () => { active = false; };
  }, [task]);
  async function submit() {
    if (!draft || !referencesLoaded) return;
    if (draft.status === "done" && draft.recurrence !== "none") {
      setError(new WorkbenchError({ code: "VALIDATION_ERROR", message: "重复任务请从任务清单勾选完成，以保留本次历史和下一次任务。" })); return;
    }
    await mutate(async () => {
      const saved = await (save ? save(draft) : call<Task>("upsert_task", { task: draft }));
      onClose(); await onChanged(saved);
    });
  }
  async function remove() {
    if (!task?.id || busy) return;
    const recurrence = task.recurrence === "none" ? "" : "删除此重复任务后，它将不再出现在未来日程中；已经生成的其他任务记录不会被删除。";
    if (!window.confirm(`确认删除任务“${task.title}”？删除后无法恢复。${recurrence}`)) return;
    await mutate(async () => { await call("delete_record", { kind: "task", id: task.id }); onClose(); await onChanged(); });
  }
  const update = (value: Partial<Task>) => setDraft((current) => current ? { ...current, ...value } : current);
  const missingOption = (id: string | null | undefined, records: { id: string }[]) => id && !records.some((record) => record.id === id) ? <option value={id}>原关联（请重新选择或取消）</option> : null;
  return <Modal open={Boolean(task)} onClose={onClose} busy={busy} error={error} title={task?.id ? "编辑任务" : "新建任务"} description={description} size="lg" footer={<>{task?.id && <Button variant="danger" onClick={() => void remove()}><Trash2 size={16} />删除任务</Button>}<Button variant="secondary" onClick={onClose}>取消</Button><Button disabled={!referencesLoaded} onClick={() => void submit()}>保存任务</Button></>}>
    {draft && <div className="form-grid">
      <Field label="任务标题" className="span-2"><Input autoFocus value={draft.title} onChange={(e) => update({ title: e.currentTarget.value })} /></Field>
      <Field label="截止日期"><Input type="date" value={draft.dueDate ?? ""} onChange={(e) => update({ dueDate: e.currentTarget.value || null })} /></Field>
      <Field label="优先级"><Select value={draft.priority} onChange={(e) => update({ priority: Number(e.currentTarget.value) as Task["priority"] })}><option value="1">P1 · 重要</option><option value="2">P2 · 正常</option><option value="3">P3 · 稍后</option></Select></Field>
      <Field label="状态" hint={draft.recurrence !== "none" ? "重复任务请在清单勾选完成，系统会生成下一次。" : undefined}><Select value={draft.status} onChange={(e) => update({ status: e.currentTarget.value as Task["status"] })}><option value="todo">待办</option><option value="doing">进行中</option><option value="done" disabled={draft.recurrence !== "none"}>已完成</option></Select></Field>
      <Field label="重复"><Select value={draft.recurrence} onChange={(e) => update({ recurrence: e.currentTarget.value as Task["recurrence"] })}><option value="none">不重复</option><option value="daily">每天</option><option value="weekly">每周</option><option value="monthly">每月</option></Select></Field>
      <Field label="关联项目"><Select value={draft.projectId ?? ""} disabled={!referencesLoaded} onChange={(e) => update({ projectId: e.currentTarget.value || null })}><option value="">不关联项目</option>{missingOption(draft.projectId, projects)}{projects.map((p) => <option key={p.id} value={p.id}>{p.name}</option>)}</Select></Field>
      <Field label="关联成长目标"><Select value={draft.goalId ?? ""} disabled={!referencesLoaded} onChange={(e) => update({ goalId: e.currentTarget.value || null })}><option value="">不关联目标</option>{missingOption(draft.goalId, goals)}{goals.map((g) => <option key={g.id} value={g.id}>{g.title}</option>)}</Select></Field>
      <Field label="关联自媒体内容" className="span-2" hint="关联用于追踪行动，不自动修改目标进度或内容发布阶段。"><Select value={draft.contentId ?? ""} disabled={!referencesLoaded} onChange={(e) => update({ contentId: e.currentTarget.value || null })}><option value="">不关联内容</option>{missingOption(draft.contentId, contents)}{contents.map((c) => <option key={c.id} value={c.id}>{c.title}</option>)}</Select></Field>
      <Field label="计划开始时间"><Input type="datetime-local" value={shanghaiInput(draft.scheduledStart)} onChange={(e) => update({ scheduledStart: e.currentTarget.value || null })} /></Field>
      <Field label="计划结束时间"><Input type="datetime-local" value={shanghaiInput(draft.scheduledEnd)} onChange={(e) => update({ scheduledEnd: e.currentTarget.value || null })} /></Field>
      <Field label="说明" className="span-2"><Textarea rows={4} value={draft.notes} onChange={(e) => update({ notes: e.currentTarget.value })} /></Field>
    </div>}
  </Modal>;
}
