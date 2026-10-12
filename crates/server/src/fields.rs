//! Generic, whitelisted field handling for CRM objects.
//!
//! Contacts, companies, deals and tasks are created and updated both by people (through the
//! dashboard) and by approved AI proposals. Both paths send a JSON object of field values;
//! this module validates those against a per-object spec and builds the SQL.

use std::str::FromStr;

use chrono::{DateTime, NaiveDate, Utc};
use rust_decimal::Decimal;
use serde_json::{Map, Value};
use sqlx::{PgConnection, Postgres, QueryBuilder};
use uuid::Uuid;

use crate::error::{AppError, AppResult};

#[derive(Clone, Copy, Debug)]
pub enum FieldType {
    Text,
    Int,
    Uuid,
    Decimal,
    Date,
    Timestamp,
}

pub struct ObjectSpec {
    pub table: &'static str,
    pub label: &'static str,
    pub fields: &'static [(&'static str, FieldType)],
    /// Fields that must be present and non-empty on create.
    pub required: &'static [&'static str],
    /// Whether the table has an `updated_at` column to bump.
    pub has_updated_at: bool,
}

impl ObjectSpec {
    fn field_type(&self, name: &str) -> Option<FieldType> {
        self.fields
            .iter()
            .find(|(n, _)| *n == name)
            .map(|(_, t)| *t)
    }

    /// Keep only known fields; reject unknown ones so typos don't silently vanish.
    pub fn clean(
        &self,
        input: &Map<String, Value>,
    ) -> AppResult<Vec<(&'static str, FieldType, Value)>> {
        let mut out = Vec::new();
        for (key, value) in input {
            match self.fields.iter().find(|(n, _)| n == key) {
                Some((name, ft)) => out.push((*name, *ft, value.clone())),
                None => {
                    return Err(AppError::bad(format!(
                        "Unknown {} field: {key}",
                        self.label
                    )))
                }
            }
        }
        Ok(out)
    }

    pub fn is_field(&self, name: &str) -> bool {
        self.field_type(name).is_some()
    }
}

pub const CONTACT: ObjectSpec = ObjectSpec {
    table: "contacts",
    label: "contact",
    fields: &[
        ("first_name", FieldType::Text),
        ("last_name", FieldType::Text),
        ("email", FieldType::Text),
        ("phone", FieldType::Text),
        ("job_title", FieldType::Text),
        ("company_id", FieldType::Uuid),
        ("lifecycle_stage", FieldType::Text),
        ("lead_status", FieldType::Text),
        ("owner_id", FieldType::Uuid),
        ("source", FieldType::Text),
        ("last_contacted_at", FieldType::Timestamp),
    ],
    required: &[],
    has_updated_at: true,
};

pub const COMPANY: ObjectSpec = ObjectSpec {
    table: "companies",
    label: "company",
    fields: &[
        ("name", FieldType::Text),
        ("domain", FieldType::Text),
        ("industry", FieldType::Text),
        ("employees", FieldType::Int),
        ("city", FieldType::Text),
        ("country", FieldType::Text),
        ("phone", FieldType::Text),
        ("description", FieldType::Text),
        ("lifecycle_stage", FieldType::Text),
        ("owner_id", FieldType::Uuid),
        ("source", FieldType::Text),
    ],
    required: &["name"],
    has_updated_at: true,
};

pub const DEAL: ObjectSpec = ObjectSpec {
    table: "deals",
    label: "deal",
    fields: &[
        ("name", FieldType::Text),
        ("amount", FieldType::Decimal),
        ("currency", FieldType::Text),
        ("pipeline_id", FieldType::Uuid),
        ("stage_id", FieldType::Uuid),
        ("close_date", FieldType::Date),
        ("company_id", FieldType::Uuid),
        ("owner_id", FieldType::Uuid),
        ("priority", FieldType::Text),
        ("next_step", FieldType::Text),
        ("source", FieldType::Text),
    ],
    required: &["name", "pipeline_id", "stage_id"],
    has_updated_at: true,
};

