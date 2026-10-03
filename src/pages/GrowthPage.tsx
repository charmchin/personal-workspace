import { useCallback, useEffect, useState } from "react";
import { BookOpen, Check, Flame, GraduationCap, Plus, Sprout, Target, Trash2 } from "lucide-react";
import type { Goal, Habit, LearningItem, SearchResult } from "../types";
import { call, WorkbenchError } from "../lib/api";
import { useSearchTarget } from "../lib/useSearchTarget";
import { RelatedTasks } from "../components/RelatedTasks";
import { useMutation } from "../lib/useMutation";
import { emptyGoal, emptyHabit, emptyLearning } from "../lib/defaults";
import { localDate, percent } from "../lib/format";
import { useLocalDay } from "../lib/useLocalDay";
import { Badge, Button, Card, EmptyState, ErrorBanner, Field, Input, Modal, Progress, SectionHeader, Select, Textarea } from "../components/ui";

export function GrowthPage({ searchTarget }: { searchTarget?: SearchResult | null }) {
  const [goals, setGoals] = useState<Goal[]>([]);
  const [habits, setHabits] = useState<Habit[]>([]);
  const [learning, setLearning] = useState<LearningItem[]>([]);
  const [goalDraft, setGoalDraft] = useState<Goal | null>(null);
  const [habitDraft, setHabitDraft] = useState<Habit | null>(null);
  const [learningDraft, setLearningDraft] = useState<LearningItem | null>(null);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const { busy, mutate } = useMutation(setError);
  const date = useLocalDay();
  const load = useCallback(async () => { try { const [goalData, habitData, learningData] = await Promise.all([call<Goal[]>("list_goals"), call<Habit[]>("list_habits", { date }), call<LearningItem[]>("list_learning_items")]); setGoals(goalData); setHabits(habitData); setLearning(learningData); setError(null); } catch (value) { setError(value as WorkbenchError); } }, [date]);
  useEffect(() => { void load(); }, [load]);
  useSearchTarget(searchTarget, "goal", goals, setGoalDraft);
  useSearchTarget(searchTarget, "habit", habits, setHabitDraft);
  useSearchTarget(searchTarget, "learning", learning, setLearningDraft);
  async function saveGoal() { if (!goalDraft) return; await mutate(async () => { await call("upsert_goal", { goal: goalDraft }); setGoalDraft(null); await load(); }); }
  async function saveHabit() { if (!habitDraft) return; await mutate(async () => { await call("upsert_habit", { habit: habitDraft }); setHabitDraft(null); await load(); }); }
  async function saveLearning() { if (!learningDraft) return; await mutate(async () => { await call("upsert_learning_item", { item: learningDraft }); setLearningDraft(null); await load(); }); }
  async function checkHabit(habit: Habit) { try { await call("check_habit", { habitId: habit.id, date, checked: !habit.checkedToday }); await load(); } catch (value) { setError(value as WorkbenchError); } }
  async function remove(kind: string, id: string, name: string) { if (!window.confirm(`确认删除“${name}”？`)) return; try { await call("delete_record", { kind, id }); await load(); } catch (value) { setError(value as WorkbenchError); } }

  const activeGoals = goals.filter((goal) => goal.status === "active");
  const activeHabits = habits.filter((habit) => habit.active);
  const habitCompletion = activeHabits.filter((habit) => habit.checkedToday).length;
  const habitProgress = activeHabits.length ? (habitCompletion / activeHabits.length) * 100 : 0;
  return <div className="page growth-page">
    {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
    <div className="page-heading"><div><span className="eyebrow">GROW WITH DIRECTION</span><h1>个人成长</h1><p>让目标决定方向，让习惯提供复利，让学习留下真实进展。</p></div><Button onClick={() => setGoalDraft(emptyGoal())}><Plus size={16} />新建目标</Button></div>
    <div className="growth-overview">
      <Card className="growth-hero"><div className="growth-hero-icon"><Sprout size={30} /></div><div><span>今日成长节奏</span><h2>{habitCompletion} / {activeHabits.length} 项习惯已完成</h2><p>持续不是每天都完美，而是每次都愿意回来。</p></div><Progress value={habitProgress} label={percent(habitProgress)} /></Card>
      <Card><span className="card-kicker">活跃目标</span><strong className="overview-number">{activeGoals.length}</strong><small>横跨年 / 季 / 月</small></Card>
      <Card><span className="card-kicker">学习项目</span><strong className="overview-number">{learning.filter((item) => item.status === "learning").length}</strong><small>正在投入</small></Card>
    </div>
    <div className="growth-columns">
      <section className="module-section">
        <SectionHeader eyebrow="GOALS" title="目标与里程碑" action={<Button size="sm" variant="secondary" onClick={() => setGoalDraft(emptyGoal())}><Plus size={14} />目标</Button>} />
        <div className="goal-list">{goals.map((goal) => <Card key={goal.id} className="goal-card"><button className="card-edit-area" onClick={() => setGoalDraft(goal)}><div className="goal-icon"><Target size={18} /></div><div className="goal-content"><div><Badge tone={goal.status === "completed" ? "green" : goal.status === "paused" ? "neutral" : "amber"}>{goal.horizon === "year" ? "年度" : goal.horizon === "quarter" ? "季度" : "月度"}</Badge>{goal.targetDate && <small>目标 {localDate(goal.targetDate, "yyyy.M.d")}</small>}</div><h3>{goal.title}</h3><Progress value={goal.progress} label={`${goal.progress}%`} />{goal.notes && <p>{goal.notes}</p>}</div></button><button className="card-delete" onClick={() => void remove("goal", goal.id, goal.title)}><Trash2 size={14} /></button></Card>)}{goals.length === 0 && <EmptyState title="还没有明确目标" detail="先写下一个本季度真正想推进的结果。" />}</div>
      </section>
      <section className="module-section">
        <SectionHeader eyebrow="HABITS" title="每日习惯" action={<Button size="sm" variant="secondary" onClick={() => setHabitDraft(emptyHabit())}><Plus size={14} />习惯</Button>} />
        <Card className="habit-card-list">{habits.map((habit) => <div className={`habit-row ${habit.checkedToday ? "done" : ""}`} key={habit.id}><button className="habit-check" style={{ borderColor: habit.color, background: habit.checkedToday ? habit.color : undefined }} onClick={() => void checkHabit(habit)}>{habit.checkedToday && <Check size={16} />}</button><button className="habit-info" onClick={() => setHabitDraft(habit)}><strong>{habit.name}</strong><span><Flame size={13} /> 连续 {habit.streak} 天 · 每周 {habit.targetPerWeek} 次</span></button><button className="subtle-delete" onClick={() => void remove("habit", habit.id, habit.name)}><Trash2 size={14} /></button></div>)}{habits.length === 0 && <EmptyState compact title="从一个小习惯开始" detail="例如阅读 20 分钟或每日复盘。" />}</Card>
      </section>
    </div>
    <section className="learning-section module-section"><SectionHeader eyebrow="LEARNING" title="学习项目" description="阅读、课程和实践项目放在同一条进度线上。" action={<Button size="sm" onClick={() => setLearningDraft(emptyLearning())}><Plus size={14} />学习项目</Button>} /><div className="learning-grid">{learning.map((item) => <Card key={item.id} className="learning-card"><button onClick={() => setLearningDraft(item)}><div className="learning-card-top"><span className="learning-icon">{item.kind === "book" ? <BookOpen size={18} /> : <GraduationCap size={18} />}</span><Badge tone={item.status === "completed" ? "green" : item.status === "learning" ? "blue" : "neutral"}>{item.status === "completed" ? "已完成" : item.status === "learning" ? "学习中" : "计划中"}</Badge></div><h3>{item.title}</h3><Progress value={item.progress} /><p>{item.notes || "还没有记录学习笔记"}</p></button><footer><small>{item.targetDate ? `计划 ${localDate(item.targetDate, "M月d日")} 前完成` : "未设置目标日期"}</small><button onClick={() => void remove("learning", item.id, item.title)}><Trash2 size={14} /></button></footer></Card>)}{learning.length === 0 && <EmptyState title="学习清单是空的" detail="建立一个课程、书籍或实践项目。" />}</div></section>

    <Modal busy={busy} error={error} open={Boolean(goalDraft)} onClose={() => setGoalDraft(null)} title={goalDraft?.id ? "编辑目标" : "新建目标"} footer={<><Button variant="secondary" onClick={() => setGoalDraft(null)}>取消</Button><Button onClick={() => void saveGoal()}>保存目标</Button></>}>
      {goalDraft && <div className="form-grid"><Field label="目标标题" className="span-2"><Input autoFocus value={goalDraft.title} onChange={(event) => setGoalDraft({ ...goalDraft, title: event.currentTarget.value })} /></Field><Field label="周期"><Select value={goalDraft.horizon} onChange={(event) => setGoalDraft({ ...goalDraft, horizon: event.currentTarget.value as Goal["horizon"] })}><option value="year">年度</option><option value="quarter">季度</option><option value="month">月度</option></Select></Field><Field label="状态"><Select value={goalDraft.status} onChange={(event) => setGoalDraft({ ...goalDraft, status: event.currentTarget.value as Goal["status"] })}><option value="active">进行中</option><option value="paused">已暂停</option><option value="completed">已完成</option></Select></Field><Field label="开始日期"><Input type="date" value={goalDraft.startDate ?? ""} onChange={(event) => setGoalDraft({ ...goalDraft, startDate: event.currentTarget.value || null })} /></Field><Field label="目标日期"><Input type="date" value={goalDraft.targetDate ?? ""} onChange={(event) => setGoalDraft({ ...goalDraft, targetDate: event.currentTarget.value || null })} /></Field><Field label={`当前进度 · ${goalDraft.progress}%`} className="span-2"><Input type="range" min="0" max="100" value={goalDraft.progress} onChange={(event) => setGoalDraft({ ...goalDraft, progress: Number(event.currentTarget.value) })} /></Field><Field label="目标说明" className="span-2"><Textarea rows={5} value={goalDraft.notes} onChange={(event) => setGoalDraft({ ...goalDraft, notes: event.currentTarget.value })} /></Field>{goalDraft.id && <RelatedTasks kind="goal" id={goalDraft.id} title={goalDraft.title} />}</div>}
    </Modal>
    <Modal busy={busy} error={error} open={Boolean(habitDraft)} onClose={() => setHabitDraft(null)} title={habitDraft?.id ? "编辑习惯" : "新建习惯"} footer={<><Button variant="secondary" onClick={() => setHabitDraft(null)}>取消</Button><Button onClick={() => void saveHabit()}>保存习惯</Button></>}>
      {habitDraft && <div className="form-grid"><Field label="习惯名称" className="span-2"><Input autoFocus value={habitDraft.name} onChange={(event) => setHabitDraft({ ...habitDraft, name: event.currentTarget.value })} /></Field><Field label="频率"><Select value={habitDraft.frequency} onChange={(event) => setHabitDraft({ ...habitDraft, frequency: event.currentTarget.value as Habit["frequency"] })}><option value="daily">每日</option><option value="weekly">每周</option></Select></Field><Field label="每周目标次数"><Input type="number" min="1" max="7" value={habitDraft.targetPerWeek} onChange={(event) => setHabitDraft({ ...habitDraft, targetPerWeek: Number(event.currentTarget.value) })} /></Field><Field label="标识颜色"><Input type="color" value={habitDraft.color} onChange={(event) => setHabitDraft({ ...habitDraft, color: event.currentTarget.value })} /></Field><Field label="状态"><Select value={habitDraft.active ? "active" : "paused"} onChange={(event) => setHabitDraft({ ...habitDraft, active: event.currentTarget.value === "active" })}><option value="active">启用</option><option value="paused">暂停</option></Select></Field></div>}
    </Modal>
    <Modal busy={busy} error={error} open={Boolean(learningDraft)} onClose={() => setLearningDraft(null)} title={learningDraft?.id ? "编辑学习项目" : "新建学习项目"} footer={<><Button variant="secondary" onClick={() => setLearningDraft(null)}>取消</Button><Button onClick={() => void saveLearning()}>保存项目</Button></>}>
      {learningDraft && <div className="form-grid"><Field label="标题" className="span-2"><Input autoFocus value={learningDraft.title} onChange={(event) => setLearningDraft({ ...learningDraft, title: event.currentTarget.value })} /></Field><Field label="类型"><Select value={learningDraft.kind} onChange={(event) => setLearningDraft({ ...learningDraft, kind: event.currentTarget.value as LearningItem["kind"] })}><option value="course">课程</option><option value="book">书籍</option><option value="project">实践项目</option><option value="note">知识笔记</option></Select></Field><Field label="状态"><Select value={learningDraft.status} onChange={(event) => setLearningDraft({ ...learningDraft, status: event.currentTarget.value as LearningItem["status"] })}><option value="planned">计划中</option><option value="learning">学习中</option><option value="completed">已完成</option></Select></Field><Field label="目标日期"><Input type="date" value={learningDraft.targetDate ?? ""} onChange={(event) => setLearningDraft({ ...learningDraft, targetDate: event.currentTarget.value || null })} /></Field><Field label={`进度 · ${learningDraft.progress}%`}><Input type="range" min="0" max="100" value={learningDraft.progress} onChange={(event) => setLearningDraft({ ...learningDraft, progress: Number(event.currentTarget.value) })} /></Field><Field label="学习笔记" className="span-2"><Textarea rows={7} value={learningDraft.notes} onChange={(event) => setLearningDraft({ ...learningDraft, notes: event.currentTarget.value })} /></Field></div>}
    </Modal>
  </div>;
}
