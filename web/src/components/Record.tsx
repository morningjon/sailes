import { useMemo, useRef, useState, type ReactNode, type RefObject } from "react";
import { Link } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { ChevronLeft, Plus, Sparkles, Trash2 } from "lucide-react";
import { del, get, patch, post, qs, type Activity, type ObjectType, type Task } from "../lib/api";
import { formatDateTime, formatMoney, relativeTime, toDateInput } from "../lib/format";
import { DueLabel, TaskCheck } from "../pages/Tasks";
import { CreateTaskPanel } from "./forms";
import { channelIcon, Pill, useToast } from "./ui";

// Editable properties ------------------------------------------------------------------------------

export type PropDef = {
  key: string;
  label: string;
  type?: "text" | "email" | "tel" | "number" | "money" | "date" | "select" | "textarea" | "readonly";
  options?: { value: string; label: string }[];
  /** How to show the current value when not editing. */
  display?: (v: unknown) => ReactNode;
};

function Prop({ def, value, onSave }: { def: PropDef; value: unknown; onSave: (v: unknown) => Promise<unknown> }) {
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState("");
  const [saving, setSaving] = useState(false);
  const toast = useToast();
  const type = def.type ?? "text";
  const raw = value === null || value === undefined ? "" : String(value);

  const start = () => {
    if (type === "readonly") return;
    setDraft(type === "date" ? toDateInput(raw) : raw);
    setEditing(true);
  };
  const commit = async () => {
    if (draft === (type === "date" ? toDateInput(raw) : raw)) {
      setEditing(false);
      return;
    }
    setSaving(true);
    try {
      const v = draft.trim() === "" ? null : type === "number" || type === "money" ? Number(draft) : draft;
      await onSave(v);
      setEditing(false);
    } catch (e) {
      toast((e as Error).message, true);
    } finally {
      setSaving(false);
    }
  };

  let shown: ReactNode = def.display ? def.display(value) : raw;
  if (!def.display && type === "money" && raw) shown = formatMoney(Number(raw));
  if (!def.display && type === "select") shown = def.options?.find((o) => o.value === raw)?.label ?? raw;
  const empty = shown === "" || shown === null || shown === undefined;

  const inputId = `prop-${def.key}`;
  return (
    <div className="prop">
      <label className="label" htmlFor={inputId}>
        {def.label}
      </label>
      {editing ? (
        type === "select" ? (
          <select
            id={inputId}
            className="select"
            autoFocus
            value={draft}
            disabled={saving}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") setEditing(false);
            }}
          >
            <option value="">None</option>
            {def.options?.map((o) => (
              <option key={o.value} value={o.value}>
                {o.label}
              </option>
            ))}
          </select>
        ) : type === "textarea" ? (
          <textarea
            id={inputId}
            className="textarea"
            autoFocus
            rows={4}
            value={draft}
            disabled={saving}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => e.key === "Escape" && setEditing(false)}
          />
        ) : (
          <input
            id={inputId}
            className="input"
            autoFocus
            type={type === "money" ? "number" : type}
            value={draft}
            disabled={saving}
            onChange={(e) => setDraft(e.target.value)}
            onBlur={commit}
            onKeyDown={(e) => {
              if (e.key === "Enter") commit();
              if (e.key === "Escape") setEditing(false);
            }}
          />
        )
      ) : (
        <div
          className={`value ${empty ? "empty" : ""} ${type === "readonly" ? "readonly" : ""}`}
          role={type === "readonly" ? undefined : "button"}
          tabIndex={type === "readonly" ? undefined : 0}
          onClick={start}
          onKeyDown={(e) => (e.key === "Enter" || e.key === " ") && (e.preventDefault(), start())}
        >
          {empty ? (type === "readonly" ? "None" : "Add") : shown}
        </div>
      )}
    </div>
  );
}

export function PropertyList({
  defs,
  record,
  path,
}: {
  defs: PropDef[];
  record: Record<string, unknown>;
  path: string;
}) {
  const qc = useQueryClient();
  const save = (key: string) => async (v: unknown) => {
    await patch(path, { [key]: v });
    await qc.invalidateQueries();
  };
  return (
    <div className="props">
      <h3>About</h3>
      {defs.map((d) => (
        <Prop key={d.key} def={d} value={record[d.key]} onSave={save(d.key)} />
      ))}
    </div>
  );
}

// Hero ---------------------------------------------------------------------------------------------

