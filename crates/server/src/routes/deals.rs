use axum::{
    extract::{Path, Query, State},
    Json,
};
use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde::{Deserialize, Serialize};
use serde_json::{json, Map, Value};
use sqlx::{PgConnection, Postgres, QueryBuilder};
use uuid::Uuid;

use super::{activities, as_object, ListParams, Page};
use crate::{
    error::{AppError, AppResult},
    fields::{insert_object, update_object, DEAL},
    AppState,
};

#[derive(Debug, Serialize, sqlx::FromRow)]
pub struct Deal {
    pub id: Uuid,
    pub name: String,
    pub amount: Option<Decimal>,
    pub currency: String,
    pub pipeline_id: Uuid,
    pub pipeline_name: String,
    pub stage_id: Uuid,
    pub stage_name: String,
    pub probability: i32,
    pub is_closed: bool,
    pub is_won: bool,
    pub close_date: Option<NaiveDate>,
    pub company_id: Option<Uuid>,
    pub company_name: Option<String>,
    pub owner_id: Option<Uuid>,
    pub owner_name: Option<String>,
    pub priority: Option<String>,
    pub next_step: Option<String>,
    pub source: String,
    pub contact_names: Vec<String>,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub created_at: DateTime<Utc>,
    pub updated_at: DateTime<Utc>,
    #[serde(skip)]
    pub total: i64,
}

const SELECT: &str = "select d.id, d.name, d.amount, d.currency, d.pipeline_id, p.name as pipeline_name,
    d.stage_id, s.name as stage_name, s.probability, s.is_closed, s.is_won, d.close_date, d.company_id,
    co.name as company_name, d.owner_id, u.name as owner_name, d.priority, d.next_step, d.source,
    coalesce((select array_agg(trim(c.first_name || ' ' || c.last_name) order by c.first_name)
       from deal_contacts dc join contacts c on c.id = dc.contact_id where dc.deal_id = d.id), '{}') as contact_names,
    (select max(a.occurred_at) from activity_links l join activities a on a.id = l.activity_id
       where l.object_type = 'deal' and l.object_id = d.id) as last_activity_at,
    d.created_at, d.updated_at, count(*) over() as total
  from deals d
  join pipelines p on p.id = d.pipeline_id
  join pipeline_stages s on s.id = d.stage_id
  left join companies co on co.id = d.company_id
  left join users u on u.id = d.owner_id";

pub async fn list(
    State(state): State<AppState>,
    Query(p): Query<ListParams>,
) -> AppResult<Json<Page<Deal>>> {
    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(SELECT);
    qb.push(" where true");
    if let Some(s) = p.search() {
        qb.push(" and (d.name ilike ")
            .push_bind(s.clone())
            .push(" or co.name ilike ")
            .push_bind(s)
            .push(")");
    }
    if let Some(pid) = p.pipeline_id {
        qb.push(" and d.pipeline_id = ").push_bind(pid);
    }
    if let Some(sid) = p.stage_id {
        qb.push(" and d.stage_id = ").push_bind(sid);
    }
    if let Some(cid) = p.company_id {
        qb.push(" and d.company_id = ").push_bind(cid);
    }
    if let Some(oid) = p.owner_id {
        qb.push(" and d.owner_id = ").push_bind(oid);
    }
    match p.status.as_deref() {
        Some("open") => {
            qb.push(" and not s.is_closed");
        }
        Some("won") => {
            qb.push(" and s.is_won");
        }
        Some("lost") => {
            qb.push(" and s.is_closed and not s.is_won");
        }
        _ => {}
    }
    let sort = match p.sort.as_deref() {
        Some("name") => "lower(d.name)",
        Some("amount") => "d.amount",
        Some("close_date") => "d.close_date",
        Some("stage") => "s.position",
        Some("company") => "lower(co.name)",
        _ => "d.created_at",
    };
    qb.push(format!(
        " order by {sort} {} nulls last, d.id",
        if p.desc() { "desc" } else { "asc" }
    ));
    qb.push(" limit ")
        .push_bind(p.limit())
        .push(" offset ")
        .push_bind(p.offset());
    let items: Vec<Deal> = qb.build_query_as().fetch_all(&state.pool).await?;
    let total = items.first().map(|d| d.total).unwrap_or(0);
    Ok(Json(Page { items, total }))
}

pub async fn fetch(pool: &sqlx::PgPool, id: Uuid) -> AppResult<Deal> {
    sqlx::query_as(&format!("{SELECT} where d.id = $1"))
        .bind(id)
        .fetch_optional(pool)
        .await?
        .ok_or(AppError::NotFound)
}

