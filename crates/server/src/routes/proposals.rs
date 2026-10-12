//! The approval queue.
//!
//! Connectors and the AI pipeline never write to CRM tables directly. They create proposals:
//! "create this contact", "move this deal to Negotiation", "send this follow-up". A person
//! approves (optionally after editing), or rejects with feedback. Approval applies the change
//! in a single transaction and links the source message into the record's timeline.

use std::collections::{BTreeSet, HashMap};

use axum::{
    extract::{Path, Query, State},
    Extension, Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use super::{activities, as_object, deals};
use crate::{
    auth::User,
    error::{AppError, AppResult},
    fields::{insert_object, snapshot, update_object, COMPANY, CONTACT, TASK},
    AppState,
};

pub const KINDS: &[&str] = &[
    "create_contact",
    "update_contact",
    "create_company",
    "update_company",
    "create_deal",
    "update_deal",
    "add_note",
    "create_task",
    "send_email",
];

#[derive(Debug, Clone, Serialize, sqlx::FromRow)]
pub struct ProposalRow {
    pub id: Uuid,
    pub kind: String,
    pub status: String,
    pub title: String,
    pub rationale: Option<String>,
    pub payload: Value,
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
    pub confidence: Option<f32>,
    pub source_channel: Option<String>,
    pub source_excerpt: Option<String>,
    pub source_activity_id: Option<Uuid>,
    pub result_id: Option<Uuid>,
    pub error: Option<String>,
    pub feedback: Option<String>,
    pub edited: bool,
    pub decided_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
}

#[derive(Debug, Serialize)]
pub struct ProposalView {
    #[serde(flatten)]
    pub row: ProposalRow,
    /// Current values of the target record, for showing what an update changes.
    pub current: Option<Value>,
    /// Display name of the target record.
    pub target_name: Option<String>,
    /// Names for any record ids that appear in the payload or current values.
    pub refs: HashMap<String, String>,
    /// The source message, when the proposal came from one.
    pub source: Option<Value>,
}

const SELECT: &str = "select id, kind, status, title, rationale, payload, target_type, target_id, confidence,
    source_channel, source_excerpt, source_activity_id, result_id, error, feedback, edited, decided_at, created_at
  from proposals";

fn collect_ids(v: &Value, out: &mut BTreeSet<Uuid>) {
    match v {
        Value::String(s) => {
            if s.len() == 36 {
                if let Ok(u) = Uuid::parse_str(s) {
                    out.insert(u);
                }
            }
        }
        Value::Array(items) => items.iter().for_each(|i| collect_ids(i, out)),
        Value::Object(m) => m.values().for_each(|i| collect_ids(i, out)),
        _ => {}
    }
}

async fn view(pool: &sqlx::PgPool, row: ProposalRow) -> AppResult<ProposalView> {
    let mut conn = pool.acquire().await?;
    let (current, target_name) = match (row.target_type.as_deref(), row.target_id) {
        (Some(t), Some(id)) => {
            let spec = crate::fields::spec_for(t)?;
            let snap = snapshot(&mut conn, spec, id).await?;
            let name = snap.as_ref().map(|s| match t {
                "contact" => format!(
                    "{} {}",
                    s["first_name"].as_str().unwrap_or(""),
                    s["last_name"].as_str().unwrap_or("")
                )
                .trim()
                .to_string(),
                _ => s["name"].as_str().unwrap_or("").to_string(),
            });
            (snap, name)
        }
        _ => (None, None),
    };

    let mut ids = BTreeSet::new();
    collect_ids(&row.payload, &mut ids);
    if let Some(c) = &current {
        collect_ids(c, &mut ids);
    }
    ids.remove(&row.id);
    let refs: HashMap<String, String> = if ids.is_empty() {
        HashMap::new()
    } else {
        let rows: Vec<(Uuid, String)> = sqlx::query_as(
            "select id, name from (
                select id, name from companies
                union all select id, trim(first_name || ' ' || last_name) from contacts
                union all select id, name from deals
                union all select id, name from pipeline_stages
                union all select id, name from pipelines
                union all select id, name from users
             ) x where id = any($1)",
        )
        .bind(ids.into_iter().collect::<Vec<_>>())
        .fetch_all(&mut *conn)
        .await?;
        rows.into_iter()
            .map(|(id, name)| (id.to_string(), name))
            .collect()
    };

    let source: Option<Value> = match row.source_activity_id {
        Some(aid) => {
            sqlx::query_scalar(
                "select jsonb_build_object('id', id, 'kind', kind, 'subject', subject, 'body', body,
                    'direction', direction, 'participants', participants, 'occurred_at', occurred_at,
                    'source_url', source_url)
                 from activities where id = $1",
            )
            .bind(aid)
            .fetch_optional(&mut *conn)
            .await?
        }
        None => None,
    };

    Ok(ProposalView {
        row,
        current,
        target_name,
        refs,
        source,
    })
}

