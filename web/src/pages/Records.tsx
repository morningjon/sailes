import { useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { CircleCheck, Mail, NotebookPen, Phone, Trash2, X } from "lucide-react";
import { del, get, patch, post, type Company, type Contact, type Deal, type Task } from "../lib/api";
import { formatDate, formatDateTime, formatMoney, fullName, LIFECYCLE_STAGES, sourceLabel } from "../lib/format";
import { useCompanyOptions, useContactOptions, usePipelines } from "../lib/hooks";
import { AssocCard, PropertyList, QuickAction, RecordHero, TasksCard, Timeline, useComposer, type PropDef } from "../components/Record";
import { CreateContactPanel, CreateDealPanel, CreateTaskPanel } from "../components/forms";
import { Avatar, Empty, Loading, StagePill, useToast } from "../components/ui";

type Brief = { id: string; name?: string; first_name?: string; last_name?: string; email?: string; job_title?: string; domain?: string; city?: string; industry?: string };
type DealBrief = { id: string; name: string; amount: number | null; currency: string; stage_name: string; is_closed: boolean; is_won: boolean; close_date: string | null };

const LEAD_STATUSES = ["New", "Open", "In progress", "Attempted to contact", "Connected", "Bad timing", "Unqualified"].map((s) => ({ value: s, label: s }));

function DealsList({ deals }: { deals: DealBrief[] }) {
  return (
    <>
      {deals.map((d) => (
        <div className="assoc-item" key={d.id}>
          <div style={{ minWidth: 0 }}>
            <Link className="t" to={`/deals/${d.id}`}>
              {d.name}
            </Link>
            <div className="s num">
              {formatMoney(d.amount, d.currency)}
              {d.close_date ? `, closes ${formatDate(d.close_date)}` : ""}
            </div>
            <div style={{ marginTop: 4 }}>
              <StagePill name={d.stage_name} isClosed={d.is_closed} isWon={d.is_won} />
            </div>
          </div>
        </div>
      ))}
    </>
  );
}

function CompanyCard({ company }: { company: Brief | null }) {
  return (
    <AssocCard title="Company" empty="Not linked to a company. Set one under About.">
      {company && (
        <div className="assoc-item">
          <Avatar name={company.name ?? "?"} square />
          <div style={{ minWidth: 0 }}>
            <Link className="t" to={`/companies/${company.id}`}>
              {company.name}
            </Link>
            <div className="s">{[company.domain, company.city].filter(Boolean).join(", ")}</div>
          </div>
        </div>
      )}
    </AssocCard>
  );
}

function ContactsList({ contacts, onRemove }: { contacts: Brief[]; onRemove?: (id: string) => void }) {
  return (
    <>
      {contacts.map((c) => {
        const name = fullName(c);
        return (
          <div className="assoc-item" key={c.id}>
            <Avatar name={name} />
            <div style={{ minWidth: 0, flex: 1 }}>
              <Link className="t" to={`/contacts/${c.id}`}>
                {name}
              </Link>
              <div className="s">{c.job_title ?? ""}</div>
              {c.email && <div className="s">{c.email}</div>}
            </div>
            {onRemove && (
              <button className="btn ghost small icon" aria-label={`Remove ${name} from this deal`} onClick={() => onRemove(c.id)}>
                <X size={14} />
              </button>
            )}
          </div>
        );
      })}
    </>
  );
}

function DeleteRecord({ path, noun, after }: { path: string; noun: string; after: string }) {
  const navigate = useNavigate();
  const qc = useQueryClient();
  const toast = useToast();
  const m = useMutation({
    mutationFn: () => del(path),
    onSuccess: () => {
      toast(`${noun[0].toUpperCase()}${noun.slice(1)} deleted`);
      qc.invalidateQueries();
      navigate(after);
    },
    onError: (e: Error) => toast(e.message, true),
  });
  return (
    <div style={{ padding: "0 20px 24px" }}>
      <button
        className="btn ghost small danger"
        disabled={m.isPending}
        onClick={() => confirm(`Delete this ${noun}? This can't be undone.`) && m.mutate()}
      >
        <Trash2 size={14} /> Delete {noun}
      </button>
    </div>
  );
}

// Contact -------------------------------------------------------------------------------------------

export function ContactRecord() {
  const { id = "" } = useParams();
  const q = useQuery({
    queryKey: ["contact", id],
    queryFn: () => get<{ record: Contact; company: Brief | null; deals: DealBrief[]; tasks: Task[] }>(`/contacts/${id}`),
  });
  const companies = useCompanyOptions();
  const composer = useComposer();
  const [panel, setPanel] = useState<null | "deal" | "task">(null);

  if (q.isLoading) return <Loading />;
  if (!q.data) return <Empty title="Contact not found">It may have been deleted.</Empty>;
  const { record: c, company, deals, tasks } = q.data;
  const name = fullName(c);

  const defs: PropDef[] = [
    { key: "first_name", label: "First name" },
    { key: "last_name", label: "Last name" },
    { key: "email", label: "Email", type: "email" },
    { key: "phone", label: "Phone number", type: "tel" },
    { key: "job_title", label: "Job title" },
    { key: "company_id", label: "Company", type: "select", options: companies.data ?? [], display: () => c.company_name ?? "" },
    { key: "lifecycle_stage", label: "Lifecycle stage", type: "select", options: LIFECYCLE_STAGES },
    { key: "lead_status", label: "Lead status", type: "select", options: LEAD_STATUSES },
    { key: "owner_name", label: "Owner", type: "readonly" },
    { key: "source", label: "Found via", type: "readonly", display: (v) => sourceLabel(v as string) },
    { key: "created_at", label: "Created", type: "readonly", display: (v) => formatDateTime(v as string) },
  ];

  return (
    <div className="record">
      <section className="record-left">
        <RecordHero
          back="/contacts"
          backLabel="Contacts"
          avatar={<Avatar name={name} size="lg" />}
          title={name}
          subline={[c.job_title, c.company_name].filter(Boolean).join(" at ")}
          actions={
            <>
              <QuickAction icon={<NotebookPen size={16} />} label="Note" onClick={() => composer.focus("note")} />
              {c.email && <QuickAction icon={<Mail size={16} />} label="Email" href={`mailto:${c.email}`} />}
              <QuickAction icon={<Phone size={16} />} label="Call" onClick={() => composer.focus("call")} />
              <QuickAction icon={<CircleCheck size={16} />} label="Task" onClick={() => setPanel("task")} />
            </>
          }
        />
        <PropertyList defs={defs} record={c as unknown as Record<string, unknown>} path={`/contacts/${c.id}`} />
        <DeleteRecord path={`/contacts/${c.id}`} noun="contact" after="/contacts" />
      </section>
      <section className="record-middle">
        <Timeline objectType="contact" objectId={c.id} composerRef={composer.ref} composerKind={composer.kind} setComposerKind={composer.setKind} />
      </section>
      <section className="record-right">
        <CompanyCard company={company} />
        <AssocCard title="Deals" count={deals.length} onAdd={() => setPanel("deal")} addLabel="Add deal" empty="No deals with this contact.">
          <DealsList deals={deals} />
        </AssocCard>
        <TasksCard tasks={tasks} defaults={{ contact_id: c.id, ...(c.company_id ? { company_id: c.company_id } : {}) }} />
      </section>
      {panel === "deal" && (
        <CreateDealPanel
          onClose={() => setPanel(null)}
          defaults={{ contact_ids: [c.id], ...(c.company_id ? { company_id: c.company_id } : {}), name: c.company_name ? `${c.company_name}: ` : "" }}
        />
      )}
      {panel === "task" && <CreateTaskPanel onClose={() => setPanel(null)} defaults={{ contact_id: c.id }} />}
    </div>
  );
}

// Company -------------------------------------------------------------------------------------------

export function CompanyRecord() {
  const { id = "" } = useParams();
  const q = useQuery({
    queryKey: ["company", id],
    queryFn: () => get<{ record: Company; contacts: Brief[]; deals: DealBrief[]; tasks: Task[] }>(`/companies/${id}`),
  });
  const composer = useComposer();
  const [panel, setPanel] = useState<null | "contact" | "deal" | "task">(null);

  if (q.isLoading) return <Loading />;
  if (!q.data) return <Empty title="Company not found">It may have been deleted.</Empty>;
  const { record: co, contacts, deals, tasks } = q.data;

  const defs: PropDef[] = [
    { key: "name", label: "Company name" },
    { key: "domain", label: "Domain" },
    { key: "industry", label: "Industry" },
    { key: "employees", label: "Number of employees", type: "number" },
    { key: "city", label: "City" },
    { key: "country", label: "Country" },
    { key: "phone", label: "Phone number", type: "tel" },
    { key: "lifecycle_stage", label: "Lifecycle stage", type: "select", options: LIFECYCLE_STAGES },
    { key: "description", label: "Description", type: "textarea" },
    { key: "owner_name", label: "Owner", type: "readonly" },
    { key: "source", label: "Found via", type: "readonly", display: (v) => sourceLabel(v as string) },
  ];

  return (
    <div className="record">
      <section className="record-left">
        <RecordHero
          back="/companies"
          backLabel="Companies"
          avatar={<Avatar name={co.name} size="lg" square />}
          title={co.name}
          subline={
            co.domain ? (
              <a href={`https://${co.domain}`} target="_blank" rel="noreferrer">
                {co.domain}
              </a>
            ) : undefined
          }
          actions={
            <>
              <QuickAction icon={<NotebookPen size={16} />} label="Note" onClick={() => composer.focus("note")} />
              <QuickAction icon={<Phone size={16} />} label="Call" onClick={() => composer.focus("call")} />
              <QuickAction icon={<CircleCheck size={16} />} label="Task" onClick={() => setPanel("task")} />
            </>
          }
        />
        <PropertyList defs={defs} record={co as unknown as Record<string, unknown>} path={`/companies/${co.id}`} />
        <DeleteRecord path={`/companies/${co.id}`} noun="company" after="/companies" />
      </section>
      <section className="record-middle">
        <Timeline objectType="company" objectId={co.id} composerRef={composer.ref} composerKind={composer.kind} setComposerKind={composer.setKind} />
      </section>
      <section className="record-right">
        <AssocCard title="Contacts" count={contacts.length} onAdd={() => setPanel("contact")} addLabel="Add contact" empty="No contacts at this company yet.">
          <ContactsList contacts={contacts} />
        </AssocCard>
        <AssocCard title="Deals" count={deals.length} onAdd={() => setPanel("deal")} addLabel="Add deal" empty="No deals with this company.">
          <DealsList deals={deals} />
        </AssocCard>
        <TasksCard tasks={tasks} defaults={{ company_id: co.id }} />
      </section>
      {panel === "contact" && <CreateContactPanel onClose={() => setPanel(null)} defaults={{ company_id: co.id }} />}
      {panel === "deal" && <CreateDealPanel onClose={() => setPanel(null)} defaults={{ company_id: co.id, name: `${co.name}: ` }} />}
      {panel === "task" && <CreateTaskPanel onClose={() => setPanel(null)} defaults={{ company_id: co.id }} />}
    </div>
  );
}

// Deal ------------------------------------------------------------------------------------------------

function StageStepper({ deal }: { deal: Deal }) {
  const pipelines = usePipelines();
  const qc = useQueryClient();
  const toast = useToast();
  const stages = pipelines.data?.find((p) => p.id === deal.pipeline_id)?.stages ?? [];
  const move = useMutation({
    mutationFn: (stage_id: string) => patch<Deal>(`/deals/${deal.id}`, { stage_id }),
    onSuccess: (d) => {
      toast(`Moved to ${d.stage_name}`);
      qc.invalidateQueries();
    },
    onError: (e: Error) => toast(e.message, true),
  });
  const current = stages.findIndex((s) => s.id === deal.stage_id);
  return (
    <div style={{ marginBottom: 20 }}>
      <div style={{ display: "flex", gap: 3 }} role="group" aria-label="Deal stage">
        {stages.map((s, i) => {
          const active = s.id === deal.stage_id;
          const passed = !deal.is_closed && i < current;
          const bg = active ? (s.is_won ? "var(--green)" : s.is_closed ? "var(--red)" : "var(--primary)") : passed ? "var(--primary-soft)" : "var(--surface)";
          return (
            <button
              key={s.id}
              aria-pressed={active}
              disabled={move.isPending}
              onClick={() => !active && move.mutate(s.id)}
              style={{
                flex: 1,
                minWidth: 0,
                padding: "8px 6px",
                border: `1px solid ${active ? "transparent" : "var(--line)"}`,
                background: bg,
                color: active ? "#fff" : passed ? "var(--primary)" : "var(--text-2)",
                fontWeight: 500,
                fontSize: 12,
                cursor: active ? "default" : "pointer",
                borderRadius: i === 0 ? "6px 0 0 6px" : i === stages.length - 1 ? "0 6px 6px 0" : 0,
                overflow: "hidden",
                textOverflow: "ellipsis",
                whiteSpace: "nowrap",
              }}
              title={`${s.name} (${s.probability}%)`}
            >
              {s.name}
            </button>
          );
        })}
      </div>
    </div>
  );
}

export function DealRecord() {
  const { id = "" } = useParams();
  const q = useQuery({
    queryKey: ["deal", id],
    queryFn: () => get<{ record: Deal; company: Brief | null; contacts: Brief[]; tasks: Task[] }>(`/deals/${id}`),
  });
  const pipelines = usePipelines();
  const companies = useCompanyOptions();
  const contactOptions = useContactOptions();
  const composer = useComposer();
  const qc = useQueryClient();
  const toast = useToast();
  const [panel, setPanel] = useState<null | "task">(null);
  const [adding, setAdding] = useState(false);

  const addContact = useMutation({
    mutationFn: (contact_id: string) => post(`/deals/${id}/contacts`, { contact_id }),
    onSuccess: () => {
      setAdding(false);
      qc.invalidateQueries();
    },
    onError: (e: Error) => toast(e.message, true),
  });
  const removeContact = useMutation({
    mutationFn: (contact_id: string) => del(`/deals/${id}/contacts/${contact_id}`),
    onSuccess: () => qc.invalidateQueries(),
  });

  if (q.isLoading) return <Loading />;
  if (!q.data) return <Empty title="Deal not found">It may have been deleted.</Empty>;
  const { record: d, company, contacts, tasks } = q.data;
  const stages = pipelines.data?.find((p) => p.id === d.pipeline_id)?.stages ?? [];

  const defs: PropDef[] = [
    { key: "name", label: "Deal name" },
    { key: "amount", label: "Amount", type: "money", display: (v) => formatMoney(v as number, d.currency) },
    { key: "stage_id", label: "Deal stage", type: "select", options: stages.map((s) => ({ value: s.id, label: s.name })) },
    { key: "close_date", label: "Close date", type: "date", display: (v) => formatDate(v as string) },
    { key: "next_step", label: "Next step" },
    {
      key: "priority",
      label: "Priority",
      type: "select",
      options: [
        { value: "low", label: "Low" },
        { value: "medium", label: "Medium" },
        { value: "high", label: "High" },
      ],
    },
    { key: "company_id", label: "Company", type: "select", options: companies.data ?? [], display: () => d.company_name ?? "" },
    { key: "owner_name", label: "Owner", type: "readonly" },
    { key: "created_at", label: "Created", type: "readonly", display: (v) => formatDateTime(v as string) },
  ];

  const linked = new Set(contacts.map((c) => c.id));

  return (
    <div className="record">
      <section className="record-left">
        <RecordHero
          back="/deals"
          backLabel="Deals"
          avatar={<Avatar name={d.company_name ?? d.name} size="lg" square />}
          title={d.name}
          subline={
            <span className="num">
              {formatMoney(d.amount, d.currency)}
              {d.close_date ? `, closes ${formatDate(d.close_date)}` : ""}
            </span>
          }
          actions={
            <>
              <QuickAction icon={<NotebookPen size={16} />} label="Note" onClick={() => composer.focus("note")} />
              <QuickAction icon={<Phone size={16} />} label="Call" onClick={() => composer.focus("call")} />
              <QuickAction icon={<CircleCheck size={16} />} label="Task" onClick={() => setPanel("task")} />
            </>
          }
        />
        <PropertyList defs={defs} record={d as unknown as Record<string, unknown>} path={`/deals/${d.id}`} />
        <DeleteRecord path={`/deals/${d.id}`} noun="deal" after="/deals" />
      </section>
      <section className="record-middle">
        <StageStepper deal={d} />
        <Timeline objectType="deal" objectId={d.id} composerRef={composer.ref} composerKind={composer.kind} setComposerKind={composer.setKind} />
      </section>
      <section className="record-right">
        <CompanyCard company={company} />
        <AssocCard title="Contacts" count={contacts.length} onAdd={() => setAdding(true)} addLabel="Add contact" empty="No contacts on this deal.">
          <ContactsList contacts={contacts} onRemove={(cid) => removeContact.mutate(cid)} />
        </AssocCard>
        {adding && (
          <div className="assoc" style={{ padding: 12 }}>
            <label className="faint" htmlFor="add-contact" style={{ fontSize: 12 }}>
              Add an existing contact
            </label>
            <select
              id="add-contact"
              className="select"
              autoFocus
              defaultValue=""
              onChange={(e) => e.target.value && addContact.mutate(e.target.value)}
            >
              <option value="">Choose a contact</option>
              {(contactOptions.data ?? [])
                .filter((o) => !linked.has(o.value))
                .map((o) => (
                  <option key={o.value} value={o.value}>
                    {o.label}
                    {o.sub ? ` (${o.sub})` : ""}
                  </option>
                ))}
            </select>
            <button className="btn ghost small" style={{ marginTop: 8 }} onClick={() => setAdding(false)}>
              Cancel
            </button>
          </div>
        )}
        <TasksCard tasks={tasks} defaults={{ deal_id: d.id, ...(d.company_id ? { company_id: d.company_id } : {}) }} />
      </section>
      {panel === "task" && <CreateTaskPanel onClose={() => setPanel(null)} defaults={{ deal_id: d.id }} />}
    </div>
  );
}
