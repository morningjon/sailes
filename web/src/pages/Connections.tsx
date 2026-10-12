import { useQuery } from "@tanstack/react-query";
import { CircleAlert, FolderSync } from "lucide-react";
import { get } from "../lib/api";
import { formatDateTime, relativeTime } from "../lib/format";
import { Loading, Pill } from "../components/ui";

interface BridgeStatus {
  folder_enabled: boolean;
  outbox: { queued: number; drafted: number; failed: number };
  recent: {
    source: string;
    activities: number;
    proposals: number;
    skipped: number;
    error: string | null;
    summary: string | null;
    created_at: string;
  }[];
}

const CHANNELS = [
  { name: "Outlook", logo: "O", colour: "#2a6fdb", body: "Email in and out, threaded reply drafts." },
  { name: "Teams", logo: "T", colour: "#5b5fc7", body: "Chats and meeting transcripts." },
  { name: "Slack", logo: "S", colour: "#4a154b", body: "Channels, direct messages and Slack Connect." },
];

export function ConnectionsPage() {
  const q = useQuery({ queryKey: ["bridge"], queryFn: () => get<BridgeStatus>("/bridge"), refetchInterval: 30_000 });
  const s = q.data;
  const last = s?.recent.find((r) => !r.error || r.activities > 0 || r.proposals > 0);

  return (
    <div className="page" style={{ maxWidth: 880 }}>
      <div className="page-head">
        <div>
          <h1>Connections</h1>
          <div className="sub">Where sAIles reads your conversations. Everything it finds goes to Review before it touches your CRM.</div>
        </div>
      </div>

      <div className="table-card">
        <div className="connector">
          <div className="logo" style={{ background: "var(--primary)" }} aria-hidden>
            <FolderSync size={20} />
          </div>
          <div className="body">
            <h3>Claude daily run</h3>
            <p>
              Each morning Claude reads your Outlook, Teams and Slack through your Claude connectors, drops what it found into the
              shared folder, and turns the emails you approve into Outlook drafts.
            </p>
            {q.isLoading ? (
              <Loading />
            ) : !s?.folder_enabled ? (
              <p style={{ color: "var(--amber)" }}>
                The shared folder isn't set up on this server. Set SAILES_BRIDGE_DIR and restart sAIles.
              </p>
            ) : last ? (
              <p>
                Last import {relativeTime(last.created_at)}: {last.activities} {last.activities === 1 ? "message" : "messages"},{" "}
                {last.proposals} {last.proposals === 1 ? "suggestion" : "suggestions"}
                {last.skipped ? `, ${last.skipped} already seen` : ""}.
              </p>
            ) : (
              <p>Waiting for the first import.</p>
            )}
          </div>
          {s?.folder_enabled ? <Pill tone="green">Folder connected</Pill> : <Pill tone="amber">Not set up</Pill>}
        </div>
        {CHANNELS.map((c) => (
          <div className="connector" key={c.name}>
            <div className="logo" style={{ background: c.colour }} aria-hidden>
              {c.logo}
            </div>
            <div className="body">
              <h3>{c.name}</h3>
              <p>{c.body}</p>
            </div>
            <Pill>Through Claude</Pill>
          </div>
        ))}
      </div>

      {s && s.recent.length > 0 && (
        <>
          <h2 style={{ margin: "28px 0 12px" }}>Recent imports</h2>
          <div className="table-card">
            <div className="table-scroll">
              <table className="data">
                <thead>
                  <tr>
                    <th>When</th>
                    <th>Batch</th>
                    <th>Messages</th>
                    <th>Suggestions</th>
                    <th>Already seen</th>
                  </tr>
                </thead>
                <tbody>
                  {s.recent.map((r, i) => (
                    <tr key={i}>
                      <td className="num">{formatDateTime(r.created_at)}</td>
                      <td style={{ whiteSpace: "normal" }}>
                        <div>{r.summary ?? r.source}</div>
                        {r.error && (
                          <div style={{ color: "var(--amber)", fontSize: 13, display: "flex", gap: 6, marginTop: 2 }}>
                            <CircleAlert size={14} style={{ flex: "none", marginTop: 2 }} />
                            <span style={{ whiteSpace: "pre-wrap" }}>{r.error}</span>
                          </div>
                        )}
                      </td>
                      <td className="num">{r.activities}</td>
                      <td className="num">{r.proposals}</td>
                      <td className="num">{r.skipped}</td>
                    </tr>
                  ))}
                </tbody>
              </table>
            </div>
          </div>
        </>
      )}
    </div>
  );
}
