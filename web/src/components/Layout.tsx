import { useCallback, useEffect, useRef, useState } from "react";
import { NavLink, Outlet, useLocation, useNavigate } from "react-router-dom";
import { useQuery } from "@tanstack/react-query";
import {
  Building2,
  ChevronDown,
  CircleCheck,
  Handshake,
  Inbox,
  LogOut,
  Menu,
  Plug,
  Plus,
  Search,
  Send,
  Users,
} from "lucide-react";
import { get, type SearchHit, type User } from "../lib/api";
import { useSummary } from "../lib/hooks";
import { CreateCompanyPanel, CreateContactPanel, CreateDealPanel, CreateTaskPanel } from "./forms";
import { Avatar, useClickAway } from "./ui";

export function Brand() {
  return (
    <div className="brand">
      <svg width="26" height="26" viewBox="0 0 32 32" aria-hidden>
        <rect width="32" height="32" rx="8" fill="#6a3fd1" />
        <path
          d="M10 21.5c1.4 1.3 3.4 2 5.6 2 3.3 0 5.4-1.6 5.4-4 0-2.3-1.6-3.3-4.8-4l-1.6-.4c-1.9-.4-2.6-1-2.6-2 0-1.2 1.2-2 3-2 1.6 0 2.9.6 3.9 1.6"
          fill="none"
          stroke="#fff"
          strokeWidth="2.4"
          strokeLinecap="round"
        />
        <circle cx="23.5" cy="8.5" r="2.5" fill="#f2c14e" />
      </svg>
      <span>
        s<span className="ai-letters">AI</span>les
      </span>
    </div>
  );
}

function GlobalSearch() {
  const [q, setQ] = useState("");
  const [open, setOpen] = useState(false);
  const [hl, setHl] = useState(0);
  const [debounced, setDebounced] = useState("");
  const navigate = useNavigate();
  const close = useCallback(() => setOpen(false), []);
  const ref = useClickAway<HTMLDivElement>(open, close);
  const inputRef = useRef<HTMLInputElement>(null);

  useEffect(() => {
    const t = setTimeout(() => setDebounced(q.trim()), 150);
    return () => clearTimeout(t);
  }, [q]);

  useEffect(() => {
    const onKey = (e: KeyboardEvent) => {
      if (e.key === "/" && !(e.target instanceof HTMLInputElement || e.target instanceof HTMLTextAreaElement || e.target instanceof HTMLSelectElement)) {
        e.preventDefault();
        inputRef.current?.focus();
      }
    };
    window.addEventListener("keydown", onKey);
    return () => window.removeEventListener("keydown", onKey);
  }, []);

  const results = useQuery({
    queryKey: ["search", debounced],
    queryFn: () => get<SearchHit[]>(`/search?q=${encodeURIComponent(debounced)}`),
    enabled: debounced.length > 0,
  });
  const hits = results.data ?? [];

  const go = (h: SearchHit) => {
    navigate(`/${h.type === "company" ? "companies" : h.type + "s"}/${h.id}`);
    setQ("");
    setOpen(false);
    inputRef.current?.blur();
  };

  return (
    <div className="search" ref={ref}>
      <Search size={16} />
      <input
        ref={inputRef}
        type="search"
        placeholder="Search contacts, companies and deals"
        aria-label="Search"
        value={q}
        onChange={(e) => {
          setQ(e.target.value);
          setOpen(true);
          setHl(0);
        }}
        onFocus={() => setOpen(true)}
        onKeyDown={(e) => {
          if (e.key === "ArrowDown") {
            e.preventDefault();
            setHl((h) => Math.min(h + 1, hits.length - 1));
          } else if (e.key === "ArrowUp") {
            e.preventDefault();
            setHl((h) => Math.max(h - 1, 0));
          } else if (e.key === "Enter" && hits[hl]) {
            go(hits[hl]);
          }
        }}
      />
      {open && debounced && (
        <div className="search-results">
          {hits.length === 0 && !results.isFetching && <div className="empty">No contacts, companies or deals match “{debounced}”.</div>}
          {hits.map((h, i) => (
            <button key={`${h.type}-${h.id}`} className={i === hl ? "hl" : ""} onMouseEnter={() => setHl(i)} onClick={() => go(h)}>
              <Avatar name={h.title ?? "?"} square={h.type !== "contact"} />
              <span style={{ minWidth: 0 }}>
                <div style={{ fontWeight: 500 }}>{h.title ?? "Unnamed"}</div>
                <div className="faint" style={{ fontSize: 12 }}>
                  {h.type === "contact" ? "Contact" : h.type === "company" ? "Company" : "Deal"}
                  {h.subtitle ? `, ${h.subtitle}` : ""}
                </div>
              </span>
            </button>
          ))}
        </div>
      )}
    </div>
  );
}

