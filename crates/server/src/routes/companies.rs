use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, Utc};
use rust_decimal::Decimal;
use serde::Serialize;
use serde_json::{json, Value};
use sqlx::{Postgres, QueryBuilder};
use uuid::Uuid;

use super::{as_object, ListParams, Page};
use crate::{
    error::{AppError, AppResult},
    fields::{insert_object, update_object, COMPANY},
    AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Company {
    pub id: Uuid,
    pub name: String,
    pub domain: Option<String>,
    pub industry: Option<String>,
    pub employees: Option<i32>,
    pub city: Option<String>,
    pub country: Option<String>,
    pub phone: Option<String>,
    pub description: Option<String>,
    pub lifecycle_stage: String,
    pub owner_id: Option<Uuid>,
    pub owner_name: Option<String>,
    pub source: String,
    pub contact_count: i64,
    pub open_deal_count: i64,
    pub open_deal_value: Option<Decimal>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub total: i64,
}

const SELECT: &str = "select co.id, co.name, co.domain, co.industry, co.employees, co.city, co.country, co.phone,
    co.description, co.lifecycle_stage, co.owner_id, u.name as owner_name, co.source,
    (select count(*) from contacts c where c.company_id = co.id) as contact_count,
    (select count(*) from deals d join pipeline_stages s on s.id = d.stage_id
       where d.company_id = co.id and not s.is_closed) as open_deal_count,
    (select sum(d.amount) from deals d join pipeline_stages s on s.id = d.stage_id
       where d.company_id = co.id and not s.is_closed) as open_deal_value,
    (select max(a.occurred_at) from activity_links l join activities a on a.id = l.activity_id
       where (l.object_type = 'company' and l.object_id = co.id)
          or (l.object_type = 'contact' and l.object_id in (select id from contacts where company_id = co.id))
    ) as last_activity_at,
    co.created_at, co.updated_at, count(*) over() as total
  from companies co
  left join users u on u.id = co.owner_id";

pub async fn list(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> AppResult<Json<Page<Company>>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(SELECT);
    qb.push(" where true");
    if let Some(s) = p.search() {
        qb.push(" and (co.name ilike ")
            .push_bind(s.clone())
            .push(" or co.domain ilike ")
            .push_bind(s)
            .push(")");
    }
    if let Some(stage) = &p.lifecycle_stage {
        qb.push(" and co.lifecycle_stage = ")
            .push_bind(stage.clone());
    }
    if let Some(oid) = p.owner_id {
        qb.push(" and co.owner_id = ").push_bind(oid);
    }
    let sort = match p.sort.as_deref() {
        Some("name") => "lower(co.name)",
        Some("industry") => "lower(co.industry)",
        Some("city") => "lower(co.city)",
        Some("open_deal_value") => "open_deal_value",
        Some("last_activity_at") => "last_activity_at",
        _ => "co.created_at",
    };
    qb.push(format!(
        " order by {sort} {} nulls last, co.id",
        if p.desc() { "desc" } else { "asc" }
    ));
    qb.push(" limit ")
        .push_bind(p.limit())
        .push(" offset ")
        .push_bind(p.offset());
    let items: Vec<Company> = qb.build_query_as().fetch_all(&state.pool).await?;
    let total = items.first().map(|c| c.total).unwrap_or(0);
    Ok(Json(Page { items, total }))
}

pub async fn fetch(pool: &sqlx::PgPool, id: Uuid) -> AppResult<Company> {
    sqlx::query_as(&format!("{SELECT} where co.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let company = fetch(&state.pool, id).await?;
    let contacts: Vec<Value> = sqlx::query_scalar(
        "select jsonb_build_object('id', id, 'first_name', first_name, 'last_name', last_name, 'email', email,
            'job_title', job_title, 'phone', phone)
         from contacts where company_id = $1 order by created_at",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let deals: Vec<Value> = sqlx::query_scalar(
        "select jsonb_build_object('id', d.id, 'name', d.name, 'amount', d.amount, 'currency', d.currency,
            'stage_name', s.name, 'is_closed', s.is_closed, 'is_won', s.is_won, 'close_date', d.close_date)
         from deals d join pipeline_stages s on s.id = d.stage_id
         where d.company_id = $1 order by d.created_at desc",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let tasks = super::tasks::for_object(&state.pool, "company_id", id).await?;
    Ok(Json(
        json!({ "record": company, "contacts": contacts, "deals": deals, "tasks": tasks }),
    ))
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> AppResult<Json<Company>> {
    let map = as_object(body)?;
    let mut conn = state.pool.acquire().await?;
    let id = insert_object(&mut conn, &COMPANY, &map).await?;
    drop(conn);
    Ok(Json(fetch(&state.pool, id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Company>> {
    let map = as_object(body)?;
    let mut conn = state.pool.acquire().await?;
    update_object(&mut conn, &COMPANY, id, &map).await?;
    drop(conn);
    Ok(Json(fetch(&state.pool, id).await?))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let res = sqlx::query("delete from companies where id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    sqlx::query("delete from activity_links where object_type = 'company' and object_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}
