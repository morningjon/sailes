//! The bridge: a shared folder that lets something outside sAIles act as its connector.
//!
//! Today that's a scheduled Claude task using Claude's own Outlook, Teams and Slack
//! connectors, with the folder kept in sync by Syncthing. Any other tool can use it too.
//!
//! ```text
//! <bridge>/
//!   README.md         format reference (written by sAIles)
//!   context/crm.json  what sAIles already knows, so the reader can match and not duplicate (sAIles writes)
//!   inbox/*.json      batches of conversations and suggestions (reader writes, sAIles imports)
//!   inbox/imported/   batches sAIles has imported
//!   inbox/failed/     batches sAIles couldn't read, with a .error.txt next to each
//!   outbox/<id>.json  emails you approved, waiting to be handed to Outlook (sAIles writes)
//!   sent/<id>.json    receipts: drafted, sent or failed (reader writes, sAIles reads and deletes)
//!   state/            the reader's own notes, e.g. its tone-of-voice profile (sAIles ignores it)
//! ```
//!
//! The same batch format is accepted over HTTP at `POST /api/import`.

use std::{
    collections::HashMap,
    path::{Path, PathBuf},
    time::Duration,
};

use axum::{extract::State, Json};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sha2::{Digest, Sha256};
use sqlx::{PgConnection, PgPool};
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    routes::{
        activities::{self, NewActivity},
        proposals::{self, NewProposal},
    },
    AppState,
};

const MAX_BODY_CHARS: usize = 50_000;
const FORMAT_DOC: &str = include_str!("../../../docs/bridge.md");

// Batch format ----------------------------------------------------------------------------

#[derive(Debug, Deserialize)]
pub struct Batch {
    #[serde(default)]
    pub version: Option<u32>,
    /// One line on what this batch covers, shown on the Connections page.
    #[serde(default)]
    pub summary: Option<String>,
    #[serde(default)]
    pub activities: Vec<InActivity>,
    #[serde(default)]
    pub proposals: Vec<InProposal>,
}

#[derive(Debug, Deserialize)]
pub struct InLink {
    pub object_type: String,
    pub object_id: Uuid,
}

#[derive(Debug, Deserialize)]
pub struct InActivity {
    /// Batch-local name so proposals can point at this activity.
    #[serde(rename = "ref")]
    pub reference: Option<String>,
    pub kind: String,
    /// The message's id in its own system (Outlook message id, Slack ts, ...). Used to skip repeats.
    pub external_id: Option<String>,
    pub subject: Option<String>,
    #[serde(default)]
    pub body: String,
    pub direction: Option<String>,
    #[serde(default)]
    pub participants: Option<Value>,
    pub occurred_at: Option<DateTime<Utc>>,
    pub source_url: Option<String>,
    #[serde(default)]
    pub links: Vec<InLink>,
}

#[derive(Debug, Deserialize)]
pub struct InProposal {
    pub kind: String,
    pub title: String,
    pub rationale: Option<String>,
    pub payload: Value,
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
    pub confidence: Option<f32>,
    pub source_channel: Option<String>,
    pub source_excerpt: Option<String>,
    /// The `ref` of an activity in the same batch.
    pub source_ref: Option<String>,
    /// Or an activity already in sAIles.
    pub source_activity_id: Option<Uuid>,
}

#[derive(Debug, Default, Serialize)]
pub struct ImportResult {
    pub activities: i32,
    pub proposals: i32,
    pub skipped: i32,
    pub problems: Vec<String>,
}

async fn object_exists(conn: &mut PgConnection, object_type: &str, id: Uuid) -> AppResult<bool> {
    let table = match object_type {
        "contact" => "contacts",
        "company" => "companies",
        "deal" => "deals",
        _ => return Ok(false),
    };
    Ok(sqlx::query_scalar(&format!(
        "select exists(select 1 from {table} where id = $1)"
    ))
    .bind(id)
    .fetch_one(&mut *conn)
    .await?)
}

fn truncate(s: &str, max: usize) -> String {
    if s.chars().count() <= max {
        s.to_string()
    } else {
        let mut t: String = s.chars().take(max).collect();
        t.push_str("\n\n[truncated]");
        t
    }
}

