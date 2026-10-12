export class ApiError extends Error {
  status: number;
  constructor(status: number, message: string) {
    super(message);
    this.status = status;
  }
}

let onUnauthorized: (() => void) | null = null;
export function setUnauthorizedHandler(fn: () => void) {
  onUnauthorized = fn;
}

export async function api<T>(path: string, init: RequestInit & { json?: unknown } = {}): Promise<T> {
  const { json, headers, ...rest } = init;
  const res = await fetch(`/api${path}`, {
    credentials: "same-origin",
    ...rest,
    headers: {
      ...(json !== undefined ? { "content-type": "application/json" } : {}),
      ...headers,
    },
    body: json !== undefined ? JSON.stringify(json) : rest.body,
  });
  if (res.status === 401 && !path.startsWith("/auth/")) {
    onUnauthorized?.();
  }
  const text = await res.text();
  const data = text ? JSON.parse(text) : null;
  if (!res.ok) {
    throw new ApiError(res.status, data?.error ?? `Request failed (${res.status})`);
  }
  return data as T;
}

export const get = <T,>(path: string) => api<T>(path);
export const post = <T,>(path: string, json: unknown = {}) => api<T>(path, { method: "POST", json });
export const patch = <T,>(path: string, json: unknown) => api<T>(path, { method: "PATCH", json });
export const del = <T,>(path: string) => api<T>(path, { method: "DELETE" });

export function qs(params: Record<string, string | number | undefined | null>) {
  const s = new URLSearchParams();
  for (const [k, v] of Object.entries(params)) {
    if (v !== undefined && v !== null && v !== "") s.set(k, String(v));
  }
  const str = s.toString();
  return str ? `?${str}` : "";
}

// Types ---------------------------------------------------------------------------

export type ObjectType = "contact" | "company" | "deal";

export interface User {
  id: string;
  email: string;
  name: string;
}

export interface Page<T> {
  items: T[];
  total: number;
}

export interface Contact {
  id: string;
  first_name: string;
  last_name: string;
  email: string | null;
  phone: string | null;
  job_title: string | null;
  company_id: string | null;
  company_name: string | null;
  lifecycle_stage: string;
  lead_status: string | null;
  owner_id: string | null;
  owner_name: string | null;
  source: string;
  last_contacted_at: string | null;
  last_activity_at: string | null;
  created_at: string;
  updated_at: string;
}

export interface Company {
  id: string;
  name: string;
  domain: string | null;
  industry: string | null;
  employees: number | null;
  city: string | null;
  country: string | null;
  phone: string | null;
  description: string | null;
  lifecycle_stage: string;
  owner_id: string | null;
  owner_name: string | null;
  source: string;
  contact_count: number;
  open_deal_count: number;
  open_deal_value: number | null;
  last_activity_at: string | null;
  created_at: string;
}

export interface Deal {
  id: string;
  name: string;
  amount: number | null;
  currency: string;
  pipeline_id: string;
  pipeline_name: string;
  stage_id: string;
  stage_name: string;
  probability: number;
  is_closed: boolean;
  is_won: boolean;
  close_date: string | null;
  company_id: string | null;
  company_name: string | null;
  owner_id: string | null;
  owner_name: string | null;
  priority: string | null;
  next_step: string | null;
  source: string;
  contact_names: string[];
  last_activity_at: string | null;
  created_at: string;
}

export interface Stage {
  id: string;
  name: string;
  position: number;
  probability: number;
  is_closed: boolean;
  is_won: boolean;
}

export interface Pipeline {
  id: string;
  name: string;
  stages: Stage[];
}

export interface Task {
  id: string;
  title: string;
  notes: string | null;
  kind: "todo" | "follow_up" | "call" | "email";
  priority: "low" | "medium" | "high";
  status: "open" | "done";
  due_at: string | null;
  contact_id: string | null;
  contact_name: string | null;
  company_id: string | null;
  company_name: string | null;
  deal_id: string | null;
  deal_name: string | null;
  owner_name: string | null;
  created_by: "user" | "ai";
  completed_at: string | null;
  created_at: string;
}

export interface ActivityLink {
  object_type: ObjectType;
  object_id: string;
  name: string | null;
}

export interface Activity {
  id: string;
  kind: "note" | "email" | "slack" | "teams" | "meeting" | "call" | "system";
  subject: string | null;
  body: string;
  direction: "inbound" | "outbound" | null;
  participants: string[];
  source_url: string | null;
  occurred_at: string;
  created_by: "user" | "ai" | "connector";
  links: ActivityLink[];
}

export type ProposalKind =
  | "create_contact"
  | "update_contact"
  | "create_company"
  | "update_company"
  | "create_deal"
  | "update_deal"
  | "add_note"
  | "create_task"
  | "send_email";

export interface Proposal {
  id: string;
  kind: ProposalKind;
  status: "pending" | "approved" | "rejected" | "failed";
  title: string;
  rationale: string | null;
  payload: Record<string, unknown>;
  target_type: ObjectType | null;
  target_id: string | null;
  confidence: number | null;
  source_channel: string | null;
  source_excerpt: string | null;
  source_activity_id: string | null;
  result_id: string | null;
  error: string | null;
  feedback: string | null;
  edited: boolean;
  decided_at: string | null;
  created_at: string;
  current: Record<string, unknown> | null;
  target_name: string | null;
  refs: Record<string, string>;
  source: {
    id: string;
    kind: Activity["kind"];
    subject: string | null;
    body: string;
    direction: string | null;
    participants: string[];
    occurred_at: string;
  } | null;
}

export interface ProposalList {
  items: Proposal[];
  counts: Record<string, number>;
}

export interface Summary {
  pending_proposals: number;
  pending_emails: number;
  tasks_overdue: number;
  tasks_today: number;
  open_deal_count: number;
  open_pipeline_value: number;
  weighted_pipeline_value: number;
  won_this_month: number;
  outbox_queued: number;
  approved_this_week: number;
}

export interface OutboxItem {
  id: string;
  channel: string;
  recipients: string[];
  cc: string[];
  subject: string | null;
  body: string;
  status: "queued" | "drafted" | "sent" | "failed";
  error: string | null;
  external_url: string | null;
  contact_id: string | null;
  contact_name: string | null;
  deal_id: string | null;
  deal_name: string | null;
  created_at: string;
  sent_at: string | null;
}

export interface SearchHit {
  type: ObjectType;
  id: string;
  title: string | null;
  subtitle: string | null;
}