#[derive(Deserialize)]
pub struct ProposalQuery {
    status: Option<String>,
    kind: Option<String>,
    limit: Option<i64>,
}

#[derive(Serialize)]
pub struct ProposalList {
    items: Vec<ProposalView>,
    counts: HashMap<String, i64>,
}

pub async fn list(
    State(state): State<AppState>,
    Query(q): Query<ProposalQuery>,
) -> AppResult<Json<ProposalList>> {
    let status = q.status.unwrap_or_else(|| "pending".into());
    let kind = q.kind.filter(|k| !k.is_empty() && k != "all");
    let order = if status == "pending" {
        "created_at desc"
    } else {
        "decided_at desc nulls last"
    };
    let rows: Vec<ProposalRow> = sqlx::query_as(&format!(
        "{SELECT} where status = $1 and ($2::text is null or kind = $2) order by {order} limit $3"
    ))
    .bind(&status)
    .bind(kind)
    .bind(q.limit.unwrap_or(100).clamp(1, 500))
    .fetch_all(&state.pool)
    .await?;
    let mut items = Vec::with_capacity(rows.len());
    for r in rows {
        items.push(view(&state.pool, r).await?);
    }
    let counts: Vec<(String, i64)> =
        sqlx::query_as("select kind, count(*) from proposals where status = $1 group by kind")
            .bind(&status)
            .fetch_all(&state.pool)
            .await?;
    Ok(Json(ProposalList {
        items,
        counts: counts.into_iter().collect(),
    }))
}

async fn fetch_row(pool: &sqlx::PgPool, id: Uuid) -> AppResult<ProposalRow> {
    sqlx::query_as(&format!("{SELECT} where id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn get(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<ProposalView>> {
    let row = fetch_row(&state.pool, id).await?;
    Ok(Json(view(&state.pool, row).await?))
}

#[derive(Deserialize)]
pub struct NewProposal {
    pub kind: String,
    pub title: String,
    pub rationale: Option<String>,
    pub payload: Value,
    pub target_type: Option<String>,
    pub target_id: Option<Uuid>,
    pub confidence: Option<f32>,
    pub source_channel: Option<String>,
    pub source_excerpt: Option<String>,
    pub source_activity_id: Option<Uuid>,
}

pub async fn insert(conn: &mut PgConnection, p: NewProposal) -> AppResult<Uuid> {
    if !KINDS.contains(&p.kind.as_str()) {
        return Err(AppError::bad(format!("Unknown proposal kind: {}", p.kind)));
    }
    if !p.payload.is_object() {
        return Err(AppError::bad("payload must be an object"));
    }
    if p.kind.starts_with("update_") && (p.target_id.is_none() || p.target_type.is_none()) {
        return Err(AppError::bad(
            "Update proposals need a target_type and target_id.",
        ));
    }
    let id: Uuid = sqlx::query_scalar(
        "insert into proposals (kind, title, rationale, payload, target_type, target_id, confidence,
            source_channel, source_excerpt, source_activity_id)
         values ($1, $2, $3, $4, $5, $6, $7, $8, $9, $10) returning id",
    )
    .bind(&p.kind)
    .bind(&p.title)
    .bind(&p.rationale)
    .bind(&p.payload)
    .bind(&p.target_type)
    .bind(p.target_id)
    .bind(p.confidence)
    .bind(&p.source_channel)
    .bind(&p.source_excerpt)
    .bind(p.source_activity_id)
    .fetch_one(&mut *conn)
    .await?;
    Ok(id)
}

pub async fn create(
    State(state): State<AppState>,
    Json(p): Json<NewProposal>,
) -> AppResult<Json<ProposalView>> {
    let mut conn = state.pool.acquire().await?;
    let id = insert(&mut conn, p).await?;
    drop(conn);
    let row = fetch_row(&state.pool, id).await?;
    Ok(Json(view(&state.pool, row).await?))
}

#[derive(Deserialize)]
pub struct EditBody {
    payload: Value,
}

/// Save edits to a pending proposal without deciding on it.
pub async fn edit(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(b): Json<EditBody>,
) -> AppResult<Json<ProposalView>> {
    as_object(b.payload.clone())?;
    let res = sqlx::query(
        "update proposals set payload = $2, edited = true where id = $1 and status = 'pending'",
    )
    .bind(id)
    .bind(&b.payload)
    .execute(&state.pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::bad("Only pending suggestions can be edited."));
    }
    let row = fetch_row(&state.pool, id).await?;
    Ok(Json(view(&state.pool, row).await?))
}