/// Import one batch in a single transaction. Individual bad items are skipped and reported;
/// they don't stop the rest.
pub async fn import_batch(pool: &PgPool, batch: Batch, source: &str) -> AppResult<ImportResult> {
    if let Some(v) = batch.version {
        if v != 1 {
            return Err(AppError::bad(format!(
                "Unsupported batch version {v}; expected 1."
            )));
        }
    }
    let mut out = ImportResult::default();
    let mut tx = pool.begin().await?;
    // ref -> (activity id, already existed before this batch)
    let mut refs: HashMap<String, (Uuid, bool)> = HashMap::new();

    for (i, a) in batch.activities.into_iter().enumerate() {
        let label = a
            .reference
            .clone()
            .unwrap_or_else(|| format!("activity {}", i + 1));
        let kind: &'static str = match a.kind.as_str() {
            "email" => "email",
            "slack" => "slack",
            "teams" => "teams",
            "meeting" => "meeting",
            "call" => "call",
            "note" => "note",
            other => {
                out.problems
                    .push(format!("{label}: unknown kind '{other}'"));
                continue;
            }
        };
        let direction = a.direction.filter(|d| d == "inbound" || d == "outbound");

        if let Some(ext) = a.external_id.as_deref().filter(|e| !e.is_empty()) {
            let existing: Option<Uuid> = sqlx::query_scalar(
                "select id from activities where kind = $1 and external_id = $2",
            )
            .bind(kind)
            .bind(ext)
            .fetch_optional(&mut *tx)
            .await?;
            if let Some(id) = existing {
                out.skipped += 1;
                if let Some(r) = &a.reference {
                    refs.insert(r.clone(), (id, true));
                }
                continue;
            }
        }

        let mut links: Vec<(&str, Uuid)> = Vec::new();
        for l in &a.links {
            let t: &'static str = match l.object_type.as_str() {
                "contact" => "contact",
                "company" => "company",
                "deal" => "deal",
                _ => {
                    out.problems
                        .push(format!("{label}: can't link to '{}'", l.object_type));
                    continue;
                }
            };
            if object_exists(&mut tx, t, l.object_id).await? {
                links.push((t, l.object_id));
            } else {
                out.problems
                    .push(format!("{label}: linked {t} {} doesn't exist", l.object_id));
            }
        }

        let id = activities::insert(
            &mut tx,
            NewActivity {
                kind,
                subject: a.subject.filter(|s| !s.trim().is_empty()),
                body: truncate(a.body.trim(), MAX_BODY_CHARS),
                direction,
                participants: a.participants,
                external_id: a.external_id.filter(|e| !e.is_empty()),
                source_url: a.source_url,
                occurred_at: a.occurred_at,
                created_by: "connector",
            },
            &links,
        )
        .await?;
        out.activities += 1;
        if let Some(r) = a.reference {
            refs.insert(r, (id, false));
        }

        // Keep "last contacted" current for linked contacts.
        for (t, oid) in &links {
            if *t == "contact" {
                sqlx::query(
                    "update contacts set last_contacted_at = greatest(coalesce(last_contacted_at, $2), $2) where id = $1",
                )
                .bind(oid)
                .bind(a.occurred_at.unwrap_or_else(Utc::now))
                .execute(&mut *tx)
                .await?;
            }
        }
    }

    for (i, p) in batch.proposals.into_iter().enumerate() {
        let label = format!("suggestion {} ({})", i + 1, p.title);
        if !proposals::KINDS.contains(&p.kind.as_str()) {
            out.problems
                .push(format!("{label}: unknown kind '{}'", p.kind));
            continue;
        }
        if !p.payload.is_object() {
            out.problems
                .push(format!("{label}: payload must be an object"));
            continue;
        }

        let mut source_activity_id = p.source_activity_id;
        if let Some(r) = &p.source_ref {
            match refs.get(r) {
                Some((_, true)) => {
                    // The message was already imported in an earlier run, so this is a repeat.
                    out.skipped += 1;
                    continue;
                }
                Some((id, false)) => source_activity_id = Some(*id),
                None => out
                    .problems
                    .push(format!("{label}: source_ref '{r}' isn't in this batch")),
            }
        }

        match (p.target_type.as_deref(), p.target_id) {
            (Some(t), Some(id)) => {
                if !object_exists(&mut tx, t, id).await? {
                    out.problems
                        .push(format!("{label}: target {t} {id} doesn't exist"));
                    continue;
                }
            }
            (None, None) => {
                if p.kind.starts_with("update_") {
                    out.problems
                        .push(format!("{label}: updates need target_type and target_id"));
                    continue;
                }
            }
            _ => {
                out.problems.push(format!(
                    "{label}: give both target_type and target_id, or neither"
                ));
                continue;
            }
        }

        let duplicate: bool = sqlx::query_scalar(
            "select exists(select 1 from proposals where status = 'pending' and kind = $1
                and target_id is not distinct from $2 and lower(title) = lower($3))",
        )
        .bind(&p.kind)
        .bind(p.target_id)
        .bind(&p.title)
        .fetch_one(&mut *tx)
        .await?;
        if duplicate {
            out.skipped += 1;
            continue;
        }

        proposals::insert(
            &mut tx,
            NewProposal {
                kind: p.kind,
                title: p.title,
                rationale: p.rationale,
                payload: p.payload,
                target_type: p.target_type,
                target_id: p.target_id,
                confidence: p.confidence.map(|c| c.clamp(0.0, 1.0)),
                source_channel: p.source_channel,
                source_excerpt: p.source_excerpt.map(|e| truncate(&e, 300)),
                source_activity_id,
            },
        )
        .await?;
        out.proposals += 1;
    }

    sqlx::query(
        "insert into imports (source, activities, proposals, skipped, error, summary) values ($1, $2, $3, $4, $5, $6)",
    )
    .bind(source)
    .bind(out.activities)
    .bind(out.proposals)
    .bind(out.skipped)
    .bind((!out.problems.is_empty()).then(|| out.problems.join("\n")))
    .bind(batch.summary)
    .execute(&mut *tx)
    .await?;

    tx.commit().await?;
    Ok(out)
}

