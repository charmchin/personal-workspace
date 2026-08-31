import type { ButtonHTMLAttributes, InputHTMLAttributes, ReactElement, ReactNode, SelectHTMLAttributes, TextareaHTMLAttributes } from "react";
import { cloneElement, isValidElement, useEffect, useId, useRef } from "react";
import { AlertCircle, Inbox } from "lucide-react";

export function Button({ variant = "primary", size = "md", className = "", type = "button", ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { variant?: "primary" | "secondary" | "ghost" | "danger"; size?: "sm" | "md" }) {
  return <button type={type} className={`button button-${variant} button-${size} ${className}`} {...props} />;
}

export function IconButton({ label, className = "", children, type = "button", ...props }: ButtonHTMLAttributes<HTMLButtonElement> & { label: string; children: ReactNode }) {
  return <button type={type} className={`icon-button ${className}`} aria-label={label} title={label} {...props}>{children}</button>;
}

export function Card({ className = "", children, ...props }: React.HTMLAttributes<HTMLDivElement>) {
  return <div className={`card ${className}`} {...props}>{children}</div>;
}

export function Badge({ tone = "neutral", children }: { tone?: "neutral" | "green" | "amber" | "red" | "blue"; children: ReactNode }) {
  return <span className={`badge badge-${tone}`}>{children}</span>;
}

export function Field({ label, hint, children, className = "" }: { label: string; hint?: string; children: ReactNode; className?: string }) {
  const controlId = useId();
  const labelId = useId();
  const hintId = useId();
  const isSimpleControl = isValidElement(children) && [Input, Select, Textarea].includes(children.type as typeof Input);
  const control = isValidElement(children)
    ? cloneElement(children as ReactElement<Record<string, unknown>>, isSimpleControl
      ? { id: controlId, "aria-describedby": hint ? hintId : undefined }
      : { "aria-labelledby": labelId, "aria-describedby": hint ? hintId : undefined })
    : children;
  return <div className={`field ${className}`}>{isSimpleControl ? <label id={labelId} htmlFor={controlId} className="field-label">{label}</label> : <span id={labelId} className="field-label">{label}</span>}{control}{hint && <span id={hintId} className="field-hint">{hint}</span>}</div>;
}

export function Input(props: InputHTMLAttributes<HTMLInputElement>) {
  return <input className="input" {...props} />;
}

export function Textarea(props: TextareaHTMLAttributes<HTMLTextAreaElement>) {
  return <textarea className="textarea" {...props} />;
}

export function Select(props: SelectHTMLAttributes<HTMLSelectElement>) {
  return <select className="select" {...props} />;
}

export function Switch({ checked, onChange, label }: { checked: boolean; onChange: (checked: boolean) => void; label: string }) {
  return <button type="button" className={`switch ${checked ? "is-on" : ""}`} role="switch" aria-checked={checked} aria-label={label} onClick={() => onChange(!checked)}><span /></button>;
}

export function Modal({ open, onClose, title, description, children, footer, size = "md" }: { open: boolean; onClose: () => void; title: string; description?: string; children: ReactNode; footer?: ReactNode; size?: "sm" | "md" | "lg" }) {
  const ref = useRef<HTMLDialogElement>(null);
  const titleId = useId();
  const descriptionId = useId();
  useEffect(() => {
    const dialog = ref.current;
    if (!dialog) return;
    if (open && !dialog.open) dialog.showModal();
    if (!open && dialog.open) dialog.close();
  }, [open]);
  return (
    <dialog ref={ref} className={`modal modal-${size}`} aria-labelledby={titleId} aria-describedby={description ? descriptionId : undefined} onCancel={(event) => { event.preventDefault(); onClose(); }} onClick={(event) => { if (event.target === ref.current) onClose(); }}>
      <div className="modal-panel">
        <header className="modal-header"><div><h2 id={titleId}>{title}</h2>{description && <p id={descriptionId}>{description}</p>}</div><IconButton label="关闭" onClick={onClose}>×</IconButton></header>
        <div className="modal-body">{children}</div>
        {footer && <footer className="modal-footer">{footer}</footer>}
      </div>
    </dialog>
  );
}

export function EmptyState({ title, detail, action, compact = false }: { title: string; detail: string; action?: ReactNode; compact?: boolean }) {
  return <div className={`empty-state ${compact ? "compact" : ""}`}><span className="empty-icon"><Inbox size={20} /></span><strong>{title}</strong><p>{detail}</p>{action}</div>;
}

export function ErrorBanner({ message, recovery, onDismiss }: { message: string; recovery?: string | null; onDismiss?: () => void }) {
  return <div className="error-banner" role="alert"><AlertCircle size={18} /><div><strong>{message}</strong>{recovery && <p>{recovery}</p>}</div>{onDismiss && <button onClick={onDismiss} aria-label="关闭错误">×</button>}</div>;
}

export function Progress({ value, label }: { value: number; label?: string }) {
  const bounded = Math.max(0, Math.min(100, value));
  return <div className="progress-wrap" aria-label={label} aria-valuenow={bounded} role="progressbar"><div className="progress-track"><span style={{ width: `${bounded}%` }} /></div>{label && <small>{label}</small>}</div>;
}

export function SectionHeader({ eyebrow, title, description, action }: { eyebrow?: string; title: string; description?: string; action?: ReactNode }) {
  return <div className="section-header"><div>{eyebrow && <span className="eyebrow">{eyebrow}</span>}<h2>{title}</h2>{description && <p>{description}</p>}</div>{action && <div className="section-actions">{action}</div>}</div>;
}

export function LoadingBlock({ rows = 3 }: { rows?: number }) {
  return <div className="loading-block" aria-label="正在加载">{Array.from({ length: rows }, (_, index) => <span key={index} />)}</div>;
}