export function RecordHero({
  back,
  backLabel,
  avatar,
  title,
  subline,
  actions,
}: {
  back: string;
  backLabel: string;
  avatar: ReactNode;
  title: string;
  subline?: ReactNode;
  actions?: ReactNode;
}) {
  return (
    <div className="record-hero">
      <Link className="back" to={back}>
        <ChevronLeft size={15} /> {backLabel}
      </Link>
      <div>{avatar}</div>
      <h1>{title}</h1>
      {subline && <div className="subline">{subline}</div>}
      {actions && <div className="quick-actions">{actions}</div>}
    </div>
  );
}

export function QuickAction({ icon, label, onClick, href }: { icon: ReactNode; label: string; onClick?: () => void; href?: string }) {
  const inner = (
    <>
      <span className="circle">{icon}</span>
      {label}
    </>
  );
  if (href) {
    return (
      <a href={href} style={{ display: "flex", flexDirection: "column", alignItems: "center", gap: 4, color: "var(--text-2)", fontSize: 12 }}>
        {inner}
      </a>
    );
  }
  return <button onClick={onClick}>{inner}</button>;
}

// Associations ---------------------------------------------------------------------------------------

export function AssocCard({
  title,
  count,
  onAdd,
  addLabel,
  children,
  empty,
}: {
  title: string;
  count?: number;
  onAdd?: () => void;
  addLabel?: string;
  children?: ReactNode;
  empty: string;
}) {
  return (
    <div className="assoc">
      <div className="assoc-head">
        <h3>
          {title}
          {count !== undefined && <span className="faint" style={{ fontWeight: 400 }}> ({count})</span>}
        </h3>
        {onAdd && (
          <button className="btn ghost small" onClick={onAdd} aria-label={addLabel ?? `Add ${title.toLowerCase()}`}>
            <Plus size={14} /> Add
          </button>
        )}
      </div>
      {count === 0 || !children ? <div className="assoc-empty">{empty}</div> : children}
    </div>
  );
}

export function TasksCard({ tasks, defaults }: { tasks: Task[]; defaults: Record<string, string> }) {
  const [creating, setCreating] = useState(false);
  const open = tasks.filter((t) => t.status === "open");
  return (
    <>
      <AssocCard title="Tasks" count={open.length} onAdd={() => setCreating(true)} addLabel="Add task" empty="No open tasks.">
        {open.map((t) => (
          <div className="assoc-item" key={t.id}>
            <TaskCheck task={t} />
            <div style={{ minWidth: 0 }}>
              <div className="t">{t.title}</div>
              <div className="s">
                <DueLabel task={t} />
              </div>
            </div>
          </div>
        ))}
      </AssocCard>
      {creating && <CreateTaskPanel onClose={() => setCreating(false)} defaults={defaults} />}
    </>
  );
}

// Timeline ----------------------------------------------------------------------------------------

const FILTERS = [
  { key: "all", label: "Activity" },
  { key: "note", label: "Notes" },
  { key: "email", label: "Emails" },
  { key: "messages", label: "Messages" },
  { key: "meeting", label: "Meetings" },
  { key: "call", label: "Calls" },
];

function describe(a: Activity): string {
  const who = a.participants?.[0];
  switch (a.kind) {
    case "email":
      return a.direction === "outbound" ? `Email sent${who ? ` to ${who}` : ""}` : `Email${who ? ` from ${who}` : ""}`;
    case "slack":
      return `Slack message${who ? ` from ${who}` : ""}`;
    case "teams":
      return `Teams message${who ? ` from ${who}` : ""}`;
    case "meeting":
      return a.participants?.length ? `Meeting with ${a.participants.join(", ")}` : "Meeting";
    case "call":
      return "Call logged";
    case "note":
      return "Note";
    case "system":
      return a.subject ?? "Update";
  }
}

function Event({ a, onDelete }: { a: Activity; onDelete?: () => void }) {
  const [expanded, setExpanded] = useState(false);
  const long = a.body.length > 280 || a.body.split("\n").length > 4;
  return (
    <div className={`event k-${a.kind}`}>
      <div className="dot">{channelIcon(a.kind, 15)}</div>
      <div className="event-card">
        <div className="event-top">
          <span className="what">{describe(a)}</span>
          {a.created_by === "ai" && (
            <Pill tone="ai">
              <Sparkles size={11} /> Added by sAIles
            </Pill>
          )}
          <span className="when" title={formatDateTime(a.occurred_at)}>
            {relativeTime(a.occurred_at)}
          </span>
          {onDelete && (a.kind === "note" || a.kind === "call" || a.kind === "meeting") && a.created_by !== "connector" && (
            <button className="btn ghost small icon" aria-label="Delete" onClick={onDelete}>
              <Trash2 size={13} />
            </button>
          )}
        </div>
        {a.kind !== "system" && a.subject && <div style={{ fontWeight: 500, marginTop: 4 }}>{a.subject}</div>}
        {a.body && (
          <>
            <div className={`event-body ${long && !expanded ? "clamped" : ""}`}>{a.body}</div>
            {long && (
              <button className="link-btn" style={{ marginTop: 4, fontSize: 13 }} onClick={() => setExpanded((e) => !e)}>
                {expanded ? "Show less" : "Show more"}
              </button>
            )}
          </>
        )}
      </div>
    </div>
  );
}

