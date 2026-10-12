use axum::{
    extract::{Query, State},
    Json,
};
use serde::Deserialize;
use serde_json::Value;

use crate::{error::AppResult, AppState};

#[derive(Deserialize)]
pub struct SearchParams {
    q: String,
}

/// Global search across contacts, companies and deals for the top bar.
pub async fn search(
    State(state): State<AppState>,
    Query(p): Query<SearchParams>,
) -> AppResult<Json<Vec<Value>>> {
    let q = p.q.trim();
    if q.is_empty() {
        return Ok(Json(vec![]));
    }
    let like = format!(
        "%{}%",
        q.replace('\\', "\\\\")
            .replace('%', "\\%")
            .replace('_', "\\_")
    );
    let rows: Vec<Value> = sqlx::query_scalar(
        "(select jsonb_build_object('type', 'contact', 'id', c.id,
            'title', nullif(trim(c.first_name || ' ' || c.last_name), ''), 'subtitle', coalesce(c.email, co.name))
          from contacts c left join companies co on co.id = c.company_id
          where c.first_name || ' ' || c.last_name ilike $1 or c.email ilike $1
          order by c.updated_at desc limit 6)
         union all
         (select jsonb_build_object('type', 'company', 'id', id, 'title', name, 'subtitle', coalesce(domain, city))
          from companies where name ilike $1 or domain ilike $1 order by updated_at desc limit 6)
         union all
         (select jsonb_build_object('type', 'deal', 'id', d.id, 'title', d.name, 'subtitle', s.name)
          from deals d join pipeline_stages s on s.id = d.stage_id
          where d.name ilike $1 order by d.updated_at desc limit 6)",
    )
    .bind(like)
    .fetch_all(&state.pool)
    .await?;
    Ok(Json(rows))
}
