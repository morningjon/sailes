const money = new Map<string, Intl.NumberFormat>();

export function formatMoney(amount: number | null | undefined, currency = "GBP", compact = false): string {
  if (amount === null || amount === undefined || Number.isNaN(amount)) return "";
  const key = `${currency}-${compact}`;
  let f = money.get(key);
  if (!f) {
    f = new Intl.NumberFormat("en-GB", {
      style: "currency",
      currency,
      maximumFractionDigits: compact ? 1 : 0,
      notation: compact ? "compact" : "standard",
    });
    money.set(key, f);
  }
  return f.format(amount);
}

const dateFmt = new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short", year: "numeric" });
const shortDateFmt = new Intl.DateTimeFormat("en-GB", { day: "numeric", month: "short" });
const timeFmt = new Intl.DateTimeFormat("en-GB", { hour: "2-digit", minute: "2-digit" });

export function formatDate(value: string | null | undefined): string {
  if (!value) return "";
  const d = new Date(value.length === 10 ? `${value}T12:00:00` : value);
  if (Number.isNaN(d.getTime())) return value;
  return d.getFullYear() === new Date().getFullYear() ? shortDateFmt.format(d) : dateFmt.format(d);
}

export function formatDateTime(value: string | null | undefined): string {
  if (!value) return "";
  const d = new Date(value);
  return `${formatDate(value)}, ${timeFmt.format(d)}`;
}

export function relativeTime(value: string | null | undefined): string {
  if (!value) return "";
  const d = new Date(value.length === 10 ? `${value}T12:00:00` : value).getTime();
  const diff = d - Date.now();
  const abs = Math.abs(diff);
  const mins = Math.round(abs / 60000);
  const hours = Math.round(abs / 3600000);
  const days = Math.round(abs / 86400000);
  const future = diff > 0;
  const wrap = (s: string) => (future ? `in ${s}` : `${s} ago`);
  if (mins < 1) return "just now";
  if (mins < 60) return wrap(`${mins} min`);
  if (hours < 24) return wrap(`${hours} hr`);
  if (days === 1) return future ? "tomorrow" : "yesterday";
  if (days < 7) return wrap(`${days} days`);
  return formatDate(value);
}

/** Due-date state for tasks. */
export function dueState(value: string | null): "overdue" | "today" | "upcoming" | "none" {
  if (!value) return "none";
  const d = new Date(value);
  const start = new Date();
  start.setHours(0, 0, 0, 0);
  const end = new Date(start);
  end.setDate(end.getDate() + 1);
  if (d < start) return "overdue";
  if (d < end) return "today";
  return "upcoming";
}

export function fullName(c: { first_name?: string | null; last_name?: string | null; email?: string | null }): string {
  const n = `${c.first_name ?? ""} ${c.last_name ?? ""}`.trim();
  return n || c.email || "Unnamed contact";
}

export function initials(name: string): string {
  const parts = name.replace(/[^\p{L}\s&]/gu, " ").split(/\s+/).filter((p) => p && p !== "&");
  if (parts.length === 0) return "?";
  return ((parts[0][0] ?? "") + (parts.length > 1 ? parts[parts.length - 1][0] : "")).toUpperCase();
}

export const LIFECYCLE_STAGES = [
  { value: "subscriber", label: "Subscriber" },
  { value: "lead", label: "Lead" },
  { value: "opportunity", label: "Opportunity" },
  { value: "customer", label: "Customer" },
  { value: "evangelist", label: "Evangelist" },
  { value: "other", label: "Other" },
];

export function lifecycleLabel(v: string | null | undefined): string {
  return LIFECYCLE_STAGES.find((s) => s.value === v)?.label ?? (v ? v[0].toUpperCase() + v.slice(1) : "");
}

export function sourceLabel(v: string | null | undefined): string {
  switch (v) {
    case "outlook":
      return "Outlook";
    case "slack":
      return "Slack";
    case "teams":
      return "Teams";
    case "manual":
      return "Added by hand";
    default:
      return v ? v[0].toUpperCase() + v.slice(1) : "";
  }
}

export function toDateInput(value: string | null | undefined): string {
  if (!value) return "";
  return value.slice(0, 10);
}
