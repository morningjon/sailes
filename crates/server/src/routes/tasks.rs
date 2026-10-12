use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use super::{as_object, ListParams, Page};
use crate::{
    error::{AppError, AppResult},
    fields::{insert_object, update_object, TASK},
    AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Task {
    pub id: Uuid,
    pub title: String,
    pub notes: Option<String>,
    pub kind: String,
    pub priority: String,
    pub status: String,
    pub due_at: Option<DateTime<Utc>>,
    pub contact_id: Option<Uuid>,
    pub contact_name: Option<String>,
    pub company_id: Option<Uuid>,
    pub company_name: Option<String>,
    pub deal_id: Option<Uuid>,
    pub deal_name: Option<String>,
    pub owner_id: Option<Uuid>,
    pub owner_name: Option<String>,
    pub created_by: String,
    pub completed_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    #[serde(skip)]
    pub total: i64,
}

const SELECT: &str = "select t.id, t.title, t.notes, t.kind, t.priority, t.status, t.due_at,
    t.contact_id, nullif(trim(c.first_name || ' ' || c.last_name), '') as contact_name,
    t.company_id, co.name as company_name, t.deal_id, d.name as deal_name,
    t.owner_id, u.name as owner_name, t.created_by, t.completed_at, t.created_at, count(*) over() as total
  from tasks t
  left join contacts c on c.id = t.contact_id
  left join companies co on co.id = t.company_id
  left join deals d on d.id = t.deal_id
  left join users u on u.id = t.owner_id";

pub async fn list(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> AppResult<Json<Page<Task>>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(SELECT);
    qb.push(" where true");
    if let Some(s) = p.search() {
        qb.push(" and t.title ilike ").push_bind(s);
    }
    match p.status.as_deref() {
        Some("open") | None => {
            qb.push(" and t.status = 'open'");
        }
        Some("done") => {
            qb.push(" and t.status = 'done'");
        }
        _ => {}
    }
    match p.due.as_deref() {
        Some("overdue") => {
            qb.push(" and t.due_at < date_trunc('day', now())");
        }
        Some("today") => {
            qb.push(" and t.due_at >= date_trunc('day', now()) and t.due_at < date_trunc('day', now()) + interval '1 day'");
        }
        Some("upcoming") => {
            qb.push(" and t.due_at >= date_trunc('day', now()) + interval '1 day'");
        }
        _ => {}
    }
    if let Some(oid) = p.owner_id {
        qb.push(" and t.owner_id = ").push_bind(oid);
    }
    let order = if p.status.as_deref() == Some("done") {
        "t.completed_at desc"
    } else {
        "t.due_at asc nulls last"
    };
    qb.push(format!(" order by {order}, t.created_at desc"));
    qb.push(" limit ")
        .push_bind(p.limit())
        .push(" offset ")
        .push_bind(p.offset());
    let items: Vec<Task> = qb.build_query_as().fetch_all(&state.pool).await?;
    let total = items.first().map(|t| t.total).unwrap_or(0);
    Ok(Json(Page { items, total }))
}

pub async fn for_object(pool: &sqlx::PgPool, column: &str, id: Uuid) -> AppResult<Vec<Task>> {
    let col = match column {
        "contact_id" | "company_id" | "deal_id" => column,
        _ => return Err(AppError::bad("bad column")),
    };
    Ok(sqlx::query_as(&format!(
        "{SELECT} where t.{col} = $1 order by (t.status = 'done'), t.due_at asc nulls last limit 50"
    ))
    .bind(id)
    .fetch_all(pool)
    .await?)
}

pub async fn fetch(pool: &sqlx::PgPool, id: Uuid) -> AppResult<Task> {
    sqlx::query_as(&format!("{SELECT} where t.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> AppResult<Json<Task>> {
    let map = as_object(body)?;
    let mut conn = state.pool.acquire().await?;
    let id = insert_object(&mut conn, &TASK, &map).await?;
    drop(conn);
    Ok(Json(fetch(&state.pool, id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Task>> {
    let mut map = as_object(body)?;
    // Completing a task stamps completed_at; reopening clears it.
    match map.get("status").and_then(Value::as_str) {
        Some("done") if !map.contains_key("completed_at") => {
            map.insert("completed_at".into(), json!(Utc::now().to_rfc3339()));
        }
        Some("open") => {
            map.insert("completed_at".into(), Value::Null);
        }
        _ => {}
    }
    let mut conn = state.pool.acquire().await?;
    update_object(&mut conn, &TASK, id, &map).await?;
    drop(conn);
    Ok(Json(fetch(&state.pool, id).await?))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let res = sqlx::query("delete from tasks where id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(Json(json!({ "ok": true })))
}