pub const TASK: ObjectSpec = ObjectSpec {
    table: "tasks",
    label: "task",
    fields: &[
        ("title", FieldType::Text),
        ("notes", FieldType::Text),
        ("kind", FieldType::Text),
        ("priority", FieldType::Text),
        ("status", FieldType::Text),
        ("due_at", FieldType::Timestamp),
        ("contact_id", FieldType::Uuid),
        ("company_id", FieldType::Uuid),
        ("deal_id", FieldType::Uuid),
        ("owner_id", FieldType::Uuid),
        ("created_by", FieldType::Text),
        ("completed_at", FieldType::Timestamp),
    ],
    required: &["title"],
    has_updated_at: false,
};

pub fn spec_for(object_type: &str) -> AppResult<&'static ObjectSpec> {
    match object_type {
        "contact" => Ok(&CONTACT),
        "company" => Ok(&COMPANY),
        "deal" => Ok(&DEAL),
        "task" => Ok(&TASK),
        other => Err(AppError::bad(format!("Unknown object type: {other}"))),
    }
}

fn bind_value(
    qb: &mut QueryBuilder<'_, Postgres>,
    name: &str,
    ft: FieldType,
    v: &Value,
) -> AppResult<()> {
    let invalid = || AppError::bad(format!("Invalid value for {name}"));
    match ft {
        FieldType::Text => {
            let s = match v {
                Value::Null => None,
                Value::String(s) => {
                    let t = s.trim();
                    (!t.is_empty()).then(|| t.to_string())
                }
                Value::Number(n) => Some(n.to_string()),
                _ => return Err(invalid()),
            };
            qb.push_bind(s);
        }
        FieldType::Int => {
            let n = match v {
                Value::Null => None,
                Value::Number(n) => Some(n.as_i64().ok_or_else(invalid)? as i32),
                Value::String(s) if s.trim().is_empty() => None,
                Value::String(s) => Some(s.trim().parse::<i32>().map_err(|_| invalid())?),
                _ => return Err(invalid()),
            };
            qb.push_bind(n);
        }
        FieldType::Uuid => {
            let u = match v {
                Value::Null => None,
                Value::String(s) if s.trim().is_empty() => None,
                Value::String(s) => Some(Uuid::parse_str(s.trim()).map_err(|_| invalid())?),
                _ => return Err(invalid()),
            };
            qb.push_bind(u);
        }
        FieldType::Decimal => {
            let d = match v {
                Value::Null => None,
                Value::Number(n) => Some(Decimal::from_str(&n.to_string()).map_err(|_| invalid())?),
                Value::String(s) if s.trim().is_empty() => None,
                Value::String(s) => {
                    let cleaned: String = s
                        .chars()
                        .filter(|c| c.is_ascii_digit() || *c == '.' || *c == '-')
                        .collect();
                    Some(Decimal::from_str(&cleaned).map_err(|_| invalid())?)
                }
                _ => return Err(invalid()),
            };
            qb.push_bind(d);
        }
        FieldType::Date => {
            let d = match v {
                Value::Null => None,
                Value::String(s) if s.trim().is_empty() => None,
                Value::String(s) => {
                    let s = s.trim();
                    let date = NaiveDate::parse_from_str(&s[..s.len().min(10)], "%Y-%m-%d")
                        .map_err(|_| invalid())?;
                    Some(date)
                }
                _ => return Err(invalid()),
            };
            qb.push_bind(d);
        }
        FieldType::Timestamp => {
            let t = match v {
                Value::Null => None,
                Value::String(s) if s.trim().is_empty() => None,
                Value::String(s) => {
                    let s = s.trim();
                    let parsed = DateTime::parse_from_rfc3339(s)
                        .map(|d| d.with_timezone(&Utc))
                        .or_else(|_| {
                            NaiveDate::parse_from_str(s, "%Y-%m-%d")
                                .map(|d| d.and_hms_opt(9, 0, 0).unwrap().and_utc())
                        })
                        .map_err(|_| invalid())?;
                    Some(parsed)
                }
                _ => return Err(invalid()),
            };
            qb.push_bind(t);
        }
    }
    Ok(())
}