pub async fn import_api(
    State(state): State<AppState>,
    Json(batch): Json<Batch>,
) -> AppResult<Json<ImportResult>> {
    Ok(Json(import_batch(&state.pool, batch, "api").await?))
}

// Context export ----------------------------------------------------------------------------

/// Everything a reader needs to match people and deals, avoid repeats, and learn from feedback.
pub async fn context(pool: &PgPool) -> AppResult<Value> {
    let v: Value = sqlx::query_scalar(
        r#"select jsonb_build_object(
            'version', 1,
            'owner', (select jsonb_build_object('name', name, 'email', email) from users order by created_at limit 1),
            'pipelines', coalesce((select jsonb_agg(jsonb_build_object('id', p.id, 'name', p.name, 'stages',
                (select jsonb_agg(jsonb_build_object('id', s.id, 'name', s.name, 'probability', s.probability,
                    'is_closed', s.is_closed, 'is_won', s.is_won) order by s.position)
                 from pipeline_stages s where s.pipeline_id = p.id)) order by p.position) from pipelines p), '[]'),
            'companies', coalesce((select jsonb_agg(jsonb_build_object('id', id, 'name', name, 'domain', domain,
                'lifecycle_stage', lifecycle_stage) order by name) from companies), '[]'),
            'contacts', coalesce((select jsonb_agg(jsonb_build_object('id', c.id,
                'name', trim(c.first_name || ' ' || c.last_name), 'email', c.email, 'job_title', c.job_title,
                'company_id', c.company_id, 'last_contacted_at', c.last_contacted_at) order by c.first_name)
              from contacts c), '[]'),
            'deals', coalesce((select jsonb_agg(jsonb_build_object('id', d.id, 'name', d.name, 'amount', d.amount,
                'currency', d.currency, 'stage_id', d.stage_id, 'stage', s.name, 'is_closed', s.is_closed,
                'close_date', d.close_date, 'next_step', d.next_step, 'company_id', d.company_id,
                'contact_ids', coalesce((select jsonb_agg(contact_id) from deal_contacts where deal_id = d.id), '[]'),
                'last_activity_at', (select max(a.occurred_at) from activity_links l join activities a on a.id = l.activity_id
                    where l.object_type = 'deal' and l.object_id = d.id)) order by d.name)
              from deals d join pipeline_stages s on s.id = d.stage_id
              where not s.is_closed or d.updated_at > now() - interval '90 days'), '[]'),
            'open_tasks', coalesce((select jsonb_agg(jsonb_build_object('id', id, 'title', title, 'due_at', due_at,
                'contact_id', contact_id, 'company_id', company_id, 'deal_id', deal_id) order by due_at nulls last)
              from tasks where status = 'open'), '[]'),
            'pending_suggestions', coalesce((select jsonb_agg(jsonb_build_object('kind', kind, 'title', title,
                'target_type', target_type, 'target_id', target_id) order by created_at)
              from proposals where status = 'pending'), '[]'),
            'cursors', coalesce((select jsonb_object_agg(kind, latest) from (
                select kind, max(occurred_at) as latest from activities
                where created_by = 'connector' group by kind) c), '{}'),
            'feedback', jsonb_build_object(
              'rejected', coalesce((select jsonb_agg(x) from (
                  select jsonb_build_object('kind', kind, 'title', title, 'feedback', feedback, 'payload', payload) as x
                  from proposals where status = 'rejected' order by decided_at desc limit 40) r), '[]'),
              'edited', coalesce((select jsonb_agg(x) from (
                  select jsonb_build_object('kind', kind, 'title', title, 'suggested', original_payload, 'approved', payload) as x
                  from proposals where status = 'approved' and edited and original_payload is not null
                  order by decided_at desc limit 25) e), '[]'),
              'approved_emails', coalesce((select jsonb_agg(x) from (
                  select jsonb_build_object('subject', payload->>'subject', 'body', payload->>'body') as x
                  from proposals where status = 'approved' and kind = 'send_email'
                  order by decided_at desc limit 15) m), '[]')
            )
        )"#,
    )
    .fetch_one(pool)
    .await?;
    Ok(v)
}