pub async fn get(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let deal = fetch(&state.pool, id).await?;
    let contacts: Vec<Value> = sqlx::query_scalar(
        "select jsonb_build_object('id', c.id, 'first_name', c.first_name, 'last_name', c.last_name,
            'email', c.email, 'job_title', c.job_title, 'phone', c.phone)
         from deal_contacts dc join contacts c on c.id = dc.contact_id where dc.deal_id = $1 order by c.first_name",
    )
    .bind(id)
    .fetch_all(&state.pool)
    .await?;
    let company: Option<Value> = match deal.company_id {
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
    let tasks = super::tasks::for_object(&state.pool, "deal_id", id).await?;
    Ok(Json(
        json!({ "record": deal, "company": company, "contacts": contacts, "tasks": tasks }),
    ))
}

/// Fill in the default pipeline and first stage when they aren't given, and keep the
/// pipeline consistent with the chosen stage.
pub async fn normalise_pipeline(
    conn: &mut PgConnection,
    map: &mut Map<String, Value>,
) -> AppResult<()> {
    if let Some(stage) = map
        .get("stage_id")
        .and_then(Value::as_str)
        .map(str::to_string)
    {
        let sid = Uuid::parse_str(&stage).map_err(|_| AppError::bad("Invalid stage."))?;
        let pid: Uuid = sqlx::query_scalar("select pipeline_id from pipeline_stages where id = $1")
            .bind(sid)
            .fetch_optional(&mut *conn)
            .await?
            .ok_or_else(|| AppError::bad("That deal stage doesn't exist."))?;
        map.insert("pipeline_id".into(), json!(pid));
        return Ok(());
    }
    let pid = match map.get("pipeline_id").and_then(Value::as_str) {
        Some(p) => Uuid::parse_str(p).map_err(|_| AppError::bad("Invalid pipeline."))?,
        None => {
            sqlx::query_scalar("select id from pipelines order by position, created_at limit 1")
                .fetch_optional(&mut *conn)
                .await?
                .ok_or_else(|| AppError::bad("Create a pipeline first."))?
        }
    };
    let sid: Uuid = sqlx::query_scalar(
        "select id from pipeline_stages where pipeline_id = $1 order by position limit 1",
    )
    .bind(pid)
    .fetch_optional(&mut *conn)
    .await?
    .ok_or_else(|| AppError::bad("That pipeline has no stages."))?;
    map.insert("pipeline_id".into(), json!(pid));
    map.insert("stage_id".into(), json!(sid));
    Ok(())
}

/// Create a deal with optional `contact_ids`. Used by the API and by approved proposals.
pub async fn create_deal(conn: &mut PgConnection, mut map: Map<String, Value>) -> AppResult<Uuid> {
    let contact_ids = take_ids(&mut map, "contact_ids")?;
    normalise_pipeline(conn, &mut map).await?;
    let id = insert_object(conn, &DEAL, &map).await?;
    for cid in contact_ids {
        sqlx::query("insert into deal_contacts (deal_id, contact_id) values ($1, $2) on conflict do nothing")
            .bind(id)
            .bind(cid)
            .execute(&mut *conn)
            .await?;
    }
    Ok(id)
}

/// Update a deal, logging stage changes to its timeline.
pub async fn update_deal(
    conn: &mut PgConnection,
    id: Uuid,
    mut map: Map<String, Value>,
    actor: &str,
) -> AppResult<()> {
    let contact_ids = map
        .contains_key("contact_ids")
        .then(|| take_ids(&mut map, "contact_ids"))
        .transpose()?;
    let old_stage: Option<(Uuid, String)> = sqlx::query_as(
        "select s.id, s.name from deals d join pipeline_stages s on s.id = d.stage_id where d.id = $1",
    )
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    let Some((old_stage_id, old_stage_name)) = old_stage else {
        return Err(AppError::NotFound);
    };
    if map.contains_key("stage_id") {
        normalise_pipeline(conn, &mut map).await?;
    }
    update_object(conn, &DEAL, id, &map).await?;

    if let Some(ids) = contact_ids {
        sqlx::query("delete from deal_contacts where deal_id = $1")
            .bind(id)
            .execute(&mut *conn)
            .await?;
        for cid in ids {
            sqlx::query("insert into deal_contacts (deal_id, contact_id) values ($1, $2) on conflict do nothing")
                .bind(id)
                .bind(cid)
                .execute(&mut *conn)
                .await?;
        }
    }

    let new_stage: (Uuid, String) = sqlx::query_as(
        "select s.id, s.name from deals d join pipeline_stages s on s.id = d.stage_id where d.id = $1",
    )
    .bind(id)
    .fetch_one(&mut *conn)
    .await?;
    if new_stage.0 != old_stage_id {
        let who = if actor == "ai" {
            "sAIles (approved)"
        } else {
            "You"
        };
        activities::insert(
            conn,
            activities::NewActivity {
                kind: "system",
                subject: Some(format!("Deal stage changed to {}", new_stage.1)),
                body: format!(
                    "{who} moved this deal from {old_stage_name} to {}.",
                    new_stage.1
                ),
                created_by: if actor == "ai" { "ai" } else { "user" },
                ..Default::default()
            },
            &[("deal", id)],
        )
        .await?;
    }
    Ok(())
}

