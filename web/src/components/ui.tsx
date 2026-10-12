import { createContext, useCallback, useContext, useEffect, useRef, useState, type ReactNode } from "react";
import {
  AtSign,
  Calendar,
  CheckCircle2,
  Hash,
  Mail,
  MessageSquare,
  NotebookPen,
  Phone,
  Settings2,
  X,
} from "lucide-react";
import { initials, lifecycleLabel } from "../lib/format";

const AVATAR_COLOURS = ["#6a3fd1", "#2a7a8c", "#b4532a", "#3b6fc4", "#8a3f9e", "#2f8559", "#a8406b", "#5b5bd6", "#7a6a1f"];

function hash(s: string) {
  let h = 0;
  for (let i = 0; i < s.length; i++) h = (h * 31 + s.charCodeAt(i)) | 0;
  return Math.abs(h);
}

export function Avatar({ name, size, square }: { name: string; size?: "lg"; square?: boolean }) {
  const colour = AVATAR_COLOURS[hash(name) % AVATAR_COLOURS.length];
  return (
    <span className={`avatar ${size ?? ""} ${square ? "square" : ""}`} style={{ background: colour }} aria-hidden>
      {initials(name)}
    </span>
  );
}

export function Pill({ tone, children }: { tone?: "purple" | "green" | "red" | "amber" | "blue" | "ai"; children: ReactNode }) {
  return <span className={`pill ${tone ?? ""}`}>{children}</span>;
}

export function LifecyclePill({ stage }: { stage: string }) {
  const tone = stage === "customer" ? "green" : stage === "opportunity" ? "purple" : stage === "lead" ? "blue" : undefined;
  return <Pill tone={tone}>{lifecycleLabel(stage)}</Pill>;
}

export function StagePill({ name, isClosed, isWon }: { name: string; isClosed: boolean; isWon: boolean }) {
  const tone = isWon ? "green" : isClosed ? "red" : "purple";
  return <Pill tone={tone}>{name}</Pill>;
}

export function channelIcon(kind: string | null | undefined, size = 15) {
  switch (kind) {
    case "email":
    case "outlook":
      return <Mail size={size} />;
    case "slack":
      return <Hash size={size} />;
    case "teams":
      return <MessageSquare size={size} />;
    case "meeting":
      return <Calendar size={size} />;
    case "call":
      return <Phone size={size} />;
    case "note":
      return <NotebookPen size={size} />;
    case "system":
      return <Settings2 size={size} />;
    case "task":
      return <CheckCircle2 size={size} />;
    default:
      return <AtSign size={size} />;
  }
}

export function channelName(kind: string | null | undefined) {
  switch (kind) {
    case "email":
    case "outlook":
      return "Outlook";
    case "slack":
      return "Slack";
    case "teams":
      return "Teams";
    case "meeting":
      return "Meeting";
    case "call":
      return "Call";
    case "note":
      return "Note";
    case "system":
      return "Update";
    default:
      return kind ?? "";
  }
}

export function Loading({ label = "Loading" }: { label?: string }) {
  return <div className="loading">{label}…</div>;
}

export function Empty({ title, children, action }: { title: string; children?: ReactNode; action?: ReactNode }) {
  return (
    <div className="empty-state">
      <h2>{title}</h2>
      {children && <div>{children}</div>}
      {action}
    </div>
  );
}

// Slide-over panel -----------------------------------------------------------------------

export function Panel({
  title,
  onClose,
  children,
  footer,
}: {
  title: string;
  onClose: () => void;
  children: ReactNode;
  footer?: ReactNode;
}) {
  const ref = useRef<HTMLDivElement>(null);
  useEffect(() => {
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && onClose();
    window.addEventListener("keydown", onKey);
    ref.current?.querySelector<HTMLElement>("input, textarea, select")?.focus();
    return () => window.removeEventListener("keydown", onKey);
  }, [onClose]);
  return (
    <div className="overlay" onMouseDown={(e) => e.target === e.currentTarget && onClose()}>
      <div className="panel" role="dialog" aria-modal="true" aria-label={title} ref={ref}>
        <div className="panel-head">
          <h2>{title}</h2>
          <button className="btn ghost icon" onClick={onClose} aria-label="Close">
            <X size={18} />
          </button>
        </div>
        <div className="panel-body">{children}</div>
        {footer && <div className="panel-foot">{footer}</div>}
      </div>
    </div>
  );
}

// Toasts ------------------------------------------------------------------------------------

type Toast = { id: number; text: string; error?: boolean };
const ToastCtx = createContext<(text: string, error?: boolean) => void>(() => {});

export function ToastProvider({ children }: { children: ReactNode }) {
  const [toasts, setToasts] = useState<Toast[]>([]);
  const push = useCallback((text: string, error?: boolean) => {
    const id = Date.now() + Math.random();
    setToasts((t) => [...t, { id, text, error }]);
    setTimeout(() => setToasts((t) => t.filter((x) => x.id !== id)), error ? 6000 : 3200);
  }, []);
  return (
    <ToastCtx.Provider value={push}>
      {children}
      <div className="toasts" role="status" aria-live="polite">
        {toasts.map((t) => (
          <div key={t.id} className={`toast ${t.error ? "error" : ""}`}>
            {t.text}
          </div>
        ))}
      </div>
    </ToastCtx.Provider>
  );
}

export function useToast() {
  return useContext(ToastCtx);
}

// Click-away menu ---------------------------------------------------------------------------

export function useClickAway<T extends HTMLElement>(open: boolean, close: () => void) {
  const ref = useRef<T>(null);
  useEffect(() => {
    if (!open) return;
    const onDown = (e: MouseEvent) => {
      if (ref.current && !ref.current.contains(e.target as Node)) close();
    };
    const onKey = (e: KeyboardEvent) => e.key === "Escape" && close();
    document.addEventListener("mousedown", onDown);
    document.addEventListener("keydown", onKey);
    return () => {
      document.removeEventListener("mousedown", onDown);
      document.removeEventListener("keydown", onKey);
    };
  }, [open, close]);
  return ref;
}
