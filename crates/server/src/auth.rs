//! Email + password sign-in with server-side sessions.
//!
//! The first account is created either from SAILES_ADMIN_* environment variables at boot,
//! or through the one-time setup screen in the dashboard (only allowed while no users exist).

use argon2::{
    password_hash::{rand_core::OsRng, PasswordHash, PasswordHasher, PasswordVerifier, SaltString},
    Argon2,
};
use axum::{
    extract::{Request, State},
    http::header,
    middleware::Next,
    response::{IntoResponse, Response},
    Extension, Json,
};
use axum_extra::extract::cookie::{Cookie, CookieJar, SameSite};
use chrono::{Duration, Utc};
use rand::RngCore;
use serde::{Deserialize, Serialize};
use serde_json::json;
use sha2::{Digest, Sha256};
use sqlx::PgPool;
use uuid::Uuid;

use crate::{
    error::{AppError, AppResult},
    AppState,
};

pub const COOKIE: &str = "sailes_session";
const SESSION_DAYS: i64 = 30;

#[derive(Clone, Debug, Serialize, sqlx::FromRow)]
pub struct User {
    pub id: Uuid,
    pub email: String,
    pub name: String,
}

pub fn hash_password(password: &str) -> anyhow::Result<String> {
    let salt = SaltString::generate(&mut OsRng);
    Argon2::default()
        .hash_password(password.as_bytes(), &salt)
        .map(|h| h.to_string())
        .map_err(|e| anyhow::anyhow!("hashing failed: {e}"))
}

fn verify_password(password: &str, hash: &str) -> bool {
    PasswordHash::new(hash)
        .map(|parsed| {
            Argon2::default()
                .verify_password(password.as_bytes(), &parsed)
                .is_ok()
        })
        .unwrap_or(false)
}

fn hash_token(token: &str) -> String {
    hex::encode(Sha256::digest(token.as_bytes()))
}

pub async fn user_count(pool: &PgPool) -> sqlx::Result<i64> {
    sqlx::query_scalar("select count(*) from users")
        .fetch_one(pool)
        .await
}

pub async fn create_user(
    conn: &mut sqlx::PgConnection,
    email: &str,
    name: &str,
    password: &str,
) -> AppResult<User> {
    let email = email.trim().to_lowercase();
    if !email.contains('@') {
        return Err(AppError::bad("Enter a valid email address."));
    }
    if password.len() < 10 {
        return Err(AppError::bad("Use a password of at least 10 characters."));
    }
    let hash = hash_password(password)?;
    let user = sqlx::query_as::<_, User>(
        "insert into users (email, name, password_hash) values ($1, $2, $3) returning id, email, name",
    )
    .bind(&email)
    .bind(name.trim())
    .bind(hash)
    .fetch_one(&mut *conn)
    .await?;
    // Records created before any user existed (e.g. demo data) get the first user as owner.
    for table in ["companies", "contacts", "deals", "tasks"] {
        sqlx::query(&format!(
            "update {table} set owner_id = $1 where owner_id is null"
        ))
        .bind(user.id)
        .execute(&mut *conn)
        .await?;
    }
    Ok(user)
}

async fn start_session(state: &AppState, jar: CookieJar, user_id: Uuid) -> AppResult<CookieJar> {
    let mut bytes = [0u8; 32];
    rand::thread_rng().fill_bytes(&mut bytes);
    let token = hex::encode(bytes);
    sqlx::query("insert into sessions (token_hash, user_id, expires_at) values ($1, $2, $3)")
        .bind(hash_token(&token))
        .bind(user_id)
        .bind(Utc::now() + Duration::days(SESSION_DAYS))
        .execute(&state.pool)
        .await?;
    let cookie = Cookie::build((COOKIE, token))
        .path("/")
        .http_only(true)
        .same_site(SameSite::Lax)
        .secure(state.config.secure_cookies)
        .max_age(time_days(SESSION_DAYS))
        .build();
    Ok(jar.add(cookie))
}

fn time_days(days: i64) -> time::Duration {
    time::Duration::days(days)
}

// Handlers ----------------------------------------------------------------------

#[derive(Serialize)]
pub struct AuthStatus {
    needs_setup: bool,
    user: Option<User>,
}

