-- sAIles core schema
-- Everything the AI wants to change goes through `proposals` first. Only approved
-- proposals touch the CRM tables (companies, contacts, deals, activities, tasks).

-- Users and sessions ---------------------------------------------------------

create table users (
    id            uuid primary key default gen_random_uuid(),
    email         text not null unique,
    name          text not null,
    password_hash text not null,
    created_at    timestamptz not null default now()
);

create table sessions (
    token_hash text primary key,
    user_id    uuid not null references users(id) on delete cascade,
    expires_at timestamptz not null,
    created_at timestamptz not null default now()
);
create index sessions_user_idx on sessions(user_id);

-- Companies ------------------------------------------------------------------

create table companies (
    id              uuid primary key default gen_random_uuid(),
    name            text not null,
    domain          text,
    industry        text,
    employees       integer,
    city            text,
    country         text,
    phone           text,
    description     text,
    lifecycle_stage text not null default 'lead',
    owner_id        uuid references users(id) on delete set null,
    source          text not null default 'manual',
    created_at      timestamptz not null default now(),
    updated_at      timestamptz not null default now()
);
create unique index companies_domain_uq on companies(lower(domain)) where domain is not null;
create index companies_name_idx on companies(lower(name));

-- Contacts -------------------------------------------------------------------

create table contacts (
    id                uuid primary key default gen_random_uuid(),
    first_name        text not null default '',
    last_name         text not null default '',
    email             text,
    phone             text,
    job_title         text,
    company_id        uuid references companies(id) on delete set null,
    lifecycle_stage   text not null default 'lead',
    lead_status       text,
    owner_id          uuid references users(id) on delete set null,
    source            text not null default 'manual',
    last_contacted_at timestamptz,
    created_at        timestamptz not null default now(),
    updated_at        timestamptz not null default now()
);
create unique index contacts_email_uq on contacts(lower(email)) where email is not null;
create index contacts_company_idx on contacts(company_id);

-- Pipelines and deals ----------------------------------------------------------

create table pipelines (
    id         uuid primary key default gen_random_uuid(),
    name       text not null,
    position   integer not null default 0,
    created_at timestamptz not null default now()
);

create table pipeline_stages (
    id          uuid primary key default gen_random_uuid(),
    pipeline_id uuid not null references pipelines(id) on delete cascade,
    name        text not null,
    position    integer not null,
    probability integer not null default 0 check (probability between 0 and 100),
    is_closed   boolean not null default false,
    is_won      boolean not null default false
);
create index pipeline_stages_pipeline_idx on pipeline_stages(pipeline_id, position);

create table deals (
    id          uuid primary key default gen_random_uuid(),
    name        text not null,
    amount      numeric(14, 2),
    currency    text not null default 'GBP',
    pipeline_id uuid not null references pipelines(id),
    stage_id    uuid not null references pipeline_stages(id),
    close_date  date,
    company_id  uuid references companies(id) on delete set null,
    owner_id    uuid references users(id) on delete set null,
    priority    text,
    next_step   text,
    source      text not null default 'manual',
    created_at  timestamptz not null default now(),
    updated_at  timestamptz not null default now()
);
create index deals_stage_idx on deals(stage_id);
create index deals_company_idx on deals(company_id);

create table deal_contacts (
    deal_id    uuid not null references deals(id) on delete cascade,
    contact_id uuid not null references contacts(id) on delete cascade,
    primary key (deal_id, contact_id)
);
create index deal_contacts_contact_idx on deal_contacts(contact_id);

-- Activities: the timeline -----------------------------------------------------
-- One row per email, Slack message, Teams message, meeting, call, note or system event.
-- Linked to any number of contacts, companies and deals via activity_links.

create table activities (
    id          uuid primary key default gen_random_uuid(),
    kind        text not null check (kind in ('note', 'email', 'slack', 'teams', 'meeting', 'call', 'system')),
    subject     text,
    body        text not null default '',
    direction   text check (direction in ('inbound', 'outbound')),
    participants jsonb not null default '[]'::jsonb,
    external_id text,
    source_url  text,
    occurred_at timestamptz not null default now(),
    created_by  text not null default 'user' check (created_by in ('user', 'ai', 'connector')),
    created_at  timestamptz not null default now()
);
create unique index activities_external_uq on activities(kind, external_id) where external_id is not null;
create index activities_occurred_idx on activities(occurred_at desc);

create table activity_links (
    activity_id uuid not null references activities(id) on delete cascade,
    object_type text not null check (object_type in ('contact', 'company', 'deal')),
    object_id   uuid not null,
    primary key (activity_id, object_type, object_id)
);
create index activity_links_object_idx on activity_links(object_type, object_id);

-- Tasks ------------------------------------------------------------------------

create table tasks (
    id           uuid primary key default gen_random_uuid(),
    title        text not null,
    notes        text,
    kind         text not null default 'todo' check (kind in ('todo', 'follow_up', 'call', 'email')),
    priority     text not null default 'medium' check (priority in ('low', 'medium', 'high')),
    status       text not null default 'open' check (status in ('open', 'done')),
    due_at       timestamptz,
    contact_id   uuid references contacts(id) on delete cascade,
    company_id   uuid references companies(id) on delete cascade,
    deal_id      uuid references deals(id) on delete cascade,
    owner_id     uuid references users(id) on delete set null,
    created_by   text not null default 'user' check (created_by in ('user', 'ai')),
    completed_at timestamptz,
    created_at   timestamptz not null default now()
);
create index tasks_open_idx on tasks(status, due_at);

-- Proposals: the approval queue ------------------------------------------------

create table proposals (
    id                 uuid primary key default gen_random_uuid(),
    kind               text not null check (kind in (
                           'create_contact', 'update_contact',
                           'create_company', 'update_company',
                           'create_deal', 'update_deal',
                           'add_note', 'create_task', 'send_email')),
    status             text not null default 'pending' check (status in ('pending', 'approved', 'rejected', 'failed')),
    title              text not null,
    rationale          text,
    payload            jsonb not null,
    target_type        text check (target_type in ('contact', 'company', 'deal')),
    target_id          uuid,
    confidence         real check (confidence between 0 and 1),
    source_channel     text,
    source_excerpt     text,
    source_activity_id uuid references activities(id) on delete set null,
    result_id          uuid,
    error              text,
    -- Why the user rejected or edited it; kept to improve future extraction.
    feedback           text,
    edited             boolean not null default false,
    decided_by         uuid references users(id) on delete set null,
    decided_at         timestamptz,
    created_at         timestamptz not null default now()
);
create index proposals_status_idx on proposals(status, created_at desc);

-- Outbox: approved emails waiting for a connector to send them --------------------

create table outbox (
    id          uuid primary key default gen_random_uuid(),
    proposal_id uuid references proposals(id) on delete set null,
    channel     text not null default 'email',
    recipients  jsonb not null default '[]'::jsonb,
    cc          jsonb not null default '[]'::jsonb,
    subject     text,
    body        text not null,
    in_reply_to text,
    contact_id  uuid references contacts(id) on delete set null,
    deal_id     uuid references deals(id) on delete set null,
    status      text not null default 'queued' check (status in ('queued', 'sent', 'failed')),
    error       text,
    created_at  timestamptz not null default now(),
    sent_at     timestamptz
);