pub async fn context_api(State(state): State<AppState>) -> AppResult<Json<Value>> {
    Ok(Json(context(&state.pool).await?))
}

// Folder sync ----------------------------------------------------------------------------------

pub struct Bridge {
    root: PathBuf,
    last_context_hash: Option<String>,
}

fn write_atomic(path: &Path, contents: &[u8]) -> std::io::Result<()> {
    let tmp = path.with_extension("tmp~");
    std::fs::write(&tmp, contents)?;
    std::fs::rename(&tmp, path)
}

fn is_batch_file(p: &Path) -> bool {
    let name = p.file_name().and_then(|n| n.to_str()).unwrap_or("");
    p.is_file()
        && name.ends_with(".json")
        && !name.starts_with('.')
        && !name.contains(".sync-conflict")
        && !name.starts_with("~")
}

impl Bridge {
    pub fn new(root: PathBuf) -> std::io::Result<Self> {
        for d in [
            "context",
            "inbox",
            "inbox/imported",
            "inbox/failed",
            "outbox",
            "sent",
            "state",
        ] {
            std::fs::create_dir_all(root.join(d))?;
        }
        write_atomic(&root.join("README.md"), FORMAT_DOC.as_bytes())?;
        Ok(Self {
            root,
            last_context_hash: None,
        })
    }

    pub async fn tick(&mut self, pool: &PgPool) -> anyhow::Result<()> {
        self.import_inbox(pool).await?;
        self.read_receipts(pool).await?;
        self.write_outbox(pool).await?;
        self.write_context(pool).await?;
        Ok(())
    }

