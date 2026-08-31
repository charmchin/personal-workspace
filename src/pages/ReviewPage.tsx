import { useCallback, useEffect, useMemo, useState } from "react";
import { endOfWeek, format, startOfWeek, subWeeks } from "date-fns";
import { zhCN } from "date-fns/locale";
import { ArrowLeft, ArrowRight, CheckCircle2, FileClock, Plus, Sparkles } from "lucide-react";
import type { ReviewSnapshot } from "../types";
import { call, WorkbenchError } from "../lib/api";
import { emptyReview } from "../lib/defaults";
import { localDate } from "../lib/format";
import { Badge, Button, Card, EmptyState, ErrorBanner, Field, Modal, SectionHeader, Textarea } from "../components/ui";

export function ReviewPage() {
  const [weekOffset, setWeekOffset] = useState(0);
  const [reviews, setReviews] = useState<ReviewSnapshot[]>([]);
  const [draft, setDraft] = useState<ReviewSnapshot | null>(null);
  const [generating, setGenerating] = useState(false);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const range = useMemo(() => { const focus = subWeeks(new Date(), weekOffset); return { start: format(startOfWeek(focus, { weekStartsOn: 1 }), "yyyy-MM-dd"), end: format(endOfWeek(focus, { weekStartsOn: 1 }), "yyyy-MM-dd") }; }, [weekOffset]);
  const load = useCallback(async () => { try { setReviews(await call("list_reviews")); setError(null); } catch (value) { setError(value as WorkbenchError); } }, []);
  useEffect(() => { void load(); }, [load]);
  async function generate() { setGenerating(true); try { const summary = await call<string>("generate_weekly_summary", { startDate: range.start, endDate: range.end }); const existing = reviews.find((review) => review.startDate === range.start && review.endDate === range.end); setDraft({ ...(existing ?? emptyReview(range.start, range.end)), summary }); } catch (value) { setError(value as WorkbenchError); } finally { setGenerating(false); } }
  async function saveReview(confirmed: boolean) { if (!draft) return; try { await call("upsert_review", { review: { ...draft, status: confirmed ? "confirmed" : "draft" } }); setDraft(null); await load(); } catch (value) { setError(value as WorkbenchError); } }
  const current = reviews.find((review) => review.startDate === range.start && review.endDate === range.end);
  return <div className="page review-page">
    {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
    <div className="page-heading"><div><span className="eyebrow">REFLECT · ADJUST · CONTINUE</span><h1>复盘</h1><p>数据负责提示，结论由你确认；把经验留给下一周。</p></div><Button onClick={() => void generate()} disabled={generating}><Sparkles size={16} />{generating ? "汇总中…" : "生成本周摘要"}</Button></div>
    <Card className="review-focus">
      <div className="review-nav"><Button variant="ghost" onClick={() => setWeekOffset((value) => value + 1)}><ArrowLeft size={17} />上一周</Button><div><span>{format(new Date(`${range.start}T12:00:00`), "yyyy年", { locale: zhCN })}</span><h2>{localDate(range.start, "M月d日")} — {localDate(range.end, "M月d日")}</h2></div><Button variant="ghost" onClick={() => setWeekOffset((value) => Math.max(0, value - 1))} disabled={weekOffset === 0}>下一周<ArrowRight size={17} /></Button></div>
      {current ? <div className="review-content"><div className="review-status"><Badge tone={current.status === "confirmed" ? "green" : "amber"}>{current.status === "confirmed" ? "已确认" : "草稿"}</Badge><small>更新于 {localDate(current.updatedAt, "M月d日 HH:mm")}</small></div><div className="review-block"><span>自动摘要</span><p>{current.summary}</p></div><div className="review-block reflection"><span>我的判断</span><p>{current.reflection || "还没有写下这一周的判断。"}</p></div><Button variant="secondary" onClick={() => setDraft(current)}>编辑复盘</Button></div> : <EmptyState compact title="这一周还没有复盘" detail="先生成一份数据摘要，再写下真正值得保留的判断。" action={<Button onClick={() => void generate()}><Plus size={15} />开始复盘</Button>} />}
    </Card>
    <section className="review-history module-section"><SectionHeader title="历史复盘" description="已确认的复盘不会自动改写。" /><div className="review-grid">{reviews.map((review) => <Card key={review.id} className="review-history-card"><button onClick={() => setDraft(review)}><div><span className="review-history-icon">{review.status === "confirmed" ? <CheckCircle2 size={18} /> : <FileClock size={18} />}</span><Badge tone={review.status === "confirmed" ? "green" : "neutral"}>{review.status === "confirmed" ? "已确认" : "草稿"}</Badge></div><h3>{localDate(review.startDate, "M月d日")} — {localDate(review.endDate, "M月d日")}</h3><p>{review.reflection || review.summary}</p></button></Card>)}{reviews.length === 0 && <EmptyState title="历史还是空的" detail="完成第一份周复盘后，它会出现在这里。" />}</div></section>
    <Modal open={Boolean(draft)} onClose={() => setDraft(null)} title="周复盘" description={draft ? `${localDate(draft.startDate, "M月d日")} — ${localDate(draft.endDate, "M月d日")}` : undefined} size="lg" footer={<><Button variant="secondary" onClick={() => void saveReview(false)}>保存草稿</Button><Button onClick={() => void saveReview(true)}>确认复盘</Button></>}>
      {draft && <div className="form-grid one-column"><Field label="数据摘要"><Textarea rows={6} value={draft.summary} onChange={(event) => setDraft({ ...draft, summary: event.currentTarget.value })} /></Field><Field label="我的判断" hint="建议写：本周最有效的动作、需要停止的事、下周唯一重点"><Textarea rows={10} autoFocus value={draft.reflection} onChange={(event) => setDraft({ ...draft, reflection: event.currentTarget.value })} placeholder="这一周真正发生了什么？我学到了什么？下一周要调整什么？" /></Field></div>}
    </Modal>
  </div>;
}
