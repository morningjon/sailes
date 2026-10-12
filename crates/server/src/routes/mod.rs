pub mod activities;
pub mod companies;
pub mod contacts;
pub mod dashboard;
pub mod deals;
pub mod proposals;
pub mod search;
pub mod tasks;

use serde::{Deserialize, Serialize};
use serde_json::{Map, Value};

use crate::error::{AppError, AppResult};

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub q: Option<String>,
    pub sort: Option<String>,
    pub dir: Option<String>,
    pub limit: Option<i64>,
    pub offset: Option<i64>,
    pub lifecycle_stage: Option<String>,
    pub company_id: Option<uuid::Uuid>,
    pub owner_id: Option<uuid::Uuid>,
    pub pipeline_id: Option<uuid::Uuid>,
    pub stage_id: Option<uuid::Uuid>,
    pub status: Option<String>,
    pub due: Option<String>,
}

impl ListParams {
    pub fn limit(&self) -> i64 {
        self.limit.unwrap_or(50).clamp(1, 1000)
    }
    pub fn offset(&self) -> i64 {
        self.offset.unwrap_or(0).max(0)
    }
    pub fn desc(&self) -> bool {
        !matches!(self.dir.as_deref(), Some("asc"))
    }
    pub fn search(&self) -> Option<String> {
        self.q
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| {
                format!(
                    "%{}%",
                    s.replace('\\', "\\\\")
                        .replace('%', "\\%")
                        .replace('_', "\\_")
                )
            })
    }
}

#[derive(Serialize)]
pub struct Page<T> {
    pub items: Vec<T>,
    pub total: i64,
}

pub fn as_object(v: Value) -> AppResult<Map<String, Value>> {
    match v {
        Value::Object(m) => Ok(m),
        _ => Err(AppError::bad("Expected a JSON object.")),
    }
}