fn decide_body(bytes: &[u8]) -> AppResult<DecideBody> {
    if bytes.iter().all(u8::is_ascii_whitespace) {
        return Ok(DecideBody::default());
    }
    serde_json::from_slice(bytes).map_err(|e| AppError::bad(format!("Invalid request body: {e}")))
}

#[derive(Deserialize, Default)]
pub struct DecideBody {
    /// Edited payload to apply instead of the stored one.
    payload: Option<Value>,
    feedback: Option<String>,
}

pub async fn approve(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
    body: axum::body::Bytes,
) -> AppResult<Json<ProposalView>> {
    let body = decide_body(&body)?;
    approve_one(&state.pool, &user, id, body.payload).await?;
    let row = fetch_row(&state.pool, id).await?;
    Ok(Json(view(&state.pool, row).await?))
}

async fn approve_one(
    pool: &sqlx::PgPool,
    user: &User,
    id: Uuid,
    edited: Option<Value>,
) -> AppResult<()> {
    let mut tx = pool.begin().await?;
    let row: ProposalRow = sqlx::query_as(&format!("{SELECT} where id = $1 for update"))
        .bind(id)
        .fetch_optional(&mut *tx)
        .await?
        .ok_or(AppError::NotFound)?;
    if row.status != "pending" {
        return Err(AppError::bad("This suggestion has already been decided."));
    }
    let was_edited = edited.as_ref().map(|e| e != &row.payload).unwrap_or(false);
    let payload = as_object(edited.unwrap_or_else(|| row.payload.clone()))?;
    let result_id = apply(&mut tx, &row, payload.clone()).await?;
    sqlx::query(
        "update proposals set status = 'approved', payload = $2, edited = edited or $3, result_id = $4,
            decided_by = $5, decided_at = now(), error = null where id = $1",
    )
    .bind(id)
    .bind(Value::Object(payload))
    .bind(was_edited)
    .bind(result_id)
    .bind(user.id)
    .execute(&mut *tx)
    .await?;
    tx.commit().await?;
    Ok(())
}

pub async fn reject(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Path(id): Path<Uuid>,
    body: axum::body::Bytes,
) -> AppResult<Json<ProposalView>> {
    let body = decide_body(&body)?;
    reject_one(&state.pool, &user, id, body.feedback).await?;
    let row = fetch_row(&state.pool, id).await?;
    Ok(Json(view(&state.pool, row).await?))
}

