# sAIles bridge folder

This folder connects sAIles to whatever reads your conversations for it. Today that's a daily Claude task using Claude's Outlook, Teams and Slack connectors; the folder is kept in sync between machines with Syncthing.

sAIles rewrites this file on every start, so don't edit it here. The source is `docs/bridge.md` in the sAIles repo.

## Layout

| Path | Written by | What it is |
| --- | --- | --- |
| `context/crm.json` | sAIles | What sAIles already knows. Read it first. |
| `inbox/*.json` | reader | Batches of conversations and suggestions for sAIles to import. |
| `inbox/imported/` | sAIles | Batches already imported. A `.problems.txt` sits next to any batch where some items were skipped. |
| `inbox/failed/` | sAIles | Batches that couldn't be read, each with a `.error.txt`. |
| `outbox/<id>.json` | sAIles | Emails the user approved, waiting to go to Outlook. |
| `sent/<id>.json` | reader | Receipts for outbox emails. sAIles reads and deletes them. |
| `state/` | reader | The reader's own notes between runs (for example a tone-of-voice profile). sAIles ignores it. |

Write files whole. Name batch files so they sort by time, for example `inbox/2026-10-13T0745-claude.json`.

## context/crm.json

```jsonc
{
  "version": 1,
  "generated_at": "2026-10-13T06:40:00Z",
  "owner": { "name": "Jon Wray", "email": "jon@example.com" },   // whose mailbox this is
  "pipelines": [{ "id": "…", "name": "Sales pipeline",
                  "stages": [{ "id": "…", "name": "Discovery", "probability": 10, "is_closed": false, "is_won": false }] }],
  "companies": [{ "id": "…", "name": "Ferrous Labs", "domain": "ferrouslabs.io", "lifecycle_stage": "opportunity" }],
  "contacts":  [{ "id": "…", "name": "Priya Natarajan", "email": "priya@ferrouslabs.io", "job_title": "CEO",
                  "company_id": "…", "last_contacted_at": "…" }],
  "deals":     [{ "id": "…", "name": "…", "amount": 14500, "currency": "GBP", "stage_id": "…", "stage": "Proposal sent",
                  "is_closed": false, "close_date": "2026-11-02", "next_step": "…", "company_id": "…",
                  "contact_ids": ["…"], "last_activity_at": "…" }],
  "open_tasks": [{ "id": "…", "title": "…", "due_at": "…", "contact_id": "…", "company_id": null, "deal_id": "…" }],
  "pending_suggestions": [{ "kind": "send_email", "title": "…", "target_type": "contact", "target_id": "…" }],
  "cursors": { "email": "2026-10-12T17:02:00Z", "slack": "…", "teams": "…", "meeting": "…" },  // newest imported message per kind
  "feedback": {
    "rejected": [{ "kind": "…", "title": "…", "feedback": "Automated sender, ignore these.", "payload": { } }],
    "edited":   [{ "kind": "send_email", "title": "…", "suggested": { }, "approved": { } }],  // what the user changed
    "approved_emails": [{ "subject": "…", "body": "…" }]                                    // drafts the user was happy with
  }
}
```

## inbox batch

```jsonc
{
  "version": 1,
  "summary": "13 Oct: 9 emails, 4 Slack messages, 1 meeting",
  "activities": [
    {
      "ref": "m1",                         // batch-local name, used by suggestions below
      "kind": "email",                     // email | slack | teams | meeting | call | note
      "external_id": "AAMkAG…",            // the message's id in its own system; repeats are skipped
      "subject": "Re: Migration plan",
      "body": "Plain text of the message or transcript excerpt",
      "direction": "inbound",              // inbound | outbound
      "participants": ["gareth.lewis@quaysidefreight.com"],
      "occurred_at": "2026-10-12T15:04:00Z",
      "source_url": "https://outlook.office.com/…",
      "links": [{ "object_type": "contact", "object_id": "…" }, { "object_type": "deal", "object_id": "…" }]
    }
  ],
  "proposals": [
    {
      "kind": "create_contact",
      "title": "Add Nadia Kowalski from Quayside Freight",          // short, starts with a verb
      "rationale": "New person on the migration thread; her signature gives role and phone.",
      "payload": { "first_name": "Nadia", "last_name": "Kowalski", "email": "nadia@quaysidefreight.com",
                   "job_title": "Head of IT", "company_domain": "quaysidefreight.com", "lifecycle_stage": "opportunity" },
      "confidence": 0.95,
      "source_channel": "outlook",          // outlook | slack | teams | meeting
      "source_excerpt": "I'll be the technical owner for the migration",   // under 25 words, verbatim
      "source_ref": "m1"
    }
  ]
}
```

Only link activities to records that exist in `crm.json`. Messages about people who aren't in the CRM yet can go in unlinked; approving a suggestion that names the message as its `source_ref` links it to the new record.

### Suggestion kinds and payloads

Updates need `target_type` and `target_id`, and their payload holds only the fields that change.

| kind | target | payload fields |
| --- | --- | --- |
| `create_contact` | none | `first_name`, `last_name`, `email`, `phone`, `job_title`, `company_id` or `company_domain` or `company_name`, `lifecycle_stage`, `lead_status` |
| `update_contact` | contact | any of the above |
| `create_company` | none | `name` (required), `domain`, `industry`, `employees`, `city`, `country`, `phone`, `description`, `lifecycle_stage`, `contact_ids` (existing contacts to attach) |
| `update_company` | company | any of the above except `contact_ids` |
| `create_deal` | none | `name` (required), `amount`, `currency`, `stage_id` or `stage_name`, `close_date` (YYYY-MM-DD), `company_id` or `company_domain`, `contact_ids`, `next_step`, `priority` |
| `update_deal` | deal | any of the above; changing `stage_id`/`stage_name` is logged on the deal |
| `add_note` | contact, company or deal | `body` (required), `subject`, `links` (extra `{object_type, object_id}`) |
| `create_task` | contact, company or deal (optional) | `title` (required), `notes`, `kind` (todo, follow_up, call, email), `priority` (low, medium, high), `due_at` (RFC 3339), `contact_id`, `company_id`, `deal_id` |
| `send_email` | contact or deal | `to` (list, required), `cc`, `subject`, `body` (plain text, required), `in_reply_to` (Outlook message id), `contact_id`, `deal_id` |

`lifecycle_stage` is one of subscriber, lead, opportunity, customer, evangelist, other.
For `send_email`, when `source_ref` points at an email and `in_reply_to` is empty, sAIles fills `in_reply_to` with that email's `external_id` so the reply threads.

## outbox email

```jsonc
{
  "id": "…",
  "to": ["priya@ferrouslabs.io"], "cc": [],
  "subject": "Re: CRM setup proposal",
  "body": "Plain text",
  "in_reply_to": "AAMkAG…",              // Outlook message id to reply to, or null
  "contact": { "id": "…", "name": "Priya Natarajan", "email": "…" },
  "deal": { "id": "…", "name": "…" },
  "approved_at": "…"
}
```

## sent receipt

Write `sent/<same id>.json`:

```jsonc
{ "id": "…", "status": "drafted", "external_id": "AAMkAG…", "external_url": "https://outlook.office.com/…",
  "error": null, "at": "2026-10-13T06:45:00Z" }
```

`status` is `drafted` (in Outlook Drafts, waiting for the user to press send), `sent`, or `failed` (with `error`). Failed emails can be retried from the Outbox in sAIles.