    async fn import_inbox(&self, pool: &PgPool) -> anyhow::Result<()> {
        let inbox = self.root.join("inbox");
        let mut files: Vec<PathBuf> = std::fs::read_dir(&inbox)?
            .filter_map(|e| e.ok().map(|e| e.path()))
            .filter(|p| is_batch_file(p))
            .collect();
        files.sort();
        for path in files {
            let name = path.file_name().unwrap().to_string_lossy().to_string();
            let parsed = std::fs::read(&path)
                .map_err(|e| e.to_string())
                .and_then(|bytes| {
                    serde_json::from_slice::<Batch>(&bytes).map_err(|e| e.to_string())
                });
            match parsed {
                Ok(batch) => match import_batch(pool, batch, &name).await {
                    Ok(r) => {
                        tracing::info!(file = %name, activities = r.activities, proposals = r.proposals,
                            skipped = r.skipped, problems = r.problems.len(), "imported batch");
                        if !r.problems.is_empty() {
                            let _ = std::fs::write(
                                inbox.join("imported").join(format!("{name}.problems.txt")),
                                r.problems.join("\n"),
                            );
                        }
                        std::fs::rename(&path, inbox.join("imported").join(&name))?;
                    }
                    Err(e) => {
                        tracing::warn!(file = %name, error = %e, "batch import failed");
                        self.fail(&path, &name, &e.to_string(), pool).await?;
                    }
                },
                Err(e) => {
                    tracing::warn!(file = %name, error = %e, "couldn't read batch");
                    self.fail(
                        &path,
                        &name,
                        &format!("Couldn't read this file as a batch: {e}"),
                        pool,
                    )
                    .await?;
                }
            }
        }
        Ok(())
    }

    async fn fail(
        &self,
        path: &Path,
        name: &str,
        error: &str,
        pool: &PgPool,
    ) -> anyhow::Result<()> {
        let failed = self.root.join("inbox/failed");
        std::fs::write(failed.join(format!("{name}.error.txt")), error)?;
        std::fs::rename(path, failed.join(name))?;
        sqlx::query("insert into imports (source, error) values ($1, $2)")
            .bind(name)
            .bind(error)
            .execute(pool)
            .await?;
        Ok(())
    }