function CreateMenu() {
  const [open, setOpen] = useState(false);
  const [panel, setPanel] = useState<null | "contact" | "company" | "deal" | "task">(null);
  const close = useCallback(() => setOpen(false), []);
  const ref = useClickAway<HTMLDivElement>(open, close);
  const navigate = useNavigate();
  const pick = (p: typeof panel) => {
    setPanel(p);
    setOpen(false);
  };
  return (
    <div className="menu-wrap" ref={ref}>
      <button className="btn primary" onClick={() => setOpen((o) => !o)} aria-expanded={open}>
        <Plus size={16} />
        <span className="create-label">Create</span>
      </button>
      {open && (
        <div className="menu" role="menu">
          <button role="menuitem" onClick={() => pick("contact")}>
            <Users size={16} /> Contact
          </button>
          <button role="menuitem" onClick={() => pick("company")}>
            <Building2 size={16} /> Company
          </button>
          <button role="menuitem" onClick={() => pick("deal")}>
            <Handshake size={16} /> Deal
          </button>
          <button role="menuitem" onClick={() => pick("task")}>
            <CircleCheck size={16} /> Task
          </button>
        </div>
      )}
      {panel === "contact" && <CreateContactPanel onClose={() => setPanel(null)} onCreated={(c) => navigate(`/contacts/${c.id}`)} />}
      {panel === "company" && <CreateCompanyPanel onClose={() => setPanel(null)} onCreated={(c) => navigate(`/companies/${c.id}`)} />}
      {panel === "deal" && <CreateDealPanel onClose={() => setPanel(null)} onCreated={(d) => navigate(`/deals/${d.id}`)} />}
      {panel === "task" && <CreateTaskPanel onClose={() => setPanel(null)} />}
    </div>
  );
}

function UserMenu({ user, onSignOut }: { user: User; onSignOut: () => void }) {
  const [open, setOpen] = useState(false);
  const close = useCallback(() => setOpen(false), []);
  const ref = useClickAway<HTMLDivElement>(open, close);
  return (
    <div className="menu-wrap" ref={ref}>
      <button className="btn ghost" onClick={() => setOpen((o) => !o)} aria-label="Account menu" aria-expanded={open}>
        <Avatar name={user.name} />
        <ChevronDown size={14} />
      </button>
      {open && (
        <div className="menu" role="menu">
          <div style={{ padding: "6px 10px 10px", borderBottom: "1px solid var(--line)", marginBottom: 6 }}>
            <div style={{ fontWeight: 600 }}>{user.name}</div>
            <div className="faint" style={{ fontSize: 13 }}>
              {user.email}
            </div>
          </div>
          <button role="menuitem" onClick={onSignOut}>
            <LogOut size={16} /> Sign out
          </button>
        </div>
      )}
    </div>
  );
}

export function Layout({ user, onSignOut }: { user: User; onSignOut: () => void }) {
  const summary = useSummary();
  const s = summary.data;
  const [navOpen, setNavOpen] = useState(false);
  const location = useLocation();
  useEffect(() => setNavOpen(false), [location.pathname]);

  const taskCount = (s?.tasks_overdue ?? 0) + (s?.tasks_today ?? 0);
  return (
    <div className="shell">
      <nav className={`nav ${navOpen ? "open" : ""}`} aria-label="Main">
        <Brand />
        <NavLink to="/" end>
          <Inbox size={18} /> Review
          {!!s?.pending_proposals && <span className="count">{s.pending_proposals}</span>}
        </NavLink>
        <div className="section">CRM</div>
        <NavLink to="/contacts">
          <Users size={18} /> Contacts
        </NavLink>
        <NavLink to="/companies">
          <Building2 size={18} /> Companies
        </NavLink>
        <NavLink to="/deals">
          <Handshake size={18} /> Deals
        </NavLink>
        <NavLink to="/tasks">
          <CircleCheck size={18} /> Tasks
          {!!taskCount && <span className="count plain">{taskCount}</span>}
        </NavLink>
        <NavLink to="/outbox">
          <Send size={18} /> Outbox
          {!!s?.outbox_queued && <span className="count plain">{s.outbox_queued}</span>}
        </NavLink>
        <div className="spacer" />
        <NavLink to="/connections">
          <Plug size={18} /> Connections
        </NavLink>
      </nav>
      <div className="main">
        <header className="topbar">
          <button className="btn ghost icon menu-btn" onClick={() => setNavOpen((o) => !o)} aria-label="Open menu">
            <Menu size={18} />
          </button>
          <GlobalSearch />
          <div style={{ marginLeft: "auto", display: "flex", gap: 8, alignItems: "center" }}>
            <CreateMenu />
            <UserMenu user={user} onSignOut={onSignOut} />
          </div>
        </header>
        <main className="content">
          <Outlet />
        </main>
      </div>
      {navOpen && <div className="overlay" style={{ zIndex: 54 }} onClick={() => setNavOpen(false)} />}
    </div>
  );
}
