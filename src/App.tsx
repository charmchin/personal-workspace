import { lazy, Suspense, useCallback, useEffect, useRef, useState } from "react";
import { format } from "date-fns";
import { DatabaseZap, Fingerprint, Leaf, LockKeyhole, ShieldCheck } from "lucide-react";
import type { AppSettings, PageKey, SecurityStatus } from "./types";
import { call, WorkbenchError } from "./lib/api";
import { defaultSettings } from "./lib/defaults";
import { localDate } from "./lib/format";
import { LOCAL_PASSWORD_MIN_LENGTH, passwordCharacterCount } from "./lib/security";
import { Button, ErrorBanner, Field, Input, LoadingBlock } from "./components/ui";
import { Shell } from "./components/Shell";
import { CommandPalette } from "./components/CommandPalette";
import { TodayPage } from "./pages/TodayPage";

const SchedulePage = lazy(() => import("./pages/SchedulePage").then((module) => ({ default: module.SchedulePage })));
const MediaPage = lazy(() => import("./pages/MediaPage").then((module) => ({ default: module.MediaPage })));
const PortfolioPage = lazy(() => import("./pages/PortfolioPage").then((module) => ({ default: module.PortfolioPage })));
const GrowthPage = lazy(() => import("./pages/GrowthPage").then((module) => ({ default: module.GrowthPage })));
const WorkPage = lazy(() => import("./pages/WorkPage").then((module) => ({ default: module.WorkPage })));
const ReviewPage = lazy(() => import("./pages/ReviewPage").then((module) => ({ default: module.ReviewPage })));
const SettingsPage = lazy(() => import("./pages/SettingsPage").then((module) => ({ default: module.SettingsPage })));