const monthFmt = new Intl.DateTimeFormat("en-GB", { month: "long", year: "numeric" });

export function Timeline({
  objectType,
  objectId,
  composerRef,
  composerKind,
  setComposerKind,
}: {
  objectType: ObjectType;
  objectId: string;
  composerRef: RefObject<HTMLTextAreaElement | null>;
  composerKind: "note" | "call" | "meeting";
  setComposerKind: (k: "note" | "call" | "meeting") => void;
}) {
  const [filter, setFilter] = useState("all");
  const [body, setBody] = useState("");
  const qc = useQueryClient();
  const toast = useToast();
  const q = useQuery({
    queryKey: ["activities", objectType, objectId],
    queryFn: () => get<Activity[]>(`/activities${qs({ object_type: objectType, object_id: objectId })}`),
  });
  const add = useMutation({
    mutationFn: () =>
      post<Activity>("/activities", { kind: composerKind, body, links: [{ object_type: objectType, object_id: objectId }] }),
    onSuccess: () => {
      setBody("");
      toast(composerKind === "note" ? "Note saved" : composerKind === "call" ? "Call logged" : "Meeting logged");
      qc.invalidateQueries();
    },
    onError: (e: Error) => toast(e.message, true),
  });
  const remove = useMutation({
    mutationFn: (id: string) => del(`/activities/${id}`),
    onSuccess: () => {
      toast("Deleted");
      qc.invalidateQueries();
    },
  });

  const items = useMemo(() => {
    const all = q.data ?? [];
    if (filter === "all") return all;
    if (filter === "messages") return all.filter((a) => a.kind === "slack" || a.kind === "teams");
    return all.filter((a) => a.kind === filter);
  }, [q.data, filter]);

  const groups = useMemo(() => {
    const out: { month: string; items: Activity[] }[] = [];
    for (const a of items) {
      const m = monthFmt.format(new Date(a.occurred_at));
      const last = out[out.length - 1];
      if (last && last.month === m) last.items.push(a);
      else out.push({ month: m, items: [a] });
    }
    return out;
  }, [items]);

  const placeholder =
    composerKind === "note" ? "Write a note" : composerKind === "call" ? "What was the call about?" : "What happened in the meeting?";

  return (
    <>
      <div className="composer">
        <textarea ref={composerRef} aria-label={placeholder} placeholder={placeholder} value={body} onChange={(e) => setBody(e.target.value)} />
        <div className="composer-foot">
          <select
            className="select"
            aria-label="Activity type"
            value={composerKind}
            onChange={(e) => setComposerKind(e.target.value as "note" | "call" | "meeting")}
          >
            <option value="note">Note</option>
            <option value="call">Call</option>
            <option value="meeting">Meeting</option>
          </select>
          <button className="btn primary small" style={{ marginLeft: "auto" }} disabled={!body.trim() || add.isPending} onClick={() => add.mutate()}>
            {composerKind === "note" ? "Save note" : "Log activity"}
          </button>
        </div>
      </div>
      <div className="tabs" role="tablist">
        {FILTERS.map((f) => (
          <button key={f.key} role="tab" aria-selected={filter === f.key} className={filter === f.key ? "active" : ""} onClick={() => setFilter(f.key)}>
            {f.label}
          </button>
        ))}
      </div>
      {q.isLoading && <div className="loading">Loading activity…</div>}
      {!q.isLoading && items.length === 0 && (
        <div className="empty-state" style={{ padding: 24 }}>
          {filter === "all" ? "No activity yet. Emails, messages and meetings appear here once they're linked." : "Nothing of this type yet."}
        </div>
      )}
      {groups.map((g) => (
        <div key={g.month}>
          <div className="timeline-month">{g.month}</div>
          {g.items.map((a) => (
            <Event key={a.id} a={a} onDelete={() => confirm("Delete this activity?") && remove.mutate(a.id)} />
          ))}
        </div>
      ))}
    </>
  );
}

export function useComposer() {
  const ref = useRef<HTMLTextAreaElement>(null);
  const [kind, setKind] = useState<"note" | "call" | "meeting">("note");
  const focus = (k: "note" | "call" | "meeting") => {
    setKind(k);
    setTimeout(() => {
      ref.current?.focus();
      ref.current?.scrollIntoView({ block: "center", behavior: "smooth" });
    }, 0);
  };
  return { ref, kind, setKind, focus };
}