async fn reject_one(
    pool: &sqlx::PgPool,
    user: &User,
    id: Uuid,
    feedback: Option<String>,
) -> AppResult<()> {
    let res = sqlx::query(
        "update proposals set status = 'rejected', feedback = $2, decided_by = $3, decided_at = now()
         where id = $1 and status = 'pending'",
    )
    .bind(id)
    .bind(feedback.filter(|f| !f.trim().is_empty()))
    .bind(user.id)
    .execute(pool)
    .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::bad(
            "This suggestion has already been decided, or doesn't exist.",
        ));
    }
    Ok(())
}

#[derive(Deserialize)]
pub struct BulkBody {
    ids: Vec<Uuid>,
    action: String,
}

pub async fn bulk(
    State(state): State<AppState>,
    Extension(user): Extension<User>,
    Json(b): Json<BulkBody>,
) -> AppResult<Json<Value>> {
    let mut results = Vec::new();
    for id in b.ids {
        let r = match b.action.as_str() {
            "approve" => approve_one(&state.pool, &user, id, None).await,
            "reject" => reject_one(&state.pool, &user, id, None).await,
            _ => return Err(AppError::bad("action must be approve or reject")),
        };
        results.push(match r {
            Ok(()) => json!({ "id": id, "ok": true }),
            Err(e) => json!({ "id": id, "ok": false, "error": e.to_string() }),
        });
    }
    Ok(Json(json!({ "results": results })))
}

// Applying proposals --------------------------------------------------------------------

fn take_str(map: &mut Map<String, Value>, key: &str) -> Option<String> {
    map.remove(key)
        .and_then(|v| v.as_str().map(|s| s.trim().to_string()))
        .filter(|s| !s.is_empty())
}

/// Resolve `company_domain` / `company_name` helpers into `company_id`.
async fn resolve_company(conn: &mut PgConnection, map: &mut Map<String, Value>) -> AppResult<()> {
    let domain = take_str(map, "company_domain");
    let name = take_str(map, "company_name");
    if map.get("company_id").map(|v| !v.is_null()).unwrap_or(false) {
        return Ok(());
    }
    let found: Option<Uuid> = if let Some(d) = domain {
        sqlx::query_scalar("select id from companies where lower(domain) = lower($1)")
            .bind(d)
            .fetch_optional(&mut *conn)
            .await?
    } else if let Some(n) = name {
        sqlx::query_scalar("select id from companies where lower(name) = lower($1) limit 1")
            .bind(n)
            .fetch_optional(&mut *conn)
            .await?
    } else {
        None
    };
    if let Some(id) = found {
        map.insert("company_id".into(), json!(id));
    }
    Ok(())
}

/// Resolve a `stage_name` helper into `stage_id` within the deal's (or default) pipeline.
async fn resolve_stage(
    conn: &mut PgConnection,
    map: &mut Map<String, Value>,
    deal_id: Option<Uuid>,
) -> AppResult<()> {
    let Some(stage_name) = take_str(map, "stage_name") else {
        return Ok(());
    };
    if map.contains_key("stage_id") {
        return Ok(());
    }
    let sid: Option<Uuid> = sqlx::query_scalar(
        "select s.id from pipeline_stages s
         where lower(s.name) = lower($1)
           and s.pipeline_id = coalesce((select pipeline_id from deals where id = $2),
                                        (select id from pipelines order by position, created_at limit 1))",
    )
    .bind(&stage_name)
    .bind(deal_id)
    .fetch_optional(&mut *conn)
    .await?;
    let sid =
        sid.ok_or_else(|| AppError::bad(format!("There's no deal stage called {stage_name}.")))?;
    map.insert("stage_id".into(), json!(sid));
    Ok(())
}

async fn link_source(
    conn: &mut PgConnection,
    row: &ProposalRow,
    links: &[(&str, Uuid)],
) -> AppResult<()> {
    if let Some(aid) = row.source_activity_id {
        activities::link(conn, aid, links).await?;
    }
    Ok(())
}

