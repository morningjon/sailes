import { useState, type FormEvent } from "react";
import { useMutation, useQueryClient } from "@tanstack/react-query";
import { post, type Company, type Contact, type Deal, type Task } from "../lib/api";
import { LIFECYCLE_STAGES } from "../lib/format";
import { useCompanyOptions, useContactOptions, usePipelines } from "../lib/hooks";
import { Panel, useToast } from "./ui";

type Values = Record<string, string | string[]>;

function useCreate<T>(path: string, onDone: (r: T) => void, success: string) {
  const qc = useQueryClient();
  const toast = useToast();
  return useMutation({
    mutationFn: (body: unknown) => post<T>(path, body),
    onSuccess: (r) => {
      qc.invalidateQueries();
      toast(success);
      onDone(r);
    },
  });
}

/** Drop empty strings so the server stores nulls, and keep arrays as they are. */
function clean(values: Values) {
  const out: Record<string, unknown> = {};
  for (const [k, v] of Object.entries(values)) {
    if (Array.isArray(v)) out[k] = v;
    else if (v.trim() !== "") out[k] = v.trim();
  }
  return out;
}

function Text({
  label,
  name,
  values,
  set,
  type = "text",
  required,
  placeholder,
}: {
  label: string;
  name: string;
  values: Values;
  set: (k: string, v: string) => void;
  type?: string;
  required?: boolean;
  placeholder?: string;
}) {
  const id = `f-${name}`;
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <input
        id={id}
        className="input"
        type={type}
        value={(values[name] as string) ?? ""}
        required={required}
        placeholder={placeholder}
        onChange={(e) => set(name, e.target.value)}
      />
    </div>
  );
}

function Select({
  label,
  name,
  values,
  set,
  options,
  placeholder,
}: {
  label: string;
  name: string;
  values: Values;
  set: (k: string, v: string) => void;
  options: { value: string; label: string }[];
  placeholder?: string;
}) {
  const id = `f-${name}`;
  return (
    <div className="field">
      <label htmlFor={id}>{label}</label>
      <select id={id} className="select" value={(values[name] as string) ?? ""} onChange={(e) => set(name, e.target.value)}>
        {placeholder !== undefined && <option value="">{placeholder}</option>}
        {options.map((o) => (
          <option key={o.value} value={o.value}>
            {o.label}
          </option>
        ))}
      </select>
    </div>
  );
}

function useValues(initial: Values) {
  const [values, setValues] = useState<Values>(initial);
  const set = (k: string, v: string | string[]) => setValues((s) => ({ ...s, [k]: v }));
  return { values, set };
}

function Footer({ onClose, busy, label }: { onClose: () => void; busy: boolean; label: string }) {
  return (
    <>
      <button className="btn primary" type="submit" form="create-form" disabled={busy}>
        {busy ? "Saving…" : label}
      </button>
      <button className="btn" type="button" onClick={onClose}>
        Cancel
      </button>
    </>
  );
}

export function CreateContactPanel({
  onClose,
  onCreated,
  defaults = {},
}: {
  onClose: () => void;
  onCreated?: (c: Contact) => void;
  defaults?: Values;
}) {
  const { values, set } = useValues({ lifecycle_stage: "lead", ...defaults });
  const companies = useCompanyOptions();
  const m = useCreate<Contact>("/contacts", (c) => (onCreated ?? (() => {}))(c), "Contact created");
  const submit = (e: FormEvent) => {
    e.preventDefault();
    m.mutate(clean(values), { onSuccess: onClose });
  };
  return (
    <Panel title="Create contact" onClose={onClose} footer={<Footer onClose={onClose} busy={m.isPending} label="Create contact" />}>
      <form id="create-form" onSubmit={submit}>
        {m.error && <div className="error-box">{m.error.message}</div>}
        <Text label="Email" name="email" type="email" values={values} set={set} />
        <Text label="First name" name="first_name" values={values} set={set} />
        <Text label="Last name" name="last_name" values={values} set={set} />
        <Text label="Job title" name="job_title" values={values} set={set} />
        <Text label="Phone number" name="phone" type="tel" values={values} set={set} />
        <Select label="Company" name="company_id" values={values} set={set} options={companies.data ?? []} placeholder="No company" />
        <Select label="Lifecycle stage" name="lifecycle_stage" values={values} set={set} options={LIFECYCLE_STAGES} />
      </form>
    </Panel>
  );
}

export function CreateCompanyPanel({ onClose, onCreated }: { onClose: () => void; onCreated?: (c: Company) => void }) {
  const { values, set } = useValues({ lifecycle_stage: "lead" });
  const m = useCreate<Company>("/companies", (c) => (onCreated ?? (() => {}))(c), "Company created");
  const submit = (e: FormEvent) => {
    e.preventDefault();
    m.mutate(clean(values), { onSuccess: onClose });
  };
  return (
    <Panel title="Create company" onClose={onClose} footer={<Footer onClose={onClose} busy={m.isPending} label="Create company" />}>
      <form id="create-form" onSubmit={submit}>
        {m.error && <div className="error-box">{m.error.message}</div>}
        <Text label="Company domain" name="domain" values={values} set={set} placeholder="example.com" />
        <Text label="Company name" name="name" values={values} set={set} required />
        <Text label="Industry" name="industry" values={values} set={set} />
        <Text label="Number of employees" name="employees" type="number" values={values} set={set} />
        <Text label="City" name="city" values={values} set={set} />
        <Text label="Country" name="country" values={values} set={set} />
        <Select label="Lifecycle stage" name="lifecycle_stage" values={values} set={set} options={LIFECYCLE_STAGES} />
      </form>
    </Panel>
  );
}

