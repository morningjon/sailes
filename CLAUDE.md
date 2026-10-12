# sAIles: notes for working on this repo

Self-hosted, GPL CRM that fills itself in from email, Slack and Teams via an approval queue. See README for the product picture.

## Ground rules

- Commit straight to `main` and push. No feature branches or pull requests for this repo.
- User-facing copy: plain, sentence case, British English, no em dashes.
- Connectors and the AI must never write CRM tables directly. They create rows in `proposals` (via the bridge folder or `POST /api/import`); `routes/proposals.rs::apply` is the only path from a suggestion to a record.
- The Claude daily run (docs/claude-daily-run.md) is the connector for now. Keep it, docs/bridge.md and bridge.rs in step.
- Keep data on the user's infrastructure: no third-party CDNs or font services in the dashboard (fonts are self-hosted via @fontsource).

## Layout

- `crates/server` (binary `sailes`): Axum 0.8, SQLx 0.8 (runtime queries, no compile-time DB), Postgres 16.
  - `fields.rs`: whitelisted field specs (`CONTACT`, `COMPANY`, `DEAL`, `TASK`) with typed binding. Add a column here and in a migration.
  - `routes/`: one module per object. `proposals.rs` holds the queue, apply logic and the outbox.
  - `seed.rs`: fictional demo data (`SAILES_SEED_DEMO=true` or `sailes seed-demo`).
  - `auth.rs`: argon2 passwords, hashed session tokens in `sessions`, cookie `sailes_session` or `Authorization: Bearer`.
- `web`: React 19, TypeScript, Vite, TanStack Query, lucide icons. `src/styles.css` holds all tokens and component styles (aubergine nav, purple actions, gold for anything AI-suggested).

## Proposal kinds

`create_contact`, `update_contact`, `create_company`, `update_company`, `create_deal`, `update_deal`, `add_note`, `create_task`, `send_email`.
Helpers the apply step resolves: `company_domain` / `company_name` to `company_id`, `stage_name` to `stage_id`, `contact_ids` on deals and companies.
Updates need `target_type` + `target_id`. `source_activity_id` gets linked to the resulting record's timeline on approval.

## Testing locally

- Postgres: `DATABASE_URL=postgres://postgres@127.0.0.1:5432/sailes cargo run`
- Dashboard dev server: `cd web && npm run dev` (proxies `/api` to :8080)
- `npm run build` runs the type check (`tsc -b`) before bundling.

## Roadmap

1. Native connectors (Graph, Slack) feeding the same import format, for installs without Claude
2. AI pipeline: activity in, proposals out; provider trait with Claude default, local model option
3. Tone-of-voice profile from sent mail plus draft edits (`proposals.edited`, `feedback`)
4. Follow-up engine (quiet threads, promised actions)
5. Slack, Teams chats, meeting transcripts
