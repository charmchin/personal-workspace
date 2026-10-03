import { useCallback, useEffect, useRef, useState } from "react";
import { format } from "date-fns";
import { open, save } from "@tauri-apps/plugin-dialog";
import { getVersion } from "@tauri-apps/api/app";
import { ArchiveRestore, CheckCircle2, Database, FileInput, FileOutput, HardDrive, KeyRound, LockKeyhole, Network, Palette, RefreshCw, RotateCcw, ShieldCheck, Trash2 } from "lucide-react";
import type { AppSettings, BackupInfo, BackupManifest, SecurityStatus } from "../types";
import { useMutation } from "../lib/useMutation";
import { call, WorkbenchError } from "../lib/api";
import { localDate, number } from "../lib/format";
import { BACKUP_PASSWORD_MIN_LENGTH, LOCAL_PASSWORD_MIN_LENGTH, passwordCharacterCount } from "../lib/security";
import { Badge, Button, Card, ErrorBanner, Field, Input, Modal, SectionHeader, Select, Switch } from "../components/ui";

type BackupMode = "export" | "restore";

export function SettingsPage({ settings, onSettingsChange, onLock }: { settings: AppSettings; onSettingsChange: (settings: AppSettings) => void; onLock: (notice?: string) => void }) {
  const [draft, setDraft] = useState(settings);
  const [appVersion, setAppVersion] = useState("读取中…");
  const [token, setToken] = useState("");
  const [hasToken, setHasToken] = useState(false);
  const [keychainMode, setKeychainMode] = useState<SecurityStatus["keychainMode"]>("userPresence");
  const [backups, setBackups] = useState<BackupInfo[]>([]);
  const [backupMode, setBackupMode] = useState<BackupMode | null>(null);
  const [backupPath, setBackupPath] = useState("");
  const [backupPassword, setBackupPassword] = useState("");
  const [working, setWorking] = useState(false);
  const [snapshotWorking, setSnapshotWorking] = useState<string | null>(null);
  const [passwordOpen, setPasswordOpen] = useState(false);
  const [currentPassword, setCurrentPassword] = useState("");
  const [newPassword, setNewPassword] = useState("");
  const [newPasswordConfirm, setNewPasswordConfirm] = useState("");
  const [changingPassword, setChangingPassword] = useState(false);
  const [passwordError, setPasswordError] = useState<WorkbenchError | null>(null);
  const [notice, setNotice] = useState<string | null>(null);
  const [error, setError] = useState<WorkbenchError | null>(null);
  const { busy: savingSettings, mutate: mutateSettings } = useMutation(setError);
  const backupGate = useRef(false);
  const passwordGate = useRef(false);
  const loadMeta = useCallback(async () => { try { const [tokenStatus, backupData, security] = await Promise.all([call<boolean>("has_tushare_token"), call<BackupInfo[]>("list_backups"), call<SecurityStatus>("security_status")]); setHasToken(tokenStatus); setBackups(backupData); setKeychainMode(security.keychainMode); } catch (value) { setError(value as WorkbenchError); } }, []);
  useEffect(() => { setDraft(settings); }, [settings]);
  useEffect(() => { void loadMeta(); }, [loadMeta]);
  useEffect(() => {
    let active = true;
    void getVersion().then((version) => { if (active) setAppVersion(version); }).catch(() => { if (active) setAppVersion("暂不可用"); });
    return () => { active = false; };
  }, []);
  useEffect(() => {
    let active = true;
    const timer = window.setInterval(() => { void call<BackupInfo[]>("list_backups").then((value) => { if (active) setBackups(value); }).catch(() => undefined); }, 15_000);
    return () => { active = false; window.clearInterval(timer); };
  }, []);
  async function saveSettings(next = draft) { if (savingSettings) return; const saved = await mutateSettings(async () => { const updated = await call<AppSettings>("update_settings", { settings: next }); onSettingsChange(updated); setDraft(updated); setNotice("设置已保存"); }); if (!saved) setDraft(settings); }
  async function saveToken() { try { await call("configure_tushare_token", { token }); setToken(""); setHasToken(true); setNotice("Tushare Token 已安全保存到 macOS 钥匙串"); } catch (value) { setError(value as WorkbenchError); } }
  async function chooseExport() { try { const path = await save({ defaultPath: `个人工作台-${format(new Date(), "yyyy-MM-dd")}.workbench-backup`, filters: [{ name: "工作台加密备份", extensions: ["workbench-backup"] }] }); if (path) { setBackupPath(path); setBackupMode("export"); } } catch (value) { setError(new WorkbenchError(value)); } }
  async function chooseRestore() { try { const path = await open({ multiple: false, filters: [{ name: "工作台加密备份", extensions: ["workbench-backup"] }] }); if (path) { setBackupPath(path); setBackupMode("restore"); } } catch (value) { setError(new WorkbenchError(value)); } }
  async function runBackupAction() { if (backupGate.current) return; if (!backupMode) return; if (backupMode === "restore" && !window.confirm("恢复会用备份内容替换当前数据库。系统会先创建恢复前快照，是否继续？")) return; backupGate.current = true; setWorking(true); try { const command = backupMode === "export" ? "export_backup" : "restore_backup"; const manifest = await call<BackupManifest>(command, { path: backupPath, password: backupPassword }); if (backupMode === "restore") onSettingsChange(await call<AppSettings>("get_settings")); setNotice(`${backupMode === "export" ? "备份完成" : "恢复完成"}：${manifest.recordCount} 条记录已校验`); setBackupMode(null); setBackupPassword(""); await loadMeta(); } catch (value) { setError(value as WorkbenchError); } finally { backupGate.current = false; setWorking(false); } }
  async function removeSnapshot(backup: BackupInfo) {
    const kind = backup.kind === "daily" ? "日快照" : backup.kind === "weekly" ? "周快照" : backup.kind === "latest" ? "最近恢复点" : "恢复点";
    if (!window.confirm(`确认删除${kind}“${backup.name}”？删除后无法恢复。`)) return;
    try {
      await call("delete_backup", { name: backup.name });
      setBackups((current) => current.filter((item) => item.name !== backup.name));
      setNotice(`已删除快照：${backup.name}`);
    } catch (value) { setError(value as WorkbenchError); }
  }
  async function restoreSnapshot(backup: BackupInfo) {
    if (!window.confirm(`确认恢复到“${backup.name}”？当前数据库会先生成新的恢复前快照。`)) return;
    setSnapshotWorking(backup.name);
    try {
      const recordCount = await call<number>("restore_snapshot", { name: backup.name });
      onSettingsChange(await call<AppSettings>("get_settings"));
      setNotice(`快照恢复完成：${recordCount} 条记录已校验`);
      await loadMeta();
    } catch (value) { setError(value as WorkbenchError); }
    finally { setSnapshotWorking(null); }
  }
  function closePasswordModal() { if (changingPassword) return; setPasswordOpen(false); setCurrentPassword(""); setNewPassword(""); setNewPasswordConfirm(""); setPasswordError(null); }
  async function changePassword() {
    if (passwordGate.current) return;
    if (newPassword !== newPasswordConfirm) { setPasswordError(new WorkbenchError({ code: "PASSWORD_MISMATCH", message: "两次输入的新口令不一致" })); return; }
    if (currentPassword === newPassword) { setPasswordError(new WorkbenchError({ code: "PASSWORD_UNCHANGED", message: "新工作台口令不能与当前口令相同" })); return; }
    passwordGate.current = true; setChangingPassword(true); setPasswordError(null);
    try {
      await call<SecurityStatus>("security_change_password", { currentPassword, newPassword });
      setPasswordOpen(false); setCurrentPassword(""); setNewPassword(""); setNewPasswordConfirm("");
      onLock("工作台口令已修改，请使用新口令重新解锁。");
    } catch (value) { setPasswordError(value as WorkbenchError); }
    finally { passwordGate.current = false; setChangingPassword(false); }
  }

  return <div className="page settings-page">
    {error && <ErrorBanner message={error.message} recovery={error.recovery} onDismiss={() => setError(null)} />}
    {notice && <div className="notice-banner"><CheckCircle2 size={17} />{notice}<button onClick={() => setNotice(null)}>×</button></div>}
    <div className="page-heading"><div><span className="eyebrow">LOCAL · PRIVATE · YOURS</span><h1>设置</h1><p>控制主题、锁定、联网和备份；默认不向任何服务发送数据。</p></div><Button disabled={savingSettings} onClick={() => void saveSettings()}>保存设置</Button></div>
    <div className="settings-layout">
      <div className="settings-column">
      <section><SectionHeader eyebrow="APPEARANCE" title="外观与隐私" /><Card className="settings-card"><div className="setting-row"><span className="setting-icon"><Palette size={19} /></span><div><strong>界面主题</strong><small>浅色、深色或跟随 macOS</small></div><Select disabled={savingSettings} value={draft.theme} onChange={(event) => { const next = { ...draft, theme: event.currentTarget.value as AppSettings["theme"] }; setDraft(next); void saveSettings(next); }}><option value="system">跟随系统</option><option value="light">浅色</option><option value="dark">深色</option></Select></div><div className="setting-row"><span className="setting-icon"><CircleDollarSignIcon /></span><div><strong>默认隐藏金额</strong><small>首页和投资页面使用隐私遮罩</small></div><Switch disabled={savingSettings} checked={draft.amountsHidden} onChange={(checked) => { const next = { ...draft, amountsHidden: checked }; setDraft(next); void saveSettings(next); }} label="默认隐藏金额" /></div></Card></section>
      <section><SectionHeader eyebrow="SECURITY" title="锁定与本地安全" /><Card className="settings-card"><div className="setting-row"><span className="setting-icon"><LockKeyhole size={19} /></span><div><strong>自动锁定</strong><small>后端检查闲置；锁屏、休眠或会话切换后需重新解锁</small></div><Select value={draft.lockMinutes} onChange={(event) => { const next = { ...draft, lockMinutes: Number(event.currentTarget.value) as AppSettings["lockMinutes"] }; setDraft(next); void saveSettings(next); }}><option value="5">5 分钟</option><option value="15">15 分钟</option><option value="30">30 分钟</option><option value="0">不按闲置锁定</option></Select></div><div className="setting-row"><span className="setting-icon"><KeyRound size={19} /></span><div><strong>数据库密钥</strong><small>{keychainMode === "userPresence" ? "由 macOS 钥匙串保护，读取时要求 Touch ID 或系统密码" : keychainMode === "passphrase" ? "随机数据库密钥使用独立工作台口令加密；口令和裸密钥都不会写入数据库" : "本地未签名构建由当前用户的 macOS 登录钥匙串保护；签名后可启用每次在场认证"}</small></div><Badge tone={keychainMode === "userPresence" ? "green" : "amber"}>{keychainMode === "userPresence" ? "在场认证" : keychainMode === "passphrase" ? "本地口令" : "登录钥匙串"}</Badge></div>{keychainMode === "passphrase" && <div className="setting-row"><span className="setting-icon"><KeyRound size={19} /></span><div><strong>修改工作台口令</strong><small>验证当前口令后重新加密同一把数据库密钥</small></div><Button variant="secondary" size="sm" onClick={() => setPasswordOpen(true)}>修改口令</Button></div>}<div className="setting-row"><span className="setting-icon"><ShieldCheck size={19} /></span><div><strong>立即锁定</strong><small>离开电脑前主动关闭工作台</small></div><Button variant="secondary" size="sm" onClick={() => onLock()}>锁定</Button></div></Card></section>
      </div>
      <div className="settings-column">
      <section><SectionHeader eyebrow="NETWORK" title="可选联网行情" /><Card className="settings-card"><div className="setting-row"><span className="setting-icon"><Network size={19} /></span><div><strong>启用 Tushare 行情</strong><small>只发送证券代码和日期，不发送账户、数量、成本或收益</small></div><Switch checked={draft.quoteEnabled} onChange={(checked) => { const next = { ...draft, quoteEnabled: checked }; setDraft(next); void saveSettings(next); }} label="启用联网行情" /></div><div className="setting-row"><span className="setting-icon"><RefreshCw size={19} /></span><div><strong>每日自动更新一次</strong><small>仅在启用行情且打开应用时运行</small></div><Switch checked={draft.quoteAutoRefresh} onChange={(checked) => { const next = { ...draft, quoteAutoRefresh: checked }; setDraft(next); void saveSettings(next); }} label="自动更新行情" /></div><div className="token-form"><Field label={hasToken ? "更新 Tushare Token" : "配置 Tushare Token"} hint="Token 只写入 macOS 钥匙串，不进入数据库或备份"><div className="file-picker"><Input type="password" autoComplete="off" value={token} onChange={(event) => setToken(event.currentTarget.value)} placeholder={hasToken ? "已配置；输入新 Token 可替换" : "粘贴完整 Token"} /><Button variant="secondary" onClick={() => void saveToken()} disabled={!token}>保存到钥匙串</Button></div></Field></div>{draft.lastQuoteRefresh && <small className="last-sync">最近更新：{localDate(draft.lastQuoteRefresh, "yyyy-MM-dd HH:mm")}</small>}</Card></section>
      <section>
        <SectionHeader eyebrow="BACKUP" title="备份与恢复" description="日快照保留 7 个、周快照保留 4 个、最近恢复点保留 3 个；升级、口令修改和恢复前的恢复点各保留 5 个。" />
        <Card className="settings-card backup-card">
          <div className="backup-actions">
            <button onClick={() => void chooseExport()}><span><FileOutput size={20} /></span><div><strong>导出加密备份</strong><small>生成单一 .workbench-backup 文件</small></div></button>
            <button onClick={() => void chooseRestore()}><span><FileInput size={20} /></span><div><strong>从备份恢复</strong><small>验证通过后才替换当前数据库</small></div></button>
          </div>
          <div className="snapshot-list">
            <div className="snapshot-title"><Database size={16} /><strong>本地快照</strong><span>{backups.length} 个</span></div>
            {backups.length === 0 && <p className="snapshot-empty">还没有本地快照；首次解锁后会自动创建。</p>}
            {backups.length > 0 && <div className="snapshot-items">
              {backups.map((backup) => <div className="snapshot-row" key={backup.name}>
                <span className="snapshot-icon">{backup.kind === "recovery" ? <ArchiveRestore size={15} /> : <HardDrive size={15} />}</span>
                <div><strong title={backup.name}>{backup.name}</strong><small>{localDate(backup.createdAt, "M月d日 HH:mm")} · {number(backup.sizeBytes / 1024 / 1024, 2)} MB</small></div>
                <Badge tone="neutral">{backup.kind === "daily" ? "日" : backup.kind === "weekly" ? "周" : backup.kind === "latest" ? "最近" : "恢复点"}</Badge>
                <button className="snapshot-restore" disabled={snapshotWorking !== null} aria-label={`恢复快照 ${backup.name}`} title="恢复此快照" onClick={() => void restoreSnapshot(backup)}>{snapshotWorking === backup.name ? <RefreshCw className="spin" size={15} /> : <RotateCcw size={15} />}</button>
                <button className="snapshot-delete" disabled={snapshotWorking !== null} aria-label={`删除快照 ${backup.name}`} title="删除快照" onClick={() => void removeSnapshot(backup)}><Trash2 size={15} /></button>
              </div>)}
            </div>}
          </div>
        </Card>
      </section>
      </div>
    </div>
    <Card className="local-data-note"><Database size={22} /><div><strong>所有业务数据都保存在本机</strong><p>没有账号、云同步、遥测或后台服务。启用行情前，应用不会产生外部网络请求。</p></div></Card>
    <Card className="settings-card"><div className="setting-row"><span className="setting-icon"><CheckCircle2 size={19} /></span><div><strong>应用版本：{appVersion}</strong><small>来自当前运行的应用包；源码更新不会自动替换安装版。</small></div></div><div className="setting-row"><span className="setting-icon"><ShieldCheck size={19} /></span><div><strong>开源与第三方许可</strong><small>工作台采用 MIT；第三方组件保留各自条款。完整声明随应用提供，查看不需要联网。</small></div><Button variant="secondary" onClick={() => void call<void>("open_license_notices").catch((value) => setError(value as WorkbenchError))}>查看许可</Button></div></Card>
    <Modal busy={working} error={error} open={Boolean(backupMode)} onClose={() => { setBackupMode(null); setBackupPassword(""); }} title={backupMode === "export" ? "导出加密备份" : "恢复加密备份"} description={backupMode === "restore" ? "恢复前会创建当前数据库快照；密码或完整性检查失败时不会改动数据。" : "请设置一个不同于 Mac 登录密码的独立备份密码。"} footer={<><Button variant="secondary" onClick={() => setBackupMode(null)}>取消</Button><Button variant={backupMode === "restore" ? "danger" : "primary"} onClick={() => void runBackupAction()} disabled={working || passwordCharacterCount(backupPassword) < BACKUP_PASSWORD_MIN_LENGTH}>{working ? "校验中…" : backupMode === "export" ? "加密并导出" : "验证并恢复"}</Button></>}>
      <div className="form-grid one-column"><Field label="文件"><Input readOnly value={backupPath} /></Field><Field label="备份密码" hint="至少 10 个字符；忘记后无法恢复"><Input type="password" autoComplete="new-password" autoFocus value={backupPassword} onChange={(event) => setBackupPassword(event.currentTarget.value)} /></Field></div>
    </Modal>
    <Modal busy={changingPassword} error={passwordError} open={passwordOpen} onClose={closePasswordModal} title="修改工作台口令" description="只重新加密随机数据库密钥，不会改动数据库内容或已有备份密码。修改成功后工作台会立即锁定。" footer={<><Button variant="secondary" onClick={closePasswordModal} disabled={changingPassword}>取消</Button><Button onClick={() => void changePassword()} disabled={changingPassword || passwordCharacterCount(currentPassword) < LOCAL_PASSWORD_MIN_LENGTH || passwordCharacterCount(newPassword) < LOCAL_PASSWORD_MIN_LENGTH || passwordCharacterCount(newPasswordConfirm) < LOCAL_PASSWORD_MIN_LENGTH}>{changingPassword ? "正在安全更换…" : "确认修改并锁定"}</Button></>}>
      <div className="form-grid one-column password-change-form"><Field label="当前工作台口令"><Input type="password" autoComplete="current-password" autoFocus value={currentPassword} onChange={(event) => setCurrentPassword(event.currentTarget.value)} /></Field><Field label="新工作台口令" hint="至少 6 个字符；建议混合字母、数字或符号"><Input type="password" autoComplete="new-password" value={newPassword} onChange={(event) => setNewPassword(event.currentTarget.value)} /></Field><Field label="再次输入新口令"><Input type="password" autoComplete="new-password" value={newPasswordConfirm} onChange={(event) => setNewPasswordConfirm(event.currentTarget.value)} onKeyDown={(event) => { if (event.key === "Enter" && passwordCharacterCount(currentPassword) >= LOCAL_PASSWORD_MIN_LENGTH && passwordCharacterCount(newPassword) >= LOCAL_PASSWORD_MIN_LENGTH && passwordCharacterCount(newPasswordConfirm) >= LOCAL_PASSWORD_MIN_LENGTH) void changePassword(); }} /></Field><p className="password-change-note"><ShieldCheck size={16} />修改前会创建本地加密恢复点；验证或写入失败时继续保留当前口令。</p></div>
    </Modal>
  </div>;
}

function CircleDollarSignIcon() { return <span style={{ fontSize: 17, fontWeight: 700 }}>¥</span>; }