fn take_ids(map: &mut Map<String, Value>, key: &str) -> AppResult<Vec<Uuid>> {
    match map.remove(key) {
        None | Some(Value::Null) => Ok(vec![]),
        Some(Value::Array(items)) => items
            .iter()
            .map(|v| {
                v.as_str()
                    .and_then(|s| Uuid::parse_str(s).ok())
                    .ok_or_else(|| AppError::bad(format!("Invalid id in {key}")))
            })
            .collect(),
        Some(_) => Err(AppError::bad(format!("{key} must be a list of ids"))),
    }
}

pub async fn create(
    State(state): State<AppState>,
    Json(body): Json<Value>,
) -> AppResult<Json<Deal>> {
    let map = as_object(body)?;
    let mut tx = state.pool.begin().await?;
    let id = create_deal(&mut tx, map).await?;
    tx.commit().await?;
    Ok(Json(fetch(&state.pool, id).await?))
}

pub async fn update(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<Value>,
) -> AppResult<Json<Deal>> {
    let map = as_object(body)?;
    let mut tx = state.pool.begin().await?;
    update_deal(&mut tx, id, map, "user").await?;
    tx.commit().await?;
    Ok(Json(fetch(&state.pool, id).await?))
}

pub async fn delete(State(state): State<AppState>, Path(id): Path<Uuid>) -> AppResult<Json<Value>> {
    let res = sqlx::query("delete from deals where id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    sqlx::query("delete from activity_links where object_type = 'deal' and object_id = $1")
        .bind(id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

#[derive(Deserialize)]
pub struct ContactLink {
    contact_id: Uuid,
}

pub async fn add_contact(
    State(state): State<AppState>,
    Path(id): Path<Uuid>,
    Json(body): Json<ContactLink>,
) -> AppResult<Json<Value>> {
    sqlx::query(
        "insert into deal_contacts (deal_id, contact_id) values ($1, $2) on conflict do nothing",
    )
    .bind(id)
    .bind(body.contact_id)
    .execute(&state.pool)
    .await?;
    Ok(Json(json!({ "ok": true })))
}

pub async fn remove_contact(
    State(state): State<AppState>,
    Path((id, contact_id)): Path<(Uuid, Uuid)>,
) -> AppResult<Json<Value>> {
    sqlx::query("delete from deal_contacts where deal_id = $1 and contact_id = $2")
        .bind(id)
        .bind(contact_id)
        .execute(&state.pool)
        .await?;
    Ok(Json(json!({ "ok": true })))
}

// Pipelines --------------------------------------------------------------------------

pub async fn pipelines(State(state): State<AppState>) -> AppResult<Json<Vec<Value>>> {
    let rows: Vec<Value> = sqlx::query_scalar(
        "select jsonb_build_object('id', p.id, 'name', p.name, 'stages',
            coalesce((select jsonb_agg(jsonb_build_object('id', s.id, 'name', s.name, 'position', s.position,
                'probability', s.probability, 'is_closed', s.is_closed, 'is_won', s.is_won) order by s.position)
              from pipeline_stages s where s.pipeline_id = p.id), '[]'::jsonb))
         from pipelines p order by p.position, p.created_at",
    )
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}

/// Create the default sales pipeline if there isn't one yet.
pub async fn ensure_default_pipeline(pool: &sqlx::PgPool) -> anyhow::Result<()> {
    let count: i64 = sqlx::query_scalar("select count(*) from pipelines")
        .fetch_one(pool)
        .await?;
    if count > 0 {
        return Ok(());
    }
    let mut tx = pool.begin().await?;
    let pid: Uuid = sqlx::query_scalar(
        "insert into pipelines (name, position) values ('Sales pipeline', 0) returning id",
    )
    .fetch_one(&mut *tx)
    .await?;
    let stages = [
        ("Discovery", 10, false, false),
        ("Qualified", 25, false, false),
        ("Proposal sent", 50, false, false),
        ("Negotiation", 75, false, false),
        ("Closed won", 100, true, true),
        ("Closed lost", 0, true, false),
    ];
    for (i, (name, prob, closed, won)) in stages.iter().enumerate() {
        sqlx::query(
            "insert into pipeline_stages (pipeline_id, name, position, probability, is_closed, is_won)
             values ($1, $2, $3, $4, $5, $6)",
        )
        .bind(pid)
        .bind(name)
        .bind(i as i32)
        .bind(prob)
        .bind(closed)
        .bind(won)
        .execute(&mut *tx)
        .await?;
    }
    tx.commit().await?;
    Ok(())
}
