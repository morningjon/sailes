-- Bridge: batch imports from Claude (or any connector) and outbox hand-off.

-- Emails can now be handed to Outlook as drafts rather than sent directly.
alter table outbox drop constraint outbox_status_check;
alter table outbox add constraint outbox_status_check check (status in ('queued', 'drafted', 'sent', 'failed'));
alter table outbox add column external_id text;
alter table outbox add column external_url text;
alter table outbox add column handed_off_at timestamptz;

-- Keep what the AI first suggested when a person edits it, so it can learn from the difference.
alter table proposals add column original_payload jsonb;

-- Import log, shown on the Connections page.
create table imports (
    id          uuid primary key default gen_random_uuid(),
    source      text not null,
    activities  integer not null default 0,
    proposals   integer not null default 0,
    skipped     integer not null default 0,
    error       text,
    summary     text,
    created_at  timestamptz not null default now()
);
create index imports_created_idx on imports(created_at desc);
