import { Pill } from "../components/ui";

const CONNECTORS = [
  {
    name: "Microsoft Outlook",
    logo: "O",
    colour: "#2a6fdb",
    body: "Reads new email in your mailbox, suggests contacts, notes and follow-ups, and sends the replies you approve from your own address.",
  },
  {
    name: "Microsoft Teams",
    logo: "T",
    colour: "#5b5fc7",
    body: "Reads chats and meeting transcripts, turns them into notes, tasks and deal updates.",
  },
  {
    name: "Slack",
    logo: "S",
    colour: "#4a154b",
    body: "Reads the channels and direct messages you choose, including Slack Connect channels with customers.",
  },
];

export function ConnectionsPage() {
  return (
    <div className="page" style={{ maxWidth: 880 }}>
      <div className="page-head">
        <div>
          <h1>Connections</h1>
          <div className="sub">Where sAIles reads your conversations. Everything it finds goes to Review before it touches your CRM.</div>
        </div>
      </div>
      <div className="table-card">
        {CONNECTORS.map((c) => (
          <div className="connector" key={c.name}>
            <div className="logo" style={{ background: c.colour }} aria-hidden>
              {c.logo}
            </div>
            <div className="body">
              <h3>{c.name}</h3>
              <p>{c.body}</p>
            </div>
            <Pill>Coming next</Pill>
          </div>
        ))}
      </div>
      <div className="side-card" style={{ marginTop: 20 }}>
        <h3>AI provider</h3>
        <p className="muted" style={{ margin: 0 }}>
          sAIles uses Claude by default to read messages and draft replies. You can switch to a model running on your own hardware so
          nothing leaves your server. This is set with environment variables on the server.
        </p>
      </div>
    </div>
  );
}
