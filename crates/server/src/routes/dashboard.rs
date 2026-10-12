use axum::{extract::State, Json};
use serde_json::Value;

use crate::{error::AppResult, AppState};

/// Headline numbers for the top of the Review screen and the nav badges.
pub async fn summary(State(state): State<AppState>) -> AppResult<Json<Value>> {
    let v: Value = sqlx::query_scalar(
        "select jsonb_build_object(
            'pending_proposals', (select count(*) from proposals where status = 'pending'),
            'pending_emails', (select count(*) from proposals where status = 'pending' and kind = 'send_email'),
            'tasks_overdue', (select count(*) from tasks where status = 'open' and due_at < date_trunc('day', now())),
            'tasks_today', (select count(*) from tasks where status = 'open'
                and due_at >= date_trunc('day', now()) and due_at < date_trunc('day', now()) + interval '1 day'),
            'open_deal_count', (select count(*) from deals d join pipeline_stages s on s.id = d.stage_id where not s.is_closed),
            'open_pipeline_value', (select coalesce(sum(d.amount), 0) from deals d join pipeline_stages s on s.id = d.stage_id where not s.is_closed),
            'weighted_pipeline_value', (select coalesce(sum(d.amount * s.probability / 100.0), 0) from deals d join pipeline_stages s on s.id = d.stage_id where not s.is_closed),
            'won_this_month', (select coalesce(sum(d.amount), 0) from deals d join pipeline_stages s on s.id = d.stage_id
                where s.is_won and d.updated_at >= date_trunc('month', now())),
            'outbox_queued', (select count(*) from outbox where status = 'queued'),
            'approved_this_week', (select count(*) from proposals where status = 'approved' and decided_at >= now() - interval '7 days')
        )",
    )
    .fetch_one(&state.pool)
    .await?;
    Ok(Json(v))
}
