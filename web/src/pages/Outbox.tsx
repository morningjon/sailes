import { Link } from "react-router-dom";
import { useMutation, useQuery, useQueryClient } from "@tanstack/react-query";
import { del, get, post, type OutboxItem } from "../lib/api";
import { formatDateTime } from "../lib/format";
import { Empty, Loading, Pill, useToast } from "../components/ui";

export function OutboxPage() {
  const q = useQuery({ queryKey: ["outbox"], queryFn: () => get<OutboxItem[]>("/outbox") });
  const qc = useQueryClient();
  const toast = useToast();
  const cancel = useMutation({
    mutationFn: (id: string) => del(`/outbox/${id}`),
    onSuccess: () => {
      toast("Email cancelled");
      qc.invalidateQueries();
    },
    onError: (e: Error) => toast(e.message, true),
  });
  const retry = useMutation({
    mutationFn: (id: string) => post(`/outbox/${id}/retry`),
    onSuccess: () => {
      toast("Queued again for the next run");
      qc.invalidateQueries();
    },
    onError: (e: Error) => toast(e.message, true),
  });
  const items = q.data ?? [];
  const queued = items.filter((i) => i.status === "queued").length;
  return (
    <div className="page" style={{ maxWidth: 960 }}>
      <div className="page-head">
        <div>
          <h1>Outbox</h1>
          <div className="sub">
            Emails you've approved. On its next run Claude puts each one in your Outlook drafts, threaded into the
            conversation, ready for you to send{queued > 0 ? `. ${queued} waiting for the next run.` : "."}
          </div>
        </div>
      </div>
      {q.isLoading ? (
        <Loading />
      ) : items.length === 0 ? (
        <div className="table-card">
          <Empty title="No emails yet">Approve an email draft in Review and it lands here before sending.</Empty>
        </div>
      ) : (
        items.map((o) => (
          <div key={o.id} className="letter" style={{ margin: "0 0 14px" }}>
            <div className="letter-head">
              <span className="k">To</span>
              <span>
                {o.recipients.join(", ")}
                {o.contact_id && (
                  <>
                    {" "}
                    (<Link to={`/contacts/${o.contact_id}`}>{o.contact_name}</Link>)
                  </>
                )}
              </span>
              {o.cc.length > 0 && (
                <>
                  <span className="k">Cc</span>
                  <span>{o.cc.join(", ")}</span>
                </>
              )}
              <span className="k">Subject</span>
              <span style={{ fontWeight: 500 }}>{o.subject}</span>
            </div>
            <div className="letter-body">{o.body}</div>
            <div className="composer-foot">
              {o.status === "queued" && <Pill tone="amber">Waiting for the next run</Pill>}
              {o.status === "drafted" && <Pill tone="purple">In your Outlook drafts</Pill>}
              {o.status === "drafted" && o.external_url && (
                <a href={o.external_url} target="_blank" rel="noreferrer" style={{ fontSize: 13 }}>
                  Open in Outlook
                </a>
              )}
              {o.status === "sent" && <Pill tone="green">Sent {formatDateTime(o.sent_at)}</Pill>}
              {o.status === "failed" && <Pill tone="red">Couldn't hand over: {o.error}</Pill>}
              {o.deal_id && (
                <span className="muted" style={{ fontSize: 13 }}>
                  Deal: <Link to={`/deals/${o.deal_id}`}>{o.deal_name}</Link>
                </span>
              )}
              <span className="faint" style={{ marginLeft: "auto", fontSize: 12 }}>
                Approved {formatDateTime(o.created_at)}
              </span>
              {o.status === "failed" && (
                <button className="btn small" onClick={() => retry.mutate(o.id)} disabled={retry.isPending}>
                  Try again
                </button>
              )}
              {o.status === "queued" && (
                <button className="btn small danger" onClick={() => cancel.mutate(o.id)} disabled={cancel.isPending}>
                  Cancel email
                </button>
              )}
            </div>
          </div>
        ))
      )}
    </div>
  );
}
