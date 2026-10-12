import { useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { Plus } from "lucide-react";
import type { Company, Contact } from "../lib/api";
import { formatMoney, fullName, relativeTime } from "../lib/format";
import { ObjectTable } from "../components/ObjectTable";
import { CreateCompanyPanel, CreateContactPanel } from "../components/forms";
import { Avatar, LifecyclePill } from "../components/ui";

export function ContactsPage() {
  const [creating, setCreating] = useState(false);
  const navigate = useNavigate();
  const create = (
    <button className="btn primary" onClick={() => setCreating(true)}>
      <Plus size={16} /> Create contact
    </button>
  );
  return (
    <div className="page">
      <div className="page-head">
        <h1>Contacts</h1>
        <div className="actions">{create}</div>
      </div>
      <ObjectTable<Contact>
        path="/contacts"
        noun={{ one: "contact", many: "contacts" }}
        views={[
          { key: "all", label: "All contacts" },
          { key: "lead", label: "Leads", params: { lifecycle_stage: "lead" } },
          { key: "opp", label: "Opportunities", params: { lifecycle_stage: "opportunity" } },
          { key: "cust", label: "Customers", params: { lifecycle_stage: "customer" } },
        ]}
        rowHref={(c) => `/contacts/${c.id}`}
        emptyAction={create}
        columns={[
          {
            key: "name",
            label: "Name",
            sort: "name",
            render: (c) => (
              <span className="primary-cell">
                <Avatar name={fullName(c)} />
                <Link to={`/contacts/${c.id}`}>{fullName(c)}</Link>
              </span>
            ),
          },
          { key: "email", label: "Email", sort: "email", render: (c) => c.email ?? <span className="faint">None</span> },
          { key: "title", label: "Job title", render: (c) => c.job_title ?? "" },
          {
            key: "company",
            label: "Company",
            sort: "company",
            render: (c) => (c.company_id ? <Link to={`/companies/${c.company_id}`}>{c.company_name}</Link> : ""),
          },
          { key: "stage", label: "Lifecycle stage", sort: "lifecycle_stage", render: (c) => <LifecyclePill stage={c.lifecycle_stage} /> },
          {
            key: "activity",
            label: "Last activity",
            sort: "last_activity_at",
            render: (c) => <span className="muted">{relativeTime(c.last_activity_at)}</span>,
          },
        ]}
      />
      {creating && <CreateContactPanel onClose={() => setCreating(false)} onCreated={(c) => navigate(`/contacts/${c.id}`)} />}
    </div>
  );
}

export function CompaniesPage() {
  const [creating, setCreating] = useState(false);
  const navigate = useNavigate();
  const create = (
    <button className="btn primary" onClick={() => setCreating(true)}>
      <Plus size={16} /> Create company
    </button>
  );
  return (
    <div className="page">
      <div className="page-head">
        <h1>Companies</h1>
        <div className="actions">{create}</div>
      </div>
      <ObjectTable<Company>
        path="/companies"
        noun={{ one: "company", many: "companies" }}
        views={[
          { key: "all", label: "All companies" },
          { key: "lead", label: "Leads", params: { lifecycle_stage: "lead" } },
          { key: "opp", label: "Opportunities", params: { lifecycle_stage: "opportunity" } },
          { key: "cust", label: "Customers", params: { lifecycle_stage: "customer" } },
        ]}
        rowHref={(c) => `/companies/${c.id}`}
        emptyAction={create}
        columns={[
          {
            key: "name",
            label: "Company name",
            sort: "name",
            render: (c) => (
              <span className="primary-cell">
                <Avatar name={c.name} square />
                <span>
                  <Link to={`/companies/${c.id}`}>{c.name}</Link>
                  {c.domain && (
                    <div className="faint" style={{ fontSize: 12, fontWeight: 400 }}>
                      {c.domain}
                    </div>
                  )}
                </span>
              </span>
            ),
          },
          { key: "industry", label: "Industry", sort: "industry", render: (c) => c.industry ?? "" },
          { key: "city", label: "City", sort: "city", render: (c) => c.city ?? "" },
          { key: "contacts", label: "Contacts", render: (c) => <span className="num">{c.contact_count}</span> },
          {
            key: "open",
            label: "Open deals",
            sort: "open_deal_value",
            render: (c) =>
              c.open_deal_count ? (
                <span className="num">
                  {formatMoney(c.open_deal_value)} <span className="faint">({c.open_deal_count})</span>
                </span>
              ) : (
                ""
              ),
          },
          { key: "stage", label: "Lifecycle stage", render: (c) => <LifecyclePill stage={c.lifecycle_stage} /> },
          {
            key: "activity",
            label: "Last activity",
            sort: "last_activity_at",
            render: (c) => <span className="muted">{relativeTime(c.last_activity_at)}</span>,
          },
        ]}
      />
      {creating && <CreateCompanyPanel onClose={() => setCreating(false)} onCreated={(c) => navigate(`/companies/${c.id}`)} />}
    </div>
  );
}