/// Insert a row from a JSON object of fields. Returns the new id.
pub async fn insert_object(
    conn: &mut PgConnection,
    spec: &ObjectSpec,
    input: &Map<String, Value>,
) -> AppResult<Uuid> {
    let fields = spec.clean(input)?;
    for req in spec.required {
        let present = fields.iter().any(|(n, _, v)| {
            n == req && !v.is_null() && !(v.as_str().map(|s| s.trim().is_empty()).unwrap_or(false))
        });
        if !present {
            return Err(AppError::bad(format!(
                "A {} needs a {}.",
                spec.label,
                req.replace('_', " ")
            )));
        }
    }

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(format!("insert into {} (", spec.table));
    if fields.is_empty() {
        qb = QueryBuilder::new(format!(
            "insert into {} default values returning id",
            spec.table
        ));
    } else {
        for (i, (name, _, _)) in fields.iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            qb.push(*name);
        }
        qb.push(") values (");
        for (i, (name, ft, v)) in fields.iter().enumerate() {
            if i > 0 {
                qb.push(", ");
            }
            bind_value(&mut qb, name, *ft, v)?;
        }
        qb.push(") returning id");
    }
    let id: Uuid = qb.build_query_scalar().fetch_one(&mut *conn).await?;
    Ok(id)
}

/// Update a row from a JSON object of fields. Errors with NotFound if the row is missing.
pub async fn update_object(
    conn: &mut PgConnection,
    spec: &ObjectSpec,
    id: Uuid,
    input: &Map<String, Value>,
) -> AppResult<()> {
    let fields = spec.clean(input)?;
    for req in spec.required {
        if let Some((_, _, v)) = fields.iter().find(|(n, _, _)| n == req) {
            if v.is_null() || v.as_str().map(|s| s.trim().is_empty()).unwrap_or(false) {
                return Err(AppError::bad(format!(
                    "A {} needs a {}.",
                    spec.label,
                    req.replace('_', " ")
                )));
            }
        }
    }
    if fields.is_empty() {
        // Nothing to change, but still confirm the record exists.
        let exists: bool = sqlx::query_scalar(&format!(
            "select exists(select 1 from {} where id = $1)",
            spec.table
        ))
        .bind(id)
        .fetch_one(&mut *conn)
        .await?;
        return if exists {
            Ok(())
        } else {
            Err(AppError::NotFound)
        };
    }

    let mut qb: QueryBuilder<Postgres> = QueryBuilder::new(format!("update {} set ", spec.table));
    for (i, (name, ft, v)) in fields.iter().enumerate() {
        if i > 0 {
            qb.push(", ");
        }
        qb.push(*name).push(" = ");
        bind_value(&mut qb, name, *ft, v)?;
    }
    if spec.has_updated_at {
        qb.push(", updated_at = now()");
    }
    qb.push(" where id = ").push_bind(id);
    let res = qb.build().execute(&mut *conn).await?;
    if res.rows_affected() == 0 {
        return Err(AppError::NotFound);
    }
    Ok(())
}

/// Snapshot a row as JSON, used to show "current vs proposed" in the review queue.
pub async fn snapshot(
    conn: &mut PgConnection,
    spec: &ObjectSpec,
    id: Uuid,
) -> AppResult<Option<Value>> {
    let v: Option<Value> = sqlx::query_scalar(&format!(
        "select to_jsonb(t) from {} t where id = $1",
        spec.table
    ))
    .bind(id)
    .fetch_optional(&mut *conn)
    .await?;
    Ok(v)
}
