import { useEffect, useState } from "react";
import { Command } from "cmdk";
import { CalendarDays, CircleDollarSign, FileClock, LayoutDashboard, NotebookPen, Search, Settings, Sparkles, Sprout } from "lucide-react";
import type { PageKey, SearchResult } from "../types";
import { call } from "../lib/api";

const pages: Array<{ id: PageKey; label: string; icon: typeof Search }> = [
  { id: "today", label: "打开：今日", icon: LayoutDashboard },
  { id: "schedule", label: "打开：日程", icon: CalendarDays },
  { id: "media", label: "打开：自媒体", icon: Sparkles },
  { id: "portfolio", label: "打开：投资", icon: CircleDollarSign },
  { id: "growth", label: "打开：成长", icon: Sprout },
  { id: "work", label: "打开：工作记录", icon: NotebookPen },
  { id: "review", label: "打开：复盘", icon: FileClock },
  { id: "settings", label: "打开：设置", icon: Settings },
];

const resultPage: Record<string, PageKey> = { task: "schedule", calendar: "schedule", project: "work", worklog: "work", goal: "growth", learning: "growth", content: "media", instrument: "portfolio" };

export function CommandPalette({ open, onClose, navigate, quickAdd }: { open: boolean; onClose: () => void; navigate: (page: PageKey) => void; quickAdd: () => void }) {
  const [query, setQuery] = useState("");
  const [results, setResults] = useState<SearchResult[]>([]);
  useEffect(() => {
    if (!open) { setQuery(""); setResults([]); return; }
    const handle = window.setTimeout(() => {
      if (query.trim()) call<SearchResult[]>("search_records", { query }).then(setResults).catch(() => setResults([]));
      else setResults([]);
    }, 160);
    return () => window.clearTimeout(handle);
  }, [open, query]);
  if (!open) return null;
  return (
    <div className="command-overlay" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose(); }}>
      <Command className="command-panel" label="全局命令">
        <div className="command-input"><Search size={18} /><Command.Input autoFocus value={query} onValueChange={setQuery} placeholder="搜索任务、项目、内容或证券…" /></div>
        <Command.List>
          <Command.Empty>没有找到匹配内容</Command.Empty>
          {!query && <Command.Group heading="快捷操作"><Command.Item onSelect={() => { quickAdd(); onClose(); }}>＋ 快速记录</Command.Item></Command.Group>}
          <Command.Group heading="页面">
            {pages.map((page) => { const Icon = page.icon; return <Command.Item key={page.id} value={page.label} onSelect={() => { navigate(page.id); onClose(); }}><Icon size={16} />{page.label}</Command.Item>; })}
          </Command.Group>
          {results.length > 0 && <Command.Group heading="搜索结果">{results.map((result) => <Command.Item key={`${result.kind}-${result.id}`} value={`${result.title} ${result.subtitle}`} onSelect={() => { navigate(resultPage[result.kind] ?? "today"); onClose(); }}><span className="result-kind">{result.kind.slice(0, 1).toUpperCase()}</span><div><strong>{result.title}</strong><small>{result.subtitle}</small></div></Command.Item>)}</Command.Group>}
        </Command.List>
      </Command>
    </div>
  );
}
