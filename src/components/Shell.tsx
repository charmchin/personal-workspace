import type { ReactNode } from "react";
import {
  CalendarDays,
  ChevronLeft,
  ChevronRight,
  CircleDollarSign,
  FileClock,
  LayoutDashboard,
  LockKeyhole,
  Menu,
  NotebookPen,
  Plus,
  Search,
  Settings,
  Sparkles,
  Sprout,
} from "lucide-react";
import type { PageKey } from "../types";
import { IconButton } from "./ui";

const navigation: Array<{ id: PageKey; label: string; icon: typeof LayoutDashboard }> = [
  { id: "today", label: "今日", icon: LayoutDashboard },
  { id: "schedule", label: "日程", icon: CalendarDays },
  { id: "media", label: "自媒体", icon: Sparkles },
  { id: "portfolio", label: "投资", icon: CircleDollarSign },
  { id: "growth", label: "成长", icon: Sprout },
  { id: "work", label: "工作记录", icon: NotebookPen },
  { id: "review", label: "复盘", icon: FileClock },
  { id: "settings", label: "设置", icon: Settings },
];

export function Shell({ page, setPage, collapsed, setCollapsed, children, onOpenCommand, onQuickAdd, onLock }: {
  page: PageKey;
  setPage: (page: PageKey) => void;
  collapsed: boolean;
  setCollapsed: (value: boolean) => void;
  children: ReactNode;
  onOpenCommand: () => void;
  onQuickAdd: () => void;
  onLock: () => void;
}) {
  const current = navigation.find((item) => item.id === page)!;
  const CurrentIcon = current.icon;
  return (
    <div className={`app-shell ${collapsed ? "sidebar-collapsed" : ""}`}>
      <aside className="sidebar">
        <div className="brand"><div className="brand-mark">台</div>{!collapsed && <div><strong>个人工作台</strong><span>LOCAL · PRIVATE</span></div>}</div>
        <button className="quick-add-sidebar" onClick={onQuickAdd} aria-label="快速记录" title="快速记录"><Plus size={17} />{!collapsed && <span>快速记录</span>}</button>
        <nav aria-label="主导航">
          {navigation.map((item) => {
            const Icon = item.icon;
            return <button key={item.id} className={page === item.id ? "active" : ""} onClick={() => setPage(item.id)} aria-label={item.label} title={item.label} aria-current={page === item.id ? "page" : undefined}><Icon size={18} /><span>{item.label}</span></button>;
          })}
        </nav>
        <div className="sidebar-footer">
          <button onClick={onLock} aria-label="立即锁定" title="立即锁定"><LockKeyhole size={17} /><span>立即锁定</span></button>
          <button onClick={() => setCollapsed(!collapsed)} aria-label="切换侧栏" title="切换侧栏">{collapsed ? <ChevronRight size={17} /> : <ChevronLeft size={17} />}<span>切换侧栏</span></button>
        </div>
      </aside>
      <div className="workspace">
        <header className="topbar">
          <div className="topbar-title"><IconButton label="展开菜单" className="mobile-menu" onClick={() => setCollapsed(!collapsed)}><Menu size={19} /></IconButton><CurrentIcon size={19} /><strong>{current.label}</strong></div>
          <div className="topbar-actions">
            <button className="command-trigger" onClick={onOpenCommand}><Search size={16} /><span>搜索或执行命令</span><kbd>⌘ K</kbd></button>
            <IconButton label="快速记录" onClick={onQuickAdd}><Plus size={19} /></IconButton>
          </div>
        </header>
        <main key={page} className="page-container">{children}</main>
      </div>
    </div>
  );
}
