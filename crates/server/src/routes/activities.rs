use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use serde_json::{json, Value};
use sqlx::PgConnection;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    AppState,
};

#[derive(Default)]
pub struct NewActivity {
    pub kind: &'static str,
    pub subject: Option<String>,
    pub body: String,
    pub direction: Option<String>,
    pub participants: Option<Value>,
    pub external_id: Option<String>,
    pub source_url: Option<String>,
    pub occurred_at: Option<DateTime<Utc>>,
    pub created_by: &'static str,
}

pub async fn insert(
    conn: &mut PgConnection,
    a: NewActivity,
    links: &[(&str, Uuid)],
) -> AppResult<Uuid> {
    let id: Uuid = sqlx::query_scalar(
        "insert into activities (kind, subject, body, direction, participants, external_id, source_url, occurred_at, created_by)
         values ($1, $2, $3, $4, $5, $6, $7, coalesce($8, now()), $9) returning id",
    )
    .bind(a.kind)
    .bind(a.subject)
    .bind(a.body)
    .bind(a.direction)
    .bind(a.participants.unwrap_or_else(|| json!([])))
    .bind(a.external_id)
    .bind(a.source_url)
    .bind(a.occurred_at)
    .bind(if a.created_by.is_empty() { "user" } else { a.created_by })
    .fetch_one(&mut *conn)
    .await?;
    link(conn, id, links).await?;
    Ok(id)
}

pub async fn link(
    conn: &mut PgConnection,
    activity_id: Uuid,
    links: &[(&str, Uuid)],
) -> AppResult<()> {
    for (object_type, object_id) in links {
        sqlx::query(
            "insert into activity_links (activity_id, object_type, object_id) values ($1, $2, $3) on conflict do nothing",
        )
        .bind(activity_id)
        .bind(*object_type)
        .bind(*object_id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(())
}

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Activity {
    pub id: Uuid,
    pub kind: String,
    pub subject: Option<String>,
    pub body: String,
    pub direction: Option<String>,
    pub participants: Value,
    pub source_url: Option<String>,
    pub occurred_at: DateTime<Utc>,
    pub created_by: String,
    pub links: Value,
}

#[derive(Deserialize)]
pub struct TimelineParams {
    object_type: String,
    object_id: Uuid,
    kind: Option<String>,
    limit: Option<i64>,
}

const SELECT: &str = "select a.id, a.kind, a.subject, a.body, a.direction, a.participants, a.source_url,
    a.occurred_at, a.created_by,
    coalesce((select jsonb_agg(jsonb_build_object('object_type', l.object_type, 'object_id', l.object_id,
        'name', case l.object_type
            when 'contact' then (select trim(first_name || ' ' || last_name) from contacts where id = l.object_id)
            when 'company' then (select name from companies where id = l.object_id)
            when 'deal' then (select name from deals where id = l.object_id) end))
      from activity_links l where l.activity_id = a.id), '[]'::jsonb) as links
  from activities a";

/// Timeline for a record. Companies roll up activity from their contacts, like most CRMs do.
pub async fn timeline(
    State(state): State<AppState>,
    Query(p): Query<TimelineParams>,
) -> AppResult<Json<Vec<Activity>>> {
    let filter = match p.object_type.as_str() {
        "contact" | "deal" => {
            "a.id in (select activity_id from activity_links where object_type = $1 and object_id = $2)"
        }
        "company" => {
            "a.id in (select activity_id from activity_links where (object_type = $1 and object_id = $2)
               or (object_type = 'contact' and object_id in (select id from contacts where company_id = $2))
               or (object_type = 'deal' and object_id in (select id from deals where company_id = $2)))"
        }
        _ => return Err(AppError::bad("object_type must be contact, company or deal")),
    };
    let rows: Vec<Activity> = sqlx::query_as(&format!(
        "{SELECT} where {filter} and ($3::text is null or a.kind = $3) order by a.occurred_at desc limit $4"
    ))
    .bind(&p.object_type)
    .bind(p.object_id)
    .bind(p.kind.filter(|k| !k.is_empty() && k != "all"))
    .bind(p.limit.unwrap_or(200).clamp(1, 1000))
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

pub async fn fetch(pool: &sqlx::PgPool, id: Uuid) -> AppResult<Activity> {
    sqlx::query_as(&format!("{SELECT} where a.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

#[derive(Deserialize)]
pub struct LinkRef {
    pub object_type: String,
    pub object_id: Uuid,
}

#[derive(Deserialize)]
pub struct NewNote {
    body: String,
    #[serde(default)]
    kind: Option<String>,
    subject: Option<String>,
    occurred_at: Option<DateTime<Utc>>,
    links: Vec<LinkRef>,
}

/// Log a note, call or meeting by hand.
pub async fn create(
    State(state): State<AppState>,
    Json(n): Json<NewNote>,
) -> AppResult<Json<Activity>> {
    if n.body.trim().is_empty() {
        return Err(AppError::bad("Write something before saving."));
    }
    if n.links.is_empty() {
        return Err(AppError::bad("Link the note to at least one record."));
    }
    let kind: &'static str = match n.kind.as_deref().unwrap_or("note") {
        "note" => "note",
        "call" => "call",
        "meeting" => "meeting",
        "email" => "email",
        other => {
            return Err(AppError::bad(format!(
                "Can't log an activity of type {other}"
            )))
        }
    };
    for l in &n.links {
        if !matches!(l.object_type.as_str(), "contact" | "company" | "deal") {
            return Err(AppError::bad(
                "Links must point at a contact, company or deal.",
            ));
        }
    }
    let links: Vec<(&str, Uuid)> = n
        .links
        .iter()
        .map(|l| (l.object_type.as_str(), l.object_id))
        .collect();
    let mut tx = state.pool.begin().await?;
    let id = insert(
        &mut tx,
        NewActivity {
            kind,
            subject: n.subject.filter(|s| !s.trim().is_empty()),
            body: n.body.trim().to_string(),
            occurred_at: n.occurred_at,
            created_by: "user",
            ..Default::default()
        },
        &links,
    )
    .await?;
    tx.commit().await?;
    Ok(Json(fetch(&state.pool, id).await?))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let res = sqlx::query("delete from activities where id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}
