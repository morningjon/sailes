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
    fields::{insert_object, update_object, CONTACT},
    AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Contact {
    pub id: Uuid,
    pub first_name: String,
    pub last_name: String,
    pub email: Option<String>,
    pub phone: Option<String>,
    pub job_title: Option<String>,
    pub company_id: Option<Uuid>,
    pub company_name: Option<String>,
    pub lifecycle_stage: String,
    pub lead_status: Option<String>,
    pub owner_id: Option<Uuid>,
    pub owner_name: Option<String>,
    pub source: String,
    pub last_contacted_at: Option<DateTime<Utc>>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub total: i64,
}

const SELECT: &str = "select c.id, c.first_name, c.last_name, c.email, c.phone, c.job_title, c.company_id,
    co.name as company_name, c.lifecycle_stage, c.lead_status, c.owner_id, u.name as owner_name, c.source,
    c.last_contacted_at,
    (select max(a.occurred_at) from activity_links l join activities a on a.id = l.activity_id
       where l.object_type = 'contact' and l.object_id = c.id) as last_activity_at,
    c.created_at, c.updated_at, count(*) over() as total
  from contacts c
  left join companies co on co.id = c.company_id
  left join users u on u.id = c.owner_id";

pub async fn list(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> AppResult<Json<Page<Contact>>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(SELECT);
    qb.push(" where true");
    if let Some(s) = p.search() {
        qb.push(" and (c.first_name || ' ' || c.last_name ilike ")
            .push_bind(s.clone())
            .push(" or c.email ilike ")
            .push_bind(s.clone())
            .push(" or co.name ilike ")
            .push_bind(s)
            .push(")");
    }
    if let Some(stage) = &p.lifecycle_stage {
        qb.push(" and c.lifecycle_stage = ")
            .push_bind(stage.clone());
    }
    if let Some(cid) = p.company_id {
        qb.push(" and c.company_id = ").push_bind(cid);
    }
    if let Some(oid) = p.owner_id {
        qb.push(" and c.owner_id = ").push_bind(oid);
    }
    let sort = match p.sort.as_deref() {
        Some("name") => "lower(c.first_name || ' ' || c.last_name)",
        Some("email") => "lower(c.email)",
        Some("company") => "lower(co.name)",
        Some("lifecycle_stage") => "c.lifecycle_stage",
        Some("last_activity_at") => "last_activity_at",
        _ => "c.created_at",
    };
    qb.push(format!(
        " order by {sort} {} nulls last, c.id",
        if p.desc() { "desc" } else { "asc" }
    ));
    qb.push(" limit ")
        .push_bind(p.limit())
        .push(" offset ")
        .push_bind(p.offset());
    let items: Vec<Contact> = qb.build_query_as().fetch_all(&state.pool).await?;
    let total = items.first().map(|c| c.total).unwrap_or(0);
    Ok(Json(Page { items, total }))
}

pub async fn fetch(pool: &sqlx::PgPool, id: Uuid) -> AppResult<Contact> {
    let c: Contact = sqlx::query_as(&format!("{SELECT} where c.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)?;
    Ok(c)
}

pub async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let contact = fetch(&state.pool, id).await?;
    let deals: Vec<Value> = sqlx::query_scalar(
        "select jsonb_build_object('id', d.id, 'name', d.name, 'amount', d.amount, 'currency', d.currency,
            'stage_name', s.name, 'is_closed', s.is_closed, 'is_won', s.is_won, 'close_date', d.close_date)
         from deal_contacts dc join deals d on d.id = dc.deal_id join pipeline_stages s on s.id = d.stage_id
         where dc.contact_id = $1 order by d.created_at desc",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let company: Option<Value> = match contact.company_id {
        Some(cid) => {
            sqlx::query_scalar(
                "select jsonb_build_object('id', id, 'name', name, 'domain', domain, 'industry', industry, 'city', city)
                 from companies where id = $1",
            )
            .bind(cid)
            .fetch_optional(&state.pool)
            .await?
        }
        None => None,
    };
    let tasks = super::tasks::for_object(&state.pool, "contact_id", id).await?;
    Ok(Json(
        json!({ "record": contact, "company": company, "deals": deals, "tasks": tasks }),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> AppResult<Json<Contact>> {
    let map = as_object(body)?;
    if map
        .get("first_name")
        .and_then(Value::as_str)
        .unwrap_or("")
        .trim()
        .is_empty()
        && map
            .get("email")
            .and_then(Value::as_str)
            .unwrap_or("")
            .trim()
            .is_empty()
    {
        return Err(AppError::bad("A contact needs a first name or an email."));
    }
    let mut conn = state.pool.acquire().await?;
    let id = insert_object(&mut conn, &CONTACT, &map).await?;
    drop(conn);
    Ok(Json(fetch(&state.pool, id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Contact>> {
    let map = as_object(body)?;
    let mut conn = state.pool.acquire().await?;
    update_object(&mut conn, &CONTACT, id, &map).await?;
    drop(conn);
    Ok(Json(fetch(&state.pool, id).await?))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let res = sqlx::query("delete from contacts where id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    sqlx::query("delete from activity_links where object_type = 'contact' and object_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
