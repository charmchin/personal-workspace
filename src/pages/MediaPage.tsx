import { useCallback, useEffect, useState } from "react";
import { open } from "@tauri-apps/plugin-dialog";
import { Archive, BarChart3, ChevronRight, FileText, Paperclip, Plus, Sparkles, Trash2 } from "lucide-react";
import type { ContentItem, ContentMetricInput } from "../types";
import { call, WorkbenchError } from "../lib/api";
import { emptyContent } from "../lib/defaults";
import { localDate, number } from "../lib/format";
import { Badge, Button, Card, ErrorBanner, Field, Input, Modal, SectionHeader, Select, Textarea } from "../components/ui";

const stages: Array<{ id: ContentItem["status"]; label: string; tone: "neutral" | "blue" | "amber" | "green" }> = [
  { id: "idea", label: "灵感", tone: "neutral" }, { id: "planning", label: "待策划", tone: "blue" },
  { id: "creating", label: "制作中", tone: "amber" }, { id: "ready", label: "待发布", tone: "amber" },
  { id: "published", label: "已发布", tone: "green" }, { id: "review", label: "待复盘", tone: "blue" },
  { id: "archived", label: "已归档", tone: "neutral" },
];

export function MediaPage() {
  const [items, setItems] = useState<ContentItem[]>([]);
  const [draft, setDraft] = useState<ContentItem | null>(null);
  const [metricFor, setMetricFor] = useState<ContentItem | null>(null);
  const [metric, setMetric] = useState<ContentMetricInput | null>(null);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const load = useCallback(async () => { try { setItems(await call("list_content_items")); setError(null); } catch (value) { setError(value as WorkbenchError); } }, []);
  useEffect(() => { void load(); }, [load]);

  async function saveItem() { if (!draft) return; try { await call("upsert_content_item", { item: draft }); setDraft(null); await load(); } catch (value) { setError(value as WorkbenchError); } }
  async function move(item: ContentItem) { const index = stages.findIndex((stage) => stage.id === item.status); const next = stages[Math.min(index + 1, stages.length - 1)]?.id ?? "review"; try { await call("upsert_content_item", { item: { ...item, status: next } }); await load(); } catch (value) { setError(value as WorkbenchError); } }
  async function archiveItem(item: ContentItem) { try { await call("upsert_content_item", { item: { ...item, status: "archived" } }); await load(); } catch (value) { setError(value as WorkbenchError); } }
  async function remove(item: ContentItem) { if (!window.confirm(`确认删除“${item.title}”？相关指标也会删除。`)) return; try { await call("delete_record", { kind: "content", id: item.id }); await load(); } catch (value) { setError(value as WorkbenchError); } }
  async function chooseAsset() { const path = await open({ multiple: false }); if (path && draft) setDraft({ ...draft, assetPath: path }); }
  function showMetrics(item: ContentItem) { setMetricFor(item); setMetric({ contentId: item.id, recordedAt: new Date().toISOString(), views: item.views, likes: item.likes, comments: item.comments, saves: item.saves, followersDelta: 0 }); }
  async function saveMetric() { if (!metric) return; try { await call("add_content_metric", { metric }); setMetric(null); setMetricFor(null); await load(); } catch (value) { setError(value as WorkbenchError); } }

  return <div className="page media-page">
    {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
    <div className="page-heading"><div><span className="eyebrow">CREATE · PUBLISH · LEARN</span><h1>自媒体管理</h1><p>让每个想法都有去处，让每次发布都留下可复用的经验。</p></div><Button onClick={() => setDraft(emptyContent())}><Plus size={16} />新增内容</Button></div>
    <div className="media-summary-grid">
      <Card><span>内容总数</span><strong>{items.filter((item) => item.status !== "archived").length}</strong><small>当前内容池</small></Card>
      <Card><span>制作进行中</span><strong>{items.filter((item) => item.status === "creating").length}</strong><small>需要持续推进</small></Card>
      <Card><span>等待发布</span><strong>{items.filter((item) => item.status === "ready").length}</strong><small>检查排期和素材</small></Card>
      <Card><span>累计浏览</span><strong>{number(items.reduce((sum, item) => sum + item.views, 0), 0)}</strong><small>手动指标快照</small></Card>
    </div>
    <section className="module-section media-pipeline"><SectionHeader eyebrow="WORKFLOW" title="内容流程" description="按阶段推进内容；横向滚动可查看完整流程。" /><div className="kanban-scroll"><div className="kanban-board">
      {stages.map((stage) => {
        const stageItems = items.filter((item) => item.status === stage.id);
        return <section className="kanban-column" key={stage.id}><header><div><Badge tone={stage.tone}>{stage.label}</Badge><span>{stageItems.length}</span></div>{stage.id === "idea" && <button onClick={() => setDraft(emptyContent())}><Plus size={16} /></button>}</header><div className="kanban-items">
          {stageItems.map((item) => <Card className="content-card" key={item.id}><div className="content-card-head"><span className="platform-dot" /> <small>{item.platform} · {item.format}</small></div><button className="content-main" onClick={() => setDraft(item)}><h3>{item.title}</h3>{item.goal && <p>{item.goal}</p>}</button>{item.tags.length > 0 && <div className="tag-row">{item.tags.slice(0, 3).map((tag) => <span key={tag}>#{tag}</span>)}</div>}<div className="content-meta">{item.publishAt ? <span>{localDate(item.publishAt, "M月d日 HH:mm")}</span> : <span>未排期</span>}{item.assetPath && <button onClick={() => void call("open_saved_file", { path: item.assetPath! }).catch((value) => setError(value as WorkbenchError))} title="打开素材"><Paperclip size={14} /></button>}</div>{["published", "review"].includes(item.status) && <button className="metric-line" onClick={() => showMetrics(item)}><BarChart3 size={14} /> {number(item.views, 0)} 浏览 · {number(item.likes, 0)} 赞</button>}<footer><button onClick={() => setDraft(item)}><FileText size={14} />编辑</button>{!["review", "archived"].includes(item.status) && <button onClick={() => void move(item)}>推进 <ChevronRight size={14} /></button>}{item.status !== "archived" && <button onClick={() => void archiveItem(item)} aria-label="归档内容"><Archive size={14} /></button>}<button className="danger" onClick={() => void remove(item)} aria-label="删除内容"><Trash2 size={14} /></button></footer></Card>)}
          {stageItems.length === 0 && <div className="kanban-empty"><Sparkles size={18} /><span>暂无内容</span></div>}
        </div></section>;
      })}
    </div></div></section>
    <Modal open={Boolean(draft)} onClose={() => setDraft(null)} title={draft?.id ? "编辑内容" : "记录新灵感"} size="lg" footer={<><Button variant="secondary" onClick={() => setDraft(null)}>取消</Button><Button onClick={() => void saveItem()}>保存内容</Button></>}>
      {draft && <div className="form-grid"><Field label="标题" className="span-2"><Input autoFocus value={draft.title} onChange={(event) => setDraft({ ...draft, title: event.currentTarget.value })} placeholder="这个内容要讲什么？" /></Field><Field label="平台"><Select value={draft.platform} onChange={(event) => setDraft({ ...draft, platform: event.currentTarget.value })}><option>小红书</option><option>微信公众号</option><option>抖音</option><option>B站</option><option>知乎</option><option>视频号</option><option>其他</option></Select></Field><Field label="形式"><Select value={draft.format} onChange={(event) => setDraft({ ...draft, format: event.currentTarget.value })}><option>图文</option><option>短视频</option><option>长视频</option><option>文章</option><option>直播</option><option>播客</option></Select></Field><Field label="当前阶段"><Select value={draft.status} onChange={(event) => setDraft({ ...draft, status: event.currentTarget.value as ContentItem["status"] })}>{stages.map((stage) => <option value={stage.id} key={stage.id}>{stage.label}</option>)}</Select></Field><Field label="发布时间"><Input type="datetime-local" value={draft.publishAt?.slice(0, 16) ?? ""} onChange={(event) => setDraft({ ...draft, publishAt: event.currentTarget.value || null })} /></Field><Field label="内容目标" className="span-2"><Input value={draft.goal} onChange={(event) => setDraft({ ...draft, goal: event.currentTarget.value })} placeholder="希望帮助谁、带来什么结果？" /></Field><Field label="标签" className="span-2"><Input value={draft.tags.join(", ")} onChange={(event) => setDraft({ ...draft, tags: event.currentTarget.value.split(/[,，]/).map((tag) => tag.trim()).filter(Boolean) })} placeholder="效率, 理财, 成长" /></Field><Field label="笔记与脚本" className="span-2"><Textarea rows={8} value={draft.notes} onChange={(event) => setDraft({ ...draft, notes: event.currentTarget.value })} placeholder="结构、要点、标题备选、复盘…" /></Field><Field label="本机素材" className="span-2"><div className="file-picker"><Input readOnly value={draft.assetPath ?? ""} placeholder="不复制文件，仅保存所选路径" /><Button type="button" variant="secondary" onClick={() => void chooseAsset()}>选择文件</Button></div></Field></div>}
    </Modal>
    <Modal open={Boolean(metric)} onClose={() => { setMetric(null); setMetricFor(null); }} title={`记录数据 · ${metricFor?.title ?? ""}`} footer={<><Button variant="secondary" onClick={() => setMetric(null)}>取消</Button><Button onClick={() => void saveMetric()}>保存快照</Button></>}>
      {metric && <div className="form-grid"><Field label="浏览量"><Input type="number" min="0" value={metric.views} onChange={(event) => setMetric({ ...metric, views: Number(event.currentTarget.value) })} /></Field><Field label="点赞"><Input type="number" min="0" value={metric.likes} onChange={(event) => setMetric({ ...metric, likes: Number(event.currentTarget.value) })} /></Field><Field label="评论"><Input type="number" min="0" value={metric.comments} onChange={(event) => setMetric({ ...metric, comments: Number(event.currentTarget.value) })} /></Field><Field label="收藏"><Input type="number" min="0" value={metric.saves} onChange={(event) => setMetric({ ...metric, saves: Number(event.currentTarget.value) })} /></Field><Field label="粉丝变化"><Input type="number" value={metric.followersDelta} onChange={(event) => setMetric({ ...metric, followersDelta: Number(event.currentTarget.value) })} /></Field><Field label="记录时间"><Input type="datetime-local" value={metric.recordedAt.slice(0, 16)} onChange={(event) => { if (event.currentTarget.value) setMetric({ ...metric, recordedAt: new Date(event.currentTarget.value).toISOString() }); }} /></Field></div>}
    </Modal>
  </div>;
}