fn default_source(map: &mut Map<String, Value>, row: &ProposalRow) {
    if !map.contains_key("source") {
        if let Some(ch) = &row.source_channel {
            map.insert("source".into(), json!(ch));
        }
    }
}

fn require_target(row: &ProposalRow, expected: &str) -> AppResult<Uuid> {
    match (row.target_type.as_deref(), row.target_id) {
        (Some(t), Some(id)) if t == expected => Ok(id),
        _ => Err(AppError::bad(format!(
            "This suggestion needs a target {expected}."
        ))),
    }
}

async fn apply(
    conn: &mut PgConnection,
    row: &ProposalRow,
    mut map: Map<String, Value>,
) -> AppResult<Option<Uuid>> {
    match row.kind.as_str() {
        "create_contact" => {
            resolve_company(conn, &mut map).await?;
            default_source(&mut map, row);
            let id = insert_object(conn, &CONTACT, &map).await?;
            let company: Option<Uuid> =
                sqlx::query_scalar("select company_id from contacts where id = $1")
                    .bind(id)
                    .fetch_one(&mut *conn)
                    .await?;
            let mut links = vec![("contact", id)];
            if let Some(c) = company {
                links.push(("company", c));
            }
            link_source(conn, row, &links).await?;
            Ok(Some(id))
        }
        "update_contact" => {
            let id = require_target(row, "contact")?;
            resolve_company(conn, &mut map).await?;
            update_object(conn, &CONTACT, id, &map).await?;
            link_source(conn, row, &[("contact", id)]).await?;
            Ok(Some(id))
        }
        "create_company" => {
            let contact_ids: Vec<Uuid> = map
                .remove("contact_ids")
                .and_then(|v| serde_json::from_value(v).ok())
                .unwrap_or_default();
            default_source(&mut map, row);
            let id = insert_object(conn, &COMPANY, &map).await?;
            for cid in &contact_ids {
                sqlx::query(
                    "update contacts set company_id = $1, updated_at = now() where id = $2",
                )
                .bind(id)
                .bind(cid)
                .execute(&mut *conn)
                .await?;
            }
            link_source(conn, row, &[("company", id)]).await?;
            Ok(Some(id))
        }
        "update_company" => {
            let id = require_target(row, "company")?;
            update_object(conn, &COMPANY, id, &map).await?;
            link_source(conn, row, &[("company", id)]).await?;
            Ok(Some(id))
        }
        "create_deal" => {
            resolve_company(conn, &mut map).await?;
            resolve_stage(conn, &mut map, None).await?;
            default_source(&mut map, row);
            let id = deals::create_deal(conn, map).await?;
            link_source(conn, row, &[("deal", id)]).await?;
            Ok(Some(id))
        }
        "update_deal" => {
            let id = require_target(row, "deal")?;
            resolve_stage(conn, &mut map, Some(id)).await?;
            deals::update_deal(conn, id, map, "ai").await?;
            link_source(conn, row, &[("deal", id)]).await?;
            Ok(Some(id))
        }
        "add_note" => {
            let body =
                take_str(&mut map, "body").ok_or_else(|| AppError::bad("The note is empty."))?;
            let subject = take_str(&mut map, "subject");
            let mut links: Vec<(String, Uuid)> = map
                .remove("links")
                .and_then(|v| serde_json::from_value::<Vec<Value>>(v).ok())
                .unwrap_or_default()
                .into_iter()
                .filter_map(|l| {
                    let t = l["object_type"].as_str()?.to_string();
                    let id = Uuid::parse_str(l["object_id"].as_str()?).ok()?;
                    matches!(t.as_str(), "contact" | "company" | "deal").then_some((t, id))
                })
                .collect();
            if let (Some(t), Some(id)) = (&row.target_type, row.target_id) {
                if !links.iter().any(|(lt, lid)| lt == t && *lid == id) {
                    links.push((t.clone(), id));
                }
            }
            if links.is_empty() {
                return Err(AppError::bad(
                    "Link the note to a contact, company or deal first.",
                ));
            }
            let link_refs: Vec<(&str, Uuid)> =
                links.iter().map(|(t, id)| (t.as_str(), *id)).collect();
            let id = activities::insert(
                conn,
                activities::NewActivity {
                    kind: "note",
                    subject,
                    body,
                    created_by: "ai",
                    ..Default::default()
                },
                &link_refs,
            )
            .await?;
            link_source(conn, row, &link_refs).await?;
            Ok(Some(id))
        }
        "create_task" => {
            map.insert("created_by".into(), json!("ai"));
            if !map.contains_key("kind") {
                map.insert("kind".into(), json!("follow_up"));
            }
            if let (Some(t), Some(id)) = (row.target_type.as_deref(), row.target_id) {
                let key = format!("{t}_id");
                if TASK.is_field(&key) && !map.contains_key(&key) {
                    map.insert(key, json!(id));
                }
            }
            let id = insert_object(conn, &TASK, &map).await?;
            Ok(Some(id))
        }
        "send_email" => {
            let to: Vec<String> = map
                .get("to")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let to: Vec<String> = to
                .into_iter()
                .map(|s| s.trim().to_string())
                .filter(|s| s.contains('@'))
                .collect();
            if to.is_empty() {
                return Err(AppError::bad("Add at least one recipient email address."));
            }
            let cc: Vec<String> = map
                .get("cc")
                .and_then(|v| serde_json::from_value(v.clone()).ok())
                .unwrap_or_default();
            let body = map
                .get("body")
                .and_then(Value::as_str)
                .unwrap_or("")
                .trim()
                .to_string();
            if body.is_empty() {
                return Err(AppError::bad("The email is empty."));
            }
            let parse_id = |k: &str| {
                map.get(k)
                    .and_then(Value::as_str)
                    .and_then(|s| Uuid::parse_str(s).ok())
            };
            let contact_id = parse_id("contact_id").or_else(|| {
                (row.target_type.as_deref() == Some("contact"))
                    .then_some(row.target_id)
                    .flatten()
            });
            let deal_id = parse_id("deal_id").or_else(|| {
                (row.target_type.as_deref() == Some("deal"))
                    .then_some(row.target_id)
                    .flatten()
            });
            let id: Uuid = sqlx::query_scalar(
                "insert into outbox (proposal_id, channel, recipients, cc, subject, body, in_reply_to, contact_id, deal_id)
                 values ($1, 'email', $2, $3, $4, $5, $6, $7, $8) returning id",
            )
            .bind(row.id)
            .bind(json!(to))
            .bind(json!(cc))
            .bind(map.get("subject").and_then(Value::as_str))
            .bind(&body)
            .bind(map.get("in_reply_to").and_then(Value::as_str))
            .bind(contact_id)
            .bind(deal_id)
            .fetch_one(&mut *conn)
            .await?;
            Ok(Some(id))
        }
        other => Err(AppError::bad(format!("Unknown proposal kind: {other}"))),
    }
}

// Outbox ---------------------------------------------------------------------------------

pub async fn outbox(State(state): State<AppState>) -> AppResult<Json<Vec<Value>>> {
    let rows: Vec<Value> = sqlx::query_scalar(
        "select jsonb_build_object('id', o.id, 'channel', o.channel, 'recipients', o.recipients, 'cc', o.cc,
            'subject', o.subject, 'body', o.body, 'status', o.status, 'error', o.error,
            'contact_id', o.contact_id, 'contact_name', nullif(trim(c.first_name || ' ' || c.last_name), ''),
            'deal_id', o.deal_id, 'deal_name', d.name, 'created_at', o.created_at, 'sent_at', o.sent_at)
         from outbox o left join contacts c on c.id = o.contact_id left join deals d on d.id = o.deal_id
         order by o.created_at desc limit 200",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn cancel_outbox(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
) -> AppResult<Json<Value>> {
    let res = sqlx::query("delete from outbox where id = $1 and status = 'queued'")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::bad("Only queued emails can be cancelled."));
    }
    Ok(Json(json!({ "ok": true })))
}