function App() {
  const [status, setStatus] = useState<SecurityStatus | null>(null);
  const [settings, setSettings] = useState<AppSettings>(defaultSettings);
  const [page, setPage] = useState<PageKey>("today");
  const [sidebarCollapsed, setSidebarCollapsed] = useState(false);
  const [commandOpen, setCommandOpen] = useState(false);
  const [quickOpen, setQuickOpen] = useState(false);
  const [unlocking, setUnlocking] = useState(false);
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [passwordSetup, setPasswordSetup] = useState(false);
  const [localPassword, setLocalPassword] = useState("");
  const [localPasswordConfirm, setLocalPasswordConfirm] = useState("");
  const [error, setError] = useState<WorkbenchError | null>(null);
  const [lockNotice, setLockNotice] = useState<string | null>(null);
  const quoteChecked = useRef(false);

  useEffect(() => {
    call<SecurityStatus>("security_status").then(setStatus).catch((value) => setError(value as WorkbenchError));
  }, []);

  const loadSettings = useCallback(async () => {
    const value = await call<AppSettings>("get_settings");
    setSettings(value);
    return value;
  }, []);

  async function unlock() {
    if (status?.keychainMode === "passphrase") {
      setPasswordSetup(false);
      setPasswordOpen(true);
      return;
    }
    setUnlocking(true);
    setError(null);
    try {
      const next = await call<SecurityStatus>("security_unlock");
      setStatus(next);
      setLockNotice(null);
      await loadSettings();
    } catch (value) {
      const nextError = value as WorkbenchError;
      setError(nextError);
      if (nextError.code === "KEYCHAIN_FALLBACK_REQUIRED") {
        setPasswordSetup(true);
        setPasswordOpen(true);
      }
    } finally {
      setUnlocking(false);
    }
  }

  async function unlockWithPassword() {
    if (passwordSetup && localPassword !== localPasswordConfirm) {
      setError(new WorkbenchError({ code: "PASSWORD_MISMATCH", message: "两次输入的工作台口令不一致" }));
      return;
    }
    setUnlocking(true);
    setError(null);
    try {
      const command = passwordSetup ? "security_initialize_with_password" : "security_unlock_with_password";
      const next = await call<SecurityStatus>(command, { password: localPassword });
      setStatus(next);
      setLockNotice(null);
      setPasswordOpen(false);
      setLocalPassword("");
      setLocalPasswordConfirm("");
      await loadSettings();
    } catch (value) {
      setError(value as WorkbenchError);
    } finally {
      setUnlocking(false);
    }
  }

  const lock = useCallback(async (notice?: string) => {
    try {
      const next = await call<SecurityStatus>("security_lock");
      setStatus(next);
      setLockNotice(notice ?? null);
      setQuickOpen(false);
      setCommandOpen(false);
    } catch (value) {
      setError(value as WorkbenchError);
    }
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    root.dataset.theme = settings.theme;
    if (settings.theme === "system") root.removeAttribute("data-resolved-theme");
    else root.dataset.resolvedTheme = settings.theme;
  }, [settings.theme]);

  useEffect(() => {
    if (!status?.unlocked) return;
    const handler = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setCommandOpen((value) => !value);
      }
      if (event.key === "Escape") setCommandOpen(false);
    };
    const quickHandler: EventListener = () => {
      setPage("today");
      setQuickOpen(true);
    };
    window.addEventListener("keydown", handler);
    document.addEventListener("workbench:quick-add", quickHandler);
    return () => {
      window.removeEventListener("keydown", handler);
      document.removeEventListener("workbench:quick-add", quickHandler);
    };
  }, [status?.unlocked]);

  useEffect(() => {
    if (!status?.unlocked) return;
    let timer: number | undefined;
    let hiddenTimer: number | undefined;
    const reset = () => {
      if (timer) window.clearTimeout(timer);
      if (settings.lockMinutes > 0) {
        timer = window.setTimeout(() => void lock(), settings.lockMinutes * 60_000);
      }
    };
    const visibility = () => {
      if (document.hidden) hiddenTimer = window.setTimeout(() => void lock(), 1_000);
      else {
        if (hiddenTimer) window.clearTimeout(hiddenTimer);
        reset();
      }
    };
    reset();
    const events: Array<keyof WindowEventMap> = ["pointerdown", "keydown", "wheel", "touchstart"];
    events.forEach((event) => window.addEventListener(event, reset, { passive: true }));
    document.addEventListener("visibilitychange", visibility);
    return () => {
      if (timer) window.clearTimeout(timer);
      if (hiddenTimer) window.clearTimeout(hiddenTimer);
      events.forEach((event) => window.removeEventListener(event, reset));
      document.removeEventListener("visibilitychange", visibility);
    };
  }, [lock, settings.lockMinutes, status?.unlocked]);

  useEffect(() => {
    if (!status?.unlocked || !settings.quoteEnabled || !settings.quoteAutoRefresh || quoteChecked.current) return;
    const today = format(new Date(), "yyyy-MM-dd");
    if (settings.lastQuoteRefresh && localDate(settings.lastQuoteRefresh, "yyyy-MM-dd") === today) {
      quoteChecked.current = true;
      return;
    }
    quoteChecked.current = true;
    call("refresh_tushare_quotes").then(() => loadSettings()).catch(() => undefined);
  }, [loadSettings, settings.lastQuoteRefresh, settings.quoteAutoRefresh, settings.quoteEnabled, status?.unlocked]);

  if (!status) {
    return <div className="boot-screen"><div className="boot-mark"><Leaf size={30} /></div><LoadingBlock rows={3} /><span>正在打开本地工作台…</span>{error && <ErrorBanner message={error.message} recovery={error.recovery} />}</div>;
  }

  if (!status.unlocked) {
    return <div className="lock-screen">
      <div className="lock-ambient one" /><div className="lock-ambient two" />
      <section className="lock-panel">
        <div className="lock-brand"><span><Leaf size={25} /></span><div><strong>个人工作台</strong><small>LOCAL · PRIVATE</small></div></div>
        <div className="lock-hero-icon"><Fingerprint size={40} /></div>
        <h1>{status.initialized ? "欢迎回来" : "建立你的本地工作台"}</h1>
        <p>{status.initialized ? (status.keychainMode === "userPresence" ? "使用 Touch ID 或系统密码读取钥匙串密钥。" : status.keychainMode === "passphrase" ? "输入工作台口令以解密本机数据库密钥。" : "从当前用户的 macOS 登录钥匙串读取数据库密钥。") : "首次解锁将生成 256 位密钥，并优先把它存入本机 macOS 钥匙串。"}</p>
        {lockNotice && <div className="notice-banner lock-notice"><ShieldCheck size={17} />{lockNotice}</div>}
        {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
        {passwordOpen ? <div className="password-unlock-form">
          <Field label={passwordSetup ? "设置工作台口令" : "工作台口令"} hint={passwordSetup ? "至少 6 个字符；更长且混合多种字符会更安全" : undefined}><Input type="password" autoComplete={passwordSetup ? "new-password" : "current-password"} autoFocus value={localPassword} onChange={(event) => setLocalPassword(event.currentTarget.value)} onKeyDown={(event) => { if (event.key === "Enter" && passwordCharacterCount(localPassword) >= LOCAL_PASSWORD_MIN_LENGTH && (!passwordSetup || localPassword === localPasswordConfirm)) void unlockWithPassword(); }} /></Field>
          {passwordSetup && <Field label="再次输入"><Input type="password" autoComplete="new-password" value={localPasswordConfirm} onChange={(event) => setLocalPasswordConfirm(event.currentTarget.value)} onKeyDown={(event) => { if (event.key === "Enter" && passwordCharacterCount(localPassword) >= LOCAL_PASSWORD_MIN_LENGTH && localPassword === localPasswordConfirm) void unlockWithPassword(); }} /></Field>}
          <div className="inline-actions"><Button variant="secondary" onClick={() => { setPasswordOpen(false); setError(null); }}>取消</Button><Button onClick={() => void unlockWithPassword()} disabled={unlocking || passwordCharacterCount(localPassword) < LOCAL_PASSWORD_MIN_LENGTH || (passwordSetup && localPassword !== localPasswordConfirm)}>{unlocking ? "正在解密…" : passwordSetup ? "创建并解锁" : "解锁工作台"}</Button></div>
        </div> : <Button className="unlock-button" onClick={() => void unlock()} disabled={unlocking}>{unlocking ? "等待系统认证…" : <><LockKeyhole size={18} />{status.initialized ? "解锁工作台" : "安全初始化并解锁"}</>}</Button>}
        <div className="lock-assurances"><span><ShieldCheck size={15} />SQLCipher 全库加密</span><span><DatabaseZap size={15} />{status.keychainMode === "loginKeychain" ? "登录钥匙串保护" : status.keychainMode === "passphrase" ? "本地口令保护" : "系统在场认证优先"}</span></div>
      </section>
    </div>;
  }

  let content;
  if (page === "today") content = <TodayPage quickOpen={quickOpen} closeQuick={() => setQuickOpen(false)} navigate={setPage} settings={settings} onSettingsChange={setSettings} />;
  else if (page === "schedule") content = <SchedulePage />;
  else if (page === "media") content = <MediaPage />;
  else if (page === "portfolio") content = <PortfolioPage settings={settings} onSettingsChange={setSettings} />;
  else if (page === "growth") content = <GrowthPage />;
  else if (page === "work") content = <WorkPage />;
  else if (page === "review") content = <ReviewPage />;
  else content = <SettingsPage settings={settings} onSettingsChange={setSettings} onLock={(notice) => void lock(notice)} />;

  return <>
    <Shell page={page} setPage={setPage} collapsed={sidebarCollapsed} setCollapsed={setSidebarCollapsed} onOpenCommand={() => setCommandOpen(true)} onQuickAdd={() => { setPage("today"); setQuickOpen(true); }} onLock={() => void lock()}>{status.snapshotWarning && <div className="notice-banner global-warning" role="status"><DatabaseZap size={17} /><span>{status.snapshotWarning}。请检查本机空间和数据目录权限，然后重新打开工作台。</span><button aria-label="关闭提示" onClick={() => setStatus((current) => current ? { ...current, snapshotWarning: null } : current)}>×</button></div>}<Suspense fallback={<div className="page"><LoadingBlock rows={6} /></div>}>{content}</Suspense></Shell>
    <CommandPalette open={commandOpen} onClose={() => setCommandOpen(false)} navigate={setPage} quickAdd={() => { setPage("today"); setQuickOpen(true); }} />
  </>;
}

export default App;
