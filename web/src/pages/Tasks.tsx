import { useState } from "react";
import { Link } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { Check, Plus, Sparkles, Trash2 } from "lucide-react";
import { del, get, patch, qs, type Page, type Task } from "../lib/api";
import { dueState, formatDateTime } from "../lib/format";
import { CreateTaskPanel } from "../components/forms";
import { Empty, Loading, Pill, useToast } from "../components/ui";

const VIEWS = [
  { key: "all", label: "All open", params: { status: "open" } },
  { key: "overdue", label: "Overdue", params: { status: "open", due: "overdue" } },
  { key: "today", label: "Due today", params: { status: "open", due: "today" } },
  { key: "upcoming", label: "Upcoming", params: { status: "open", due: "upcoming" } },
  { key: "done", label: "Completed", params: { status: "done" } },
];

export function TaskCheck({ task }: { task: Task }) {
  const qc = useQueryClient();
  const toast = useToast();
  const toggle = useMutation({
    mutationFn: () => patch<Task>(`/tasks/${task.id}`, { status: task.status === "done" ? "open" : "done" }),
    onSuccess: (t) => {
      toast(t.status === "done" ? "Task completed" : "Task reopened");
      qc.invalidateQueries();
    },
    onError: (e: Error) => toast(e.message, true),
  });
  return (
    <button
      className={`task-check ${task.status === "done" ? "done" : ""}`}
      aria-label={task.status === "done" ? "Mark as not done" : "Mark as done"}
      disabled={toggle.isPending}
      onClick={() => toggle.mutate()}
    >
      <Check size={12} strokeWidth={3} />
    </button>
  );
}

export function DueLabel({ task }: { task: Task }) {
  if (!task.due_at) return <span className="faint">No due date</span>;
  const state = task.status === "done" ? "none" : dueState(task.due_at);
  return <span className={`due num ${state}`}>{state === "overdue" ? "Overdue, " : ""}{formatDateTime(task.due_at)}</span>;
}

const KIND_LABEL: Record<Task["kind"], string> = { todo: "To-do", follow_up: "Follow-up", call: "Call", email: "Email" };

export function TasksPage() {
  const [view, setView] = useState("all");
  const [creating, setCreating] = useState(false);
  const qc = useQueryClient();
  const toast = useToast();
  const params = VIEWS.find((v) => v.key === view)!.params;
  const list = useQuery({ queryKey: ["tasks", params], queryFn: () => get<Page<Task>>(`/tasks${qs({ ...params, limit: 500 })}`) });
  const remove = useMutation({
    mutationFn: (id: string) => del(`/tasks/${id}`),
    onSuccess: () => {
      toast("Task deleted");
      qc.invalidateQueries();
    },
  });
  const items = list.data?.items ?? [];

  return (
    <div className="page">
      <div className="page-head">
        <h1>Tasks</h1>
        <div className="actions">
          <button className="btn primary" onClick={() => setCreating(true)}>
            <Plus size={16} /> Create task
          </button>
        </div>
      </div>
      <div className="tabs" role="tablist">
        {VIEWS.map((v) => (
          <button key={v.key} role="tab" aria-selected={view === v.key} className={view === v.key ? "active" : ""} onClick={() => setView(v.key)}>
            {v.label}
          </button>
        ))}
      </div>
      <div className="table-card">
        {list.isLoading ? (
          <Loading />
        ) : items.length === 0 ? (
          <Empty title={view === "done" ? "No completed tasks yet" : "Nothing to do here"}>
            {view === "done" ? "Tasks you tick off appear here." : "Tasks you create, or approve from Review, appear here."}
          </Empty>
        ) : (
          <div className="table-scroll">
            <table className="data">
              <thead>
                <tr>
                  <th className="check" />
                  <th>Task</th>
                  <th>Due</th>
                  <th>Type</th>
                  <th>Linked to</th>
                  <th />
                </tr>
              </thead>
              <tbody>
                {items.map((t) => (
                  <tr key={t.id}>
                    <td className="check">
                      <TaskCheck task={t} />
                    </td>
                    <td style={{ whiteSpace: "normal", minWidth: 260 }}>
                      <div className={`task-title ${t.status === "done" ? "done" : ""}`} style={{ fontWeight: 500 }}>
                        {t.title}
                      </div>
                      {t.notes && <div className="muted" style={{ fontSize: 13 }}>{t.notes}</div>}
                    </td>
                    <td>
                      <DueLabel task={t} />
                    </td>
                    <td>
                      <span style={{ display: "inline-flex", gap: 6 }}>
                        <Pill tone={t.priority === "high" ? "red" : undefined}>{KIND_LABEL[t.kind]}</Pill>
                        {t.created_by === "ai" && (
                          <Pill tone="ai">
                            <Sparkles size={11} /> Suggested
                          </Pill>
                        )}
                      </span>
                    </td>
                    <td>
                      {t.deal_id ? (
                        <Link to={`/deals/${t.deal_id}`}>{t.deal_name}</Link>
                      ) : t.contact_id ? (
                        <Link to={`/contacts/${t.contact_id}`}>{t.contact_name}</Link>
                      ) : t.company_id ? (
                        <Link to={`/companies/${t.company_id}`}>{t.company_name}</Link>
                      ) : (
                        ""
                      )}
                    </td>
                    <td style={{ textAlign: "right" }}>
                      <button className="btn ghost small icon" aria-label="Delete task" onClick={() => remove.mutate(t.id)}>
                        <Trash2 size={14} />
                      </button>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </div>
        )}
      </div>
      {creating && <CreateTaskPanel onClose={() => setCreating(false)} />}
    </div>
  );
}