pub async fn status(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: axum::http::HeaderMap,
) -> AppResult<Json<AuthStatus>> {
    let needs_setup = user_count(&state.pool).await? == 0;
    let user = match token_from(&jar, &headers) {
        Some(t) => lookup_session(&state.pool, &t).await?,
        None => None,
    };
    Ok(Json(AuthStatus { needs_setup, user }))
}

#[derive(Deserialize)]
pub struct SetupRequest {
    email: String,
    name: String,
    password: String,
}

pub async fn setup(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<SetupRequest>,
) -> AppResult<Response> {
    // Serialise concurrent setup attempts so only one first account can ever be created.
    let mut tx = state.pool.begin().await?;
    sqlx::query("lock table users in exclusive mode")
        .execute(&mut *tx)
        .await?;
    let count: i64 = sqlx::query_scalar("select count(*) from users")
        .fetch_one(&mut *tx)
        .await?;
    if count > 0 {
        return Err(AppError::bad("Setup is already complete. Sign in instead."));
    }
    if req.name.trim().is_empty() {
        return Err(AppError::bad("Enter your name."));
    }
    let user = create_user(&mut tx, &req.email, &req.name, &req.password).await?;
    tx.commit().await?;
    let jar = start_session(&state, jar, user.id).await?;
    Ok((jar, Json(user)).into_response())
}

#[derive(Deserialize)]
pub struct LoginRequest {
    email: String,
    password: String,
}

pub async fn login(
    State(state): State<AppState>,
    jar: CookieJar,
    Json(req): Json<LoginRequest>,
) -> AppResult<Response> {
    let row: Option<(Uuid, String, String, String)> =
        sqlx::query_as("select id, email, name, password_hash from users where email = $1")
            .bind(req.email.trim().to_lowercase())
            .fetch_optional(&state.pool)
            .await?;
    let Some((id, email, name, hash)) = row else {
        return Err(AppError::bad(
            "That email and password don't match an account.",
        ));
    };
    if !verify_password(&req.password, &hash) {
        return Err(AppError::bad(
            "That email and password don't match an account.",
        ));
    }
    let jar = start_session(&state, jar, id).await?;
    Ok((jar, Json(User { id, email, name })).into_response())
}

pub async fn logout(
    State(state): State<AppState>,
    jar: CookieJar,
    headers: axum::http::HeaderMap,
) -> AppResult<Response> {
    if let Some(token) = token_from(&jar, &headers) {
        sqlx::query("delete from sessions where token_hash = $1")
            .bind(hash_token(&token))
            .execute(&state.pool)
            .await?;
    }
    let jar = jar.remove(Cookie::build(COOKIE).path("/").build());
    Ok((jar, Json(json!({ "ok": true }))).into_response())
}

pub async fn me(Extension(user): Extension<User>) -> Json<User> {
    Json(user)
}

// Middleware ----------------------------------------------------------------------

fn token_from(jar: &CookieJar, headers: &axum::http::HeaderMap) -> Option<String> {
    if let Some(c) = jar.get(COOKIE) {
        return Some(c.value().to_string());
    }
    headers
        .get(header::AUTHORIZATION)
        .and_then(|v| v.to_str().ok())
        .and_then(|v| v.strip_prefix("Bearer "))
        .map(|s| s.trim().to_string())
}

async fn lookup_session(pool: &PgPool, token: &str) -> sqlx::Result<Option<User>> {
    sqlx::query_as::<_, User>(
        "select u.id, u.email, u.name from sessions s join users u on u.id = s.user_id
         where s.token_hash = $1 and s.expires_at > now()",
    )
    .bind(hash_token(token))
    .fetch_optional(pool)
    .await
}

pub async fn require_user(
    State(state): State<AppState>,
    jar: CookieJar,
    mut req: Request,
    next: Next,
) -> Result<Response, AppError> {
    let token = token_from(&jar, req.headers()).ok_or(AppError::Unauthorized)?;
    let user = lookup_session(&state.pool, &token)
        .await?
        .ok_or(AppError::Unauthorized)?;
    req.extensions_mut().insert(user);
    Ok(next.run(req).await)
}