    async fn read_receipts(&self, pool: &PgPool) -> anyhow::Result<()> {
        #[derive(Deserialize)]
        struct Receipt {
            id: Uuid,
            status: String,
            external_id: Option<String>,
            external_url: Option<String>,
            error: Option<String>,
            at: Option<DateTime<Utc>>,
        }
        let dir = self.root.join("sent");
        for entry in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
            let path = entry.path();
            if !is_batch_file(&path) {
                continue;
            }
            let receipt: Receipt = match std::fs::read(&path)
                .ok()
                .and_then(|b| serde_json::from_slice(&b).ok())
            {
                Some(r) => r,
                None => {
                    tracing::warn!(file = %path.display(), "unreadable receipt");
                    continue;
                }
            };
            let status = match receipt.status.as_str() {
                "drafted" | "sent" | "failed" => receipt.status.as_str(),
                other => {
                    tracing::warn!(status = other, "unknown receipt status");
                    continue;
                }
            };
            let at = receipt.at.unwrap_or_else(Utc::now);
            let mut tx = pool.begin().await?;
            let row: Option<(Option<Uuid>, Option<Uuid>, Option<String>, String, Value)> = sqlx::query_as(
                "update outbox set status = $2, external_id = coalesce($3, external_id), external_url = coalesce($4, external_url),
                    error = $5, handed_off_at = $6, sent_at = case when $2 = 'sent' then $6 else sent_at end
                 where id = $1 and status in ('queued', 'drafted', 'failed')
                 returning contact_id, deal_id, subject, body, recipients",
            )
            .bind(receipt.id)
            .bind(status)
            .bind(&receipt.external_id)
            .bind(&receipt.external_url)
            .bind(&receipt.error)
            .bind(at)
            .fetch_optional(&mut *tx)
            .await?;
            if let (Some((contact_id, deal_id, subject, body, recipients)), "sent") = (&row, status)
            {
                // Sent straight away: log it now. Drafts get logged when the sent mail is next imported.
                let mut links = Vec::new();
                if let Some(c) = contact_id {
                    links.push(("contact", *c));
                }
                if let Some(d) = deal_id {
                    links.push(("deal", *d));
                }
                activities::insert(
                    &mut tx,
                    NewActivity {
                        kind: "email",
                        subject: subject.clone(),
                        body: body.clone(),
                        direction: Some("outbound".into()),
                        participants: Some(recipients.clone()),
                        external_id: receipt.external_id.clone(),
                        occurred_at: Some(at),
                        created_by: "connector",
                        ..Default::default()
                    },
                    &links,
                )
                .await?;
            }
            tx.commit().await?;
            let _ = std::fs::remove_file(&path);
            if status != "failed" {
                let _ = std::fs::remove_file(
                    self.root
                        .join("outbox")
                        .join(format!("{}.json", receipt.id)),
                );
            }
        }
        Ok(())
    }

    async fn write_outbox(&self, pool: &PgPool) -> anyhow::Result<()> {
        let items: Vec<(Uuid, Value)> = sqlx::query_as(
            "select o.id, jsonb_build_object('id', o.id, 'to', o.recipients, 'cc', o.cc, 'subject', o.subject,
                'body', o.body, 'in_reply_to', o.in_reply_to,
                'contact', case when c.id is null then null else jsonb_build_object('id', c.id,
                    'name', trim(c.first_name || ' ' || c.last_name), 'email', c.email) end,
                'deal', case when d.id is null then null else jsonb_build_object('id', d.id, 'name', d.name) end,
                'approved_at', o.created_at)
             from outbox o left join contacts c on c.id = o.contact_id left join deals d on d.id = o.deal_id
             where o.status = 'queued' and o.channel = 'email'",
        )
        .fetch_all(pool)
        .await?;
        let dir = self.root.join("outbox");
        let wanted: std::collections::HashSet<String> =
            items.iter().map(|(id, _)| format!("{id}.json")).collect();
        for (id, v) in &items {
            let path = dir.join(format!("{id}.json"));
            if !path.exists() {
                write_atomic(&path, serde_json::to_string_pretty(v)?.as_bytes())?;
            }
        }
        // Remove files for emails that were cancelled or already handled.
        for entry in std::fs::read_dir(&dir)?.filter_map(|e| e.ok()) {
            let name = entry.file_name().to_string_lossy().to_string();
            if name.ends_with(".json") && !wanted.contains(&name) {
                let _ = std::fs::remove_file(entry.path());
            }
        }
        Ok(())
    }

    async fn write_context(&mut self, pool: &PgPool) -> anyhow::Result<()> {
        let ctx = context(pool).await?;
        let body = serde_json::to_string_pretty(&ctx)?;
        let hash = hex::encode(Sha256::digest(body.as_bytes()));
        if self.last_context_hash.as_deref() == Some(hash.as_str())
            && self.root.join("context/crm.json").exists()
        {
            return Ok(());
        }
        let mut with_time = ctx;
        with_time["generated_at"] = json!(Utc::now());
        write_atomic(
            &self.root.join("context/crm.json"),
            serde_json::to_string_pretty(&with_time)?.as_bytes(),
        )?;
        self.last_context_hash = Some(hash);
        Ok(())
    }
}

pub fn spawn(pool: PgPool, root: PathBuf, every: Duration) -> anyhow::Result<()> {
    let mut bridge = Bridge::new(root.clone()).map_err(|e| {
        anyhow::anyhow!("couldn't set up the bridge folder {}: {e}", root.display())
    })?;
    tracing::info!(dir = %root.display(), "bridge folder enabled");
    tokio::spawn(async move {
        loop {
            if let Err(e) = bridge.tick(&pool).await {
                tracing::warn!(error = %e, "bridge sync failed");
            }
            tokio::time::sleep(every).await;
        }
    });
    Ok(())
}

// Status for the dashboard --------------------------------------------------------------------

pub async fn status(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let v: Value = sqlx::query_scalar(
        "select jsonb_build_object(
            'recent', coalesce((select jsonb_agg(x) from (
                select jsonb_build_object('source', source, 'activities', activities, 'proposals', proposals,
                    'skipped', skipped, 'error', error, 'summary', summary, 'created_at', created_at) as x
                from imports order by created_at desc limit 8) r), '[]'),
            'outbox', jsonb_build_object(
                'queued', (select count(*) from outbox where status = 'queued'),
                'drafted', (select count(*) from outbox where status = 'drafted'),
                'failed', (select count(*) from outbox where status = 'failed'))
        )",
    )
    .fetch_one(&state.pool)
    .await?;
    let mut v = v;
    v["folder_enabled"] = json!(state.config.bridge_dir.is_some());
    Ok(Json(v))
}