export function CreateDealPanel({
  onClose,
  onCreated,
  defaults = {},
}: {
  onClose: () => void;
  onCreated?: (d: Deal) => void;
  defaults?: Values;
}) {
  const pipelines = usePipelines();
  const companies = useCompanyOptions();
  const contacts = useContactOptions();
  const { values, set } = useValues({ currency: "GBP", ...defaults });
  const pipeline = pipelines.data?.find((p) => p.id === values.pipeline_id) ?? pipelines.data?.[0];
  const stageOptions = (pipeline?.stages ?? []).map((s) => ({ value: s.id, label: s.name }));
  const m = useCreate<Deal>("/deals", (d) => (onCreated ?? (() => {}))(d), "Deal created");
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const body = clean(values);
    if (!body.stage_id && stageOptions[0]) body.stage_id = stageOptions[0].value;
    m.mutate(body, { onSuccess: onClose });
  };
  const selectedContacts = (values.contact_ids as string[] | undefined) ?? [];
  return (
    <Panel title="Create deal" onClose={onClose} footer={<Footer onClose={onClose} busy={m.isPending} label="Create deal" />}>
      <form id="create-form" onSubmit={submit}>
        {m.error && <div className="error-box">{m.error.message}</div>}
        <Text label="Deal name" name="name" values={values} set={set} required />
        {(pipelines.data?.length ?? 0) > 1 && (
          <Select
            label="Pipeline"
            name="pipeline_id"
            values={values}
            set={set}
            options={(pipelines.data ?? []).map((p) => ({ value: p.id, label: p.name }))}
          />
        )}
        <Select label="Deal stage" name="stage_id" values={values} set={set} options={stageOptions} />
        <Text label="Amount" name="amount" type="number" values={values} set={set} />
        <Text label="Close date" name="close_date" type="date" values={values} set={set} />
        <Select label="Company" name="company_id" values={values} set={set} options={companies.data ?? []} placeholder="No company" />
        <div className="field">
          <label htmlFor="f-contacts">Contacts</label>
          <select
            id="f-contacts"
            className="select"
            multiple
            size={Math.min(6, Math.max(3, contacts.data?.length ?? 3))}
            value={selectedContacts}
            onChange={(e) => set("contact_ids", Array.from(e.target.selectedOptions).map((o) => o.value))}
          >
            {(contacts.data ?? []).map((c) => (
              <option key={c.value} value={c.value}>
                {c.label}
                {c.sub ? `  (${c.sub})` : ""}
              </option>
            ))}
          </select>
          <span className="hint">Hold Ctrl or Cmd to pick more than one.</span>
        </div>
        <Select
          label="Priority"
          name="priority"
          values={values}
          set={set}
          placeholder="None"
          options={[
            { value: "low", label: "Low" },
            { value: "medium", label: "Medium" },
            { value: "high", label: "High" },
          ]}
        />
      </form>
    </Panel>
  );
}

export function CreateTaskPanel({ onClose, defaults = {} }: { onClose: () => void; defaults?: Values }) {
  const { values, set } = useValues({ kind: "todo", priority: "medium", ...defaults });
  const m = useCreate<Task>("/tasks", () => {}, "Task created");
  const submit = (e: FormEvent) => {
    e.preventDefault();
    const body = clean(values);
    if (typeof body.due_at === "string") body.due_at = new Date(body.due_at as string).toISOString();
    m.mutate(body, { onSuccess: onClose });
  };
  return (
    <Panel title="Create task" onClose={onClose} footer={<Footer onClose={onClose} busy={m.isPending} label="Create task" />}>
      <form id="create-form" onSubmit={submit}>
        {m.error && <div className="error-box">{m.error.message}</div>}
        <Text label="Task" name="title" values={values} set={set} required />
        <Text label="Due" name="due_at" type="datetime-local" values={values} set={set} />
        <Select
          label="Type"
          name="kind"
          values={values}
          set={set}
          options={[
            { value: "todo", label: "To-do" },
            { value: "follow_up", label: "Follow-up" },
            { value: "call", label: "Call" },
            { value: "email", label: "Email" },
          ]}
        />
        <Select
          label="Priority"
          name="priority"
          values={values}
          set={set}
          options={[
            { value: "low", label: "Low" },
            { value: "medium", label: "Medium" },
            { value: "high", label: "High" },
          ]}
        />
        <div className="field">
          <label htmlFor="f-notes">Notes</label>
          <textarea id="f-notes" className="textarea" rows={4} value={(values.notes as string) ?? ""} onChange={(e) => set("notes", e.target.value)} />
        </div>
      </form>
    </Panel>
  );
}
