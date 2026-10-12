import { Fragment, useCallback, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { Link } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import {
  Building2,
  Check,
  CircleCheck,
  Handshake,
  Mail,
  NotebookPen,
  Pencil,
  Sparkles,
  UserPlus,
  UserRoundPen,
  X,
} from "lucide-react";
import { get, post, type Pipeline, type Proposal, type ProposalKind, type ProposalList } from "../lib/api";
import { formatDate, formatDateTime, formatMoney, LIFECYCLE_STAGES, lifecycleLabel, relativeTime, toDateInput } from "../lib/format";
import { useCompanyOptions, usePipelines, useSummary } from "../lib/hooks";
import { channelIcon, channelName, Empty, Loading, Pill, useToast } from "../components/ui";

// What each kind of suggestion looks like ------------------------------------------------------

const KIND_META: Record<ProposalKind, { icon: typeof Mail; label: string; group: string }> = {
  send_email: { icon: Mail, label: "Email draft", group: "emails" },
  create_contact: { icon: UserPlus, label: "New contact", group: "contacts" },
  update_contact: { icon: UserRoundPen, label: "Contact update", group: "contacts" },
  create_company: { icon: Building2, label: "New company", group: "companies" },
  update_company: { icon: Building2, label: "Company update", group: "companies" },
  create_deal: { icon: Handshake, label: "New deal", group: "deals" },
  update_deal: { icon: Handshake, label: "Deal update", group: "deals" },
  add_note: { icon: NotebookPen, label: "Note", group: "notes" },
  create_task: { icon: CircleCheck, label: "Task", group: "tasks" },
};

const GROUPS = [
  { key: "all", label: "All" },
  { key: "emails", label: "Emails" },
  { key: "contacts", label: "Contacts" },
  { key: "companies", label: "Companies" },
  { key: "deals", label: "Deals" },
  { key: "notes", label: "Notes" },
  { key: "tasks", label: "Tasks" },
];

type FieldType =
  | "text"
  | "email"
  | "tel"
  | "number"
  | "money"
  | "date"
  | "datetime"
  | "company"
  | "stage"
  | "lifecycle"
  | "ref"
  | "refs"
  | "textarea"
  | "priority"
  | "taskkind";

const FIELDS: Record<string, { label: string; type: FieldType }> = {
  name: { label: "Name", type: "text" },
  title: { label: "Task", type: "text" },
  first_name: { label: "First name", type: "text" },
  last_name: { label: "Last name", type: "text" },
  email: { label: "Email", type: "email" },
  job_title: { label: "Job title", type: "text" },
  phone: { label: "Phone", type: "tel" },
  company_id: { label: "Company", type: "company" },
  company_domain: { label: "Company", type: "text" },
  company_name: { label: "Company", type: "text" },
  domain: { label: "Domain", type: "text" },
  industry: { label: "Industry", type: "text" },
  employees: { label: "Employees", type: "number" },
  city: { label: "City", type: "text" },
  country: { label: "Country", type: "text" },
  description: { label: "Description", type: "textarea" },
  lifecycle_stage: { label: "Lifecycle stage", type: "lifecycle" },
  lead_status: { label: "Lead status", type: "text" },
  amount: { label: "Amount", type: "money" },
  stage_id: { label: "Deal stage", type: "stage" },
  stage_name: { label: "Deal stage", type: "text" },
  close_date: { label: "Close date", type: "date" },
  contact_ids: { label: "Contacts", type: "refs" },
  contact_id: { label: "Contact", type: "ref" },
  deal_id: { label: "Deal", type: "ref" },
  next_step: { label: "Next step", type: "text" },
  priority: { label: "Priority", type: "priority" },
  kind: { label: "Type", type: "taskkind" },
  due_at: { label: "Due", type: "datetime" },
  notes: { label: "Notes", type: "textarea" },
};

const FIELD_ORDER = Object.keys(FIELDS);
const HIDDEN = new Set(["source", "owner_id", "links", "currency", "pipeline_id"]);

function stageName(id: unknown, pipelines: Pipeline[] | undefined, refs: Record<string, string>) {
  if (typeof id !== "string") return "";
  for (const p of pipelines ?? []) {
    const s = p.stages.find((st) => st.id === id);
    if (s) return s.name;
  }
  return refs[id] ?? id;
}

function display(key: string, value: unknown, p: Proposal, pipelines: Pipeline[] | undefined): string {
  if (value === null || value === undefined || value === "") return "";
  const type = FIELDS[key]?.type ?? "text";
  switch (type) {
    case "money":
      return formatMoney(Number(value), (p.payload.currency as string) || "GBP");
    case "date":
      return formatDate(String(value));
    case "datetime":
      return formatDateTime(String(value));
    case "stage":
      return stageName(value, pipelines, p.refs);
    case "lifecycle":
      return lifecycleLabel(String(value));
    case "company":
    case "ref":
      return p.refs[String(value)] ?? String(value);
    case "refs":
      return Array.isArray(value) ? value.map((v) => p.refs[String(v)] ?? String(v)).join(", ") : String(value);
    case "priority":
    case "taskkind":
      return String(value).replace("_", "-").replace(/^./, (c) => c.toUpperCase());
    case "number":
      return Number(value).toLocaleString("en-GB");
    default:
      return String(value);
  }
}

function orderedKeys(payload: Record<string, unknown>) {
  return Object.keys(payload)
    .filter((k) => !HIDDEN.has(k))
    .sort((a, b) => {
      const ia = FIELD_ORDER.indexOf(a);
      const ib = FIELD_ORDER.indexOf(b);
      return (ia === -1 ? 999 : ia) - (ib === -1 ? 999 : ib);
    });
}

function targetHref(p: Proposal): string | null {
  if (!p.target_type || !p.target_id) return null;
  const base = p.target_type === "company" ? "companies" : `${p.target_type}s`;
  return `/${base}/${p.target_id}`;
}

function resultHref(p: Proposal): string | null {
  if (!p.result_id) return null;
  switch (p.kind) {
    case "create_contact":
    case "update_contact":
      return `/contacts/${p.result_id}`;
    case "create_company":
    case "update_company":
      return `/companies/${p.result_id}`;
    case "create_deal":
    case "update_deal":
      return `/deals/${p.result_id}`;
    case "send_email":
      return "/outbox";
    case "create_task":
      return "/tasks";
    default:
      return targetHref(p);
  }
}

// Editors ---------------------------------------------------------------------------------------

function FieldEditor({
  k,
  value,
  onChange,
  pipelines,
}: {
  k: string;
  value: unknown;
  onChange: (v: unknown) => void;
  pipelines: Pipeline[] | undefined;
}) {
  const type = FIELDS[k]?.type ?? "text";
  const companies = useCompanyOptions();
  const str = value === null || value === undefined ? "" : String(value);
  const label = FIELDS[k]?.label ?? k;
  switch (type) {
    case "stage":
      return (
        <select className="select" aria-label={label} value={str} onChange={(e) => onChange(e.target.value)}>
          {(pipelines ?? []).flatMap((p) =>
            p.stages.map((s) => (
              <option key={s.id} value={s.id}>
                {(pipelines?.length ?? 0) > 1 ? `${p.name}: ` : ""}
                {s.name}
              </option>
            )),
          )}
        </select>
      );
    case "lifecycle":
      return (
        <select className="select" aria-label={label} value={str} onChange={(e) => onChange(e.target.value)}>
          {LIFECYCLE_STAGES.map((s) => (
            <option key={s.value} value={s.value}>
              {s.label}
            </option>
          ))}
        </select>
      );
    case "company":
      return (
        <select className="select" aria-label={label} value={str} onChange={(e) => onChange(e.target.value || null)}>
          <option value="">No company</option>
          {(companies.data ?? []).map((c) => (
            <option key={c.value} value={c.value}>
              {c.label}
            </option>
          ))}
        </select>
      );
    case "priority":
      return (
        <select className="select" aria-label={label} value={str} onChange={(e) => onChange(e.target.value)}>
          {["low", "medium", "high"].map((v) => (
            <option key={v} value={v}>
              {v[0].toUpperCase() + v.slice(1)}
            </option>
          ))}
        </select>
      );
    case "taskkind":
      return (
        <select className="select" aria-label={label} value={str} onChange={(e) => onChange(e.target.value)}>
          <option value="todo">To-do</option>
          <option value="follow_up">Follow-up</option>
          <option value="call">Call</option>
          <option value="email">Email</option>
        </select>
      );
    case "textarea":
      return <textarea className="textarea" aria-label={label} rows={3} value={str} onChange={(e) => onChange(e.target.value)} />;
    case "money":
    case "number":
      return (
        <input
          className="input"
          aria-label={label}
          type="number"
          value={str}
          onChange={(e) => onChange(e.target.value === "" ? null : Number(e.target.value))}
        />
      );
    case "date":
      return <input className="input" aria-label={label} type="date" value={toDateInput(str)} onChange={(e) => onChange(e.target.value)} />;
    case "datetime": {
      const local = str ? new Date(str) : null;
      const v = local ? new Date(local.getTime() - local.getTimezoneOffset() * 60000).toISOString().slice(0, 16) : "";
      return (
        <input
          className="input"
          aria-label={label}
          type="datetime-local"
          value={v}
          onChange={(e) => onChange(e.target.value ? new Date(e.target.value).toISOString() : null)}
        />
      );
    }
    case "ref":
    case "refs":
      return <span className="muted">{Array.isArray(value) ? `${value.length} linked` : "Linked record"}</span>;
    default:
      return (
        <input
          className="input"
          aria-label={label}
          type={type === "email" ? "email" : type === "tel" ? "tel" : "text"}
          value={str}
          onChange={(e) => onChange(e.target.value)}
        />
      );
  }
}

// The card ---------------------------------------------------------------------------------------

function ChangeBlock({
  p,
  payload,
  editing,
  setField,
  pipelines,
}: {
  p: Proposal;
  payload: Record<string, unknown>;
  editing: boolean;
  setField: (k: string, v: unknown) => void;
  pipelines: Pipeline[] | undefined;
}) {
  const meta = KIND_META[p.kind];
  const href = targetHref(p);
  const isUpdate = p.kind.startsWith("update_");
  const keys = orderedKeys(payload);

  let head: ReactNode;
  if (isUpdate || p.kind === "add_note" || p.kind === "create_task") {
    const verb = p.kind === "add_note" ? "Note on" : p.kind === "create_task" ? "Task for" : "Changes to";
    head = (
      <>
        {verb} {href ? <Link to={href}><strong>{p.target_name || "record"}</strong></Link> : <strong>{p.target_name ?? "your records"}</strong>}
      </>
    );
  } else {
    head = <strong>{meta.label}</strong>;
  }

  if (p.kind === "add_note") {
    return (
      <div className="change">
        <div className="change-head">{head}</div>
        {editing ? (
          <textarea
            className="textarea"
            style={{ border: 0, borderRadius: 0, minHeight: 140 }}
            aria-label="Note"
            value={String(payload.body ?? "")}
            onChange={(e) => setField("body", e.target.value)}
          />
        ) : (
          <div className="note-preview">
            {typeof payload.subject === "string" && payload.subject && <div style={{ fontWeight: 600, marginBottom: 4 }}>{payload.subject}</div>}
            {String(payload.body ?? "")}
          </div>
        )}
      </div>
    );
  }

  return (
    <div className="change">
      <div className="change-head">{head}</div>
      <div className="change-rows">
        {keys.map((k) => {
          const newVal = display(k, payload[k], p, pipelines);
          const oldRaw = p.current?.[k];
          const oldVal = isUpdate ? display(k, oldRaw, p, pipelines) : "";
          return (
            <Fragment key={k}>
              <div className="k">{FIELDS[k]?.label ?? k.replace(/_/g, " ")}</div>
              <div className="v">
                {editing ? (
                  <FieldEditor k={k} value={payload[k]} onChange={(v) => setField(k, v)} pipelines={pipelines} />
                ) : (
                  <>
                    {isUpdate && oldVal && oldVal !== newVal && <span className="old">{oldVal}</span>}
                    {isUpdate && !oldVal && <span className="old faint" style={{ textDecoration: "none" }}>Empty</span>}
                    <span className="new">{newVal || "Clear value"}</span>
                  </>
                )}
              </div>
            </Fragment>
          );
        })}
      </div>
    </div>
  );
}

function Letter({
  payload,
  editing,
  setField,
}: {
  payload: Record<string, unknown>;
  editing: boolean;
  setField: (k: string, v: unknown) => void;
}) {
  const to = Array.isArray(payload.to) ? (payload.to as string[]) : [];
  const cc = Array.isArray(payload.cc) ? (payload.cc as string[]) : [];
  const list = (s: string) =>
    s
      .split(/[,;]/)
      .map((x) => x.trim())
      .filter(Boolean);
  return (
    <div className="letter">
      <div className="letter-head">
        <span className="k">To</span>
        {editing ? (
          <input aria-label="To" defaultValue={to.join(", ")} onChange={(e) => setField("to", list(e.target.value))} />
        ) : (
          <span>{to.join(", ")}</span>
        )}
        {(cc.length > 0 || editing) && (
          <>
            <span className="k">Cc</span>
            {editing ? (
              <input aria-label="Cc" defaultValue={cc.join(", ")} onChange={(e) => setField("cc", list(e.target.value))} />
            ) : (
              <span>{cc.join(", ")}</span>
            )}
          </>
        )}
        <span className="k">Subject</span>
        {editing ? (
          <input aria-label="Subject" value={String(payload.subject ?? "")} onChange={(e) => setField("subject", e.target.value)} />
        ) : (
          <span style={{ fontWeight: 500 }}>{String(payload.subject ?? "")}</span>
        )}
      </div>
      {editing ? (
        <textarea className="letter-body" aria-label="Email body" value={String(payload.body ?? "")} onChange={(e) => setField("body", e.target.value)} />
      ) : (
        <div className="letter-body">{String(payload.body ?? "")}</div>
      )}
    </div>
  );
}

function Evidence({ p }: { p: Proposal }) {
  if (!p.source && !p.source_excerpt) return null;
  const kind = p.source?.kind ?? p.source_channel;
  return (
    <details className="evidence">
      <summary>
        <span className="channel">
          {channelIcon(kind, 14)}
          {channelName(kind)}
        </span>
        {p.source_excerpt && <span className="quote">“{p.source_excerpt}”</span>}
      </summary>
      {p.source ? (
        <div className="source-body">
          <div className="faint" style={{ marginBottom: 6 }}>
            {p.source.subject ? `${p.source.subject}, ` : ""}
            {p.source.participants?.join(", ")}, {formatDateTime(p.source.occurred_at)}
          </div>
          {p.source.body}
        </div>
      ) : (
        <div className="source-body faint">The original message isn't stored.</div>
      )}
    </details>
  );
}

type CardActions = { approve: () => void; edit: () => void; reject: () => void };

function ProposalCard({
  p,
  pipelines,
  focused,
  selected,
  onSelect,
  onFocus,
  register,
}: {
  p: Proposal;
  pipelines: Pipeline[] | undefined;
  focused: boolean;
  selected: boolean;
  onSelect: (v: boolean) => void;
  onFocus: () => void;
  register: (id: string, actions: CardActions | null) => void;
}) {
  const qc = useQueryClient();
  const toast = useToast();
  const [editing, setEditing] = useState(false);
  const [draft, setDraft] = useState<Record<string, unknown>>(p.payload);
  const [rejecting, setRejecting] = useState(false);
  const [feedback, setFeedback] = useState("");
  const ref = useRef<HTMLElement>(null);
  const meta = KIND_META[p.kind];
  const Icon = meta.icon;
  const isEmail = p.kind === "send_email";

  useEffect(() => setDraft(p.payload), [p.payload]);
  const mounted = useRef(false);
  useEffect(() => {
    // Follow keyboard focus, but don't jump the page on first load.
    if (mounted.current && focused) ref.current?.scrollIntoView({ block: "nearest", behavior: "smooth" });
    mounted.current = true;
  }, [focused]);

  const approve = useMutation({
    mutationFn: () => post<Proposal>(`/proposals/${p.id}/approve`, editing ? { payload: draft } : {}),
    onSuccess: () => {
      toast(isEmail ? "Approved. It goes to your Outlook drafts on the next run." : `Approved: ${p.title}`);
      qc.invalidateQueries();
    },
    onError: (e: Error) => toast(e.message, true),
  });
  const reject = useMutation({
    mutationFn: () => post<Proposal>(`/proposals/${p.id}/reject`, { feedback }),
    onSuccess: () => {
      toast("Rejected");
      qc.invalidateQueries();
    },
    onError: (e: Error) => toast(e.message, true),
  });

  // Keyboard shortcuts on the Review page call these for the focused card only.
  const approveRef = useRef(approve);
  approveRef.current = approve;
  useEffect(() => {
    register(p.id, {
      approve: () => !approveRef.current.isPending && approveRef.current.mutate(),
      edit: () => setEditing(true),
      reject: () => setRejecting(true),
    });
    return () => register(p.id, null);
  }, [p.id, register]);

  const setField = (k: string, v: unknown) => setDraft((d) => ({ ...d, [k]: v }));
  const busy = approve.isPending || reject.isPending;

  return (
    <article ref={ref} className={`proposal k-${p.kind} ${focused ? "focused" : ""}`} onMouseDown={onFocus} aria-label={p.title}>
      <div className="spine" />
      <div className="proposal-inner">
        <div className="proposal-head">
          <div className="kind-icon">
            <Icon size={17} />
          </div>
          <div className="titles">
            <h3>{p.title}</h3>
            {p.rationale && <div className="why">{p.rationale}</div>}
          </div>
          <div className="meta">
            <span>{relativeTime(p.created_at)}</span>
            {p.confidence !== null && (
              <span className="confidence" title={`The AI is ${Math.round(p.confidence * 100)}% confident in this suggestion`}>
                <span className="bar">
                  <span style={{ width: `${Math.round(p.confidence * 100)}%` }} />
                </span>
                {Math.round(p.confidence * 100)}%
              </span>
            )}
            <input
              type="checkbox"
              className="select-box"
              aria-label={`Select ${p.title}`}
              checked={selected}
              onChange={(e) => onSelect(e.target.checked)}
            />
          </div>
        </div>

        {isEmail ? (
          <Letter payload={draft} editing={editing} setField={setField} />
        ) : (
          <ChangeBlock p={p} payload={editing ? draft : p.payload} editing={editing} setField={setField} pipelines={pipelines} />
        )}

        <Evidence p={p} />

        <div className="proposal-actions">
          {rejecting ? (
            <>
              <input
                className="input feedback"
                autoFocus
                placeholder="Why not? This helps sAIles learn (optional)"
                aria-label="Reason for rejecting"
                value={feedback}
                onChange={(e) => setFeedback(e.target.value)}
                onKeyDown={(e) => {
                  if (e.key === "Enter") reject.mutate();
                  if (e.key === "Escape") setRejecting(false);
                }}
              />
              <button className="btn danger" disabled={busy} onClick={() => reject.mutate()}>
                Reject
              </button>
              <button className="btn ghost" onClick={() => setRejecting(false)}>
                Cancel
              </button>
            </>
          ) : (
            <>
              <button className="btn approve" disabled={busy} onClick={() => approve.mutate()}>
                <Check size={16} />
                {editing ? "Save and approve" : isEmail ? "Approve email" : "Approve"}
              </button>
              {editing ? (
                <button
                  className="btn"
                  onClick={() => {
                    setEditing(false);
                    setDraft(p.payload);
                  }}
                >
                  Discard edits
                </button>
              ) : (
                <button className="btn" onClick={() => setEditing(true)}>
                  <Pencil size={15} /> Edit
                </button>
              )}
              <button className="btn ghost" disabled={busy} onClick={() => setRejecting(true)}>
                <X size={16} /> Reject
              </button>
            </>
          )}
        </div>
      </div>
    </article>
  );
}

// Decided history ---------------------------------------------------------------------------------

function DecidedList() {
  const q = useQuery({ queryKey: ["proposals", "decided"], queryFn: async () => {
    const [a, r] = await Promise.all([
      get<ProposalList>("/proposals?status=approved&limit=50"),
      get<ProposalList>("/proposals?status=rejected&limit=50"),
    ]);
    return [...a.items, ...r.items].sort((x, y) => (y.decided_at ?? "").localeCompare(x.decided_at ?? ""));
  } });
  if (q.isLoading) return <Loading />;
  const items = q.data ?? [];
  if (items.length === 0) return <Empty title="Nothing decided yet">Suggestions you approve or reject show up here.</Empty>;
  return (
    <div className="table-card">
      {items.map((p) => {
        const href = p.status === "approved" ? resultHref(p) : null;
        const Icon = KIND_META[p.kind].icon;
        return (
          <div className="decided-row" key={p.id}>
            <Icon size={16} className="faint" />
            <div style={{ flex: 1, minWidth: 0 }}>
              <div style={{ fontWeight: 500 }}>{href ? <Link to={href}>{p.title}</Link> : p.title}</div>
              {p.feedback && <div className="muted" style={{ fontSize: 13 }}>Your feedback: {p.feedback}</div>}
            </div>
            {p.edited && <Pill tone="ai">Edited</Pill>}
            {p.status === "approved" ? <Pill tone="green">Approved</Pill> : <Pill tone="red">Rejected</Pill>}
            <span className="faint" style={{ fontSize: 12, minWidth: 80, textAlign: "right" }}>
              {relativeTime(p.decided_at)}
            </span>
          </div>
        );
      })}
    </div>
  );
}

// Page ------------------------------------------------------------------------------------------------

export function ReviewPage() {
  const [tab, setTab] = useState<"pending" | "decided">("pending");
  const [group, setGroup] = useState("all");
  const [selected, setSelected] = useState<Set<string>>(new Set());
  const [focus, setFocus] = useState(0);
  const handlers = useRef(new Map<string, CardActions>());
  const register = useCallback((id: string, actions: CardActions | null) => {
    if (actions) handlers.current.set(id, actions);
    else handlers.current.delete(id);
  }, []);
  const qc = useQueryClient();
  const toast = useToast();
  const pipelines = usePipelines();
  const summary = useSummary();

  const q = useQuery({ queryKey: ["proposals", "pending"], queryFn: () => get<ProposalList>("/proposals?status=pending&limit=200") });
  const all = q.data?.items ?? [];
  const items = useMemo(() => (group === "all" ? all : all.filter((p) => KIND_META[p.kind].group === group)), [all, group]);
  const groupCounts = useMemo(() => {
    const c: Record<string, number> = { all: all.length };
    for (const p of all) c[KIND_META[p.kind].group] = (c[KIND_META[p.kind].group] ?? 0) + 1;
    return c;
  }, [all]);

  useEffect(() => {
    setSelected((s) => new Set([...s].filter((id) => all.some((p) => p.id === id))));
    setFocus((f) => Math.min(f, Math.max(0, items.length - 1)));
  }, [all, items.length]);

  useEffect(() => {
    if (tab !== "pending") return;
    const onKey = (e: KeyboardEvent) => {
      const t = e.target as HTMLElement;
      if (t.closest("input, textarea, select, [contenteditable]") || e.metaKey || e.ctrlKey || e.altKey) return;
      if (e.key === "j") setFocus((f) => Math.min(f + 1, items.length - 1));
      else if (e.key === "k") setFocus((f) => Math.max(f - 1, 0));
      else if (e.key === "a" || e.key === "e" || e.key === "r") {
        const current = items[focus];
        const h = current && handlers.current.get(current.id);
        if (!h) return;
        if (e.key === "a") h.approve();
        else if (e.key === "e") h.edit();
        else h.reject();
      } else return;
      e.preventDefault();
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, [tab, items, focus]);

  const bulk = useMutation({
    mutationFn: (action: "approve" | "reject") =>
      post<{ results: { id: string; ok: boolean; error?: string }[] }>("/proposals/bulk", { ids: [...selected], action }),
    onSuccess: (r, action) => {
      const failed = r.results.filter((x) => !x.ok);
      const done = r.results.length - failed.length;
      toast(
        failed.length
          ? `${done} ${action === "approve" ? "approved" : "rejected"}, ${failed.length} need attention: ${failed[0].error}`
          : `${done} ${action === "approve" ? "approved" : "rejected"}`,
        failed.length > 0,
      );
      setSelected(new Set());
      qc.invalidateQueries();
    },
  });

  const s = summary.data;
  const allVisibleSelected = items.length > 0 && items.every((p) => selected.has(p.id));

  return (
    <div className="page">
      <div className="page-head">
        <div>
          <h1>Review</h1>
          <div className="sub">Everything sAIles picked up from your email, Slack and Teams. Nothing changes until you approve it.</div>
        </div>
      </div>

      <div className="stat-strip">
        <div className="stat">
          <div className="label">Waiting for review</div>
          <div className="value">{s?.pending_proposals ?? "–"}</div>
        </div>
        <div className="stat">
          <div className="label">Emails to approve</div>
          <div className="value">{s?.pending_emails ?? "–"}</div>
        </div>
        <div className="stat">
          <div className="label">Tasks due or overdue</div>
          <div className="value">{s ? s.tasks_overdue + s.tasks_today : "–"}</div>
        </div>
        <div className="stat">
          <div className="label">Open pipeline</div>
          <div className="value">{s ? formatMoney(s.open_pipeline_value, "GBP", true) : "–"}</div>
        </div>
      </div>

      <div className="review-layout">
        <div>
          <div className="tabs" role="tablist">
            <button role="tab" aria-selected={tab === "pending"} className={tab === "pending" ? "active" : ""} onClick={() => setTab("pending")}>
              To review <span className="n">{all.length}</span>
            </button>
            <button role="tab" aria-selected={tab === "decided"} className={tab === "decided" ? "active" : ""} onClick={() => setTab("decided")}>
              Decided
            </button>
          </div>

          {tab === "decided" ? (
            <DecidedList />
          ) : (
            <>
              <div className="queue-bar">
                <div className="chips" role="group" aria-label="Filter suggestions">
                  {GROUPS.filter((g) => g.key === "all" || groupCounts[g.key]).map((g) => (
                    <button key={g.key} className={`chip ${group === g.key ? "active" : ""}`} onClick={() => setGroup(g.key)}>
                      {g.label} {groupCounts[g.key] ?? 0}
                    </button>
                  ))}
                </div>
                <div style={{ marginLeft: "auto", display: "flex", gap: 8, alignItems: "center" }}>
                  {selected.size > 0 ? (
                    <>
                      <span className="muted">{selected.size} selected</span>
                      <button className="btn small approve" disabled={bulk.isPending} onClick={() => bulk.mutate("approve")}>
                        Approve selected
                      </button>
                      <button className="btn small" disabled={bulk.isPending} onClick={() => bulk.mutate("reject")}>
                        Reject selected
                      </button>
                    </>
                  ) : (
                    items.length > 1 && (
                      <button className="link-btn" onClick={() => setSelected(new Set(items.map((p) => p.id)))}>
                        Select all
                      </button>
                    )
                  )}
                  {selected.size > 0 && allVisibleSelected && (
                    <button className="link-btn" onClick={() => setSelected(new Set())}>
                      Clear
                    </button>
                  )}
                </div>
              </div>

              {q.isLoading && <Loading label="Loading suggestions" />}
              {q.error && <div className="error-box">{(q.error as Error).message}</div>}
              {!q.isLoading && items.length === 0 && (
                <div className="table-card">
                  <Empty title="You're all caught up">
                    New suggestions arrive after each daily run. You can see when the last one came in under Connections.
                  </Empty>
                </div>
              )}
              {items.map((p, i) => (
                <ProposalCard
                  key={p.id}
                  p={p}
                  pipelines={pipelines.data}
                  focused={i === focus}
                  selected={selected.has(p.id)}
                  onFocus={() => setFocus(i)}
                  onSelect={(v) =>
                    setSelected((s) => {
                      const n = new Set(s);
                      if (v) n.add(p.id);
                      else n.delete(p.id);
                      return n;
                    })
                  }
                  register={register}
                />
              ))}
            </>
          )}
        </div>

        <aside className="review-side">
          <div className="side-card">
            <h3>
              <Sparkles size={15} style={{ verticalAlign: -2, color: "var(--ai)" }} /> This week
            </h3>
            <div className="kv">
              Approved <strong>{s?.approved_this_week ?? 0}</strong>
            </div>
            <div className="kv">
              Emails waiting to send <strong>{s?.outbox_queued ?? 0}</strong>
            </div>
            <div className="kv">
              Weighted pipeline <strong>{s ? formatMoney(s.weighted_pipeline_value, "GBP", true) : "–"}</strong>
            </div>
            <div className="kv">
              Won this month <strong>{s ? formatMoney(s.won_this_month, "GBP", true) : "–"}</strong>
            </div>
          </div>
          <div className="side-card">
            <h3>Keyboard shortcuts</h3>
            <ul className="shortcut-list">
              <li>
                Next and previous <span><kbd>j</kbd> <kbd>k</kbd></span>
              </li>
              <li>
                Approve <kbd>a</kbd>
              </li>
              <li>
                Edit <kbd>e</kbd>
              </li>
              <li>
                Reject <kbd>r</kbd>
              </li>
              <li>
                Search <kbd>/</kbd>
              </li>
            </ul>
          </div>
        </aside>
      </div>
    </div>
  );
}
