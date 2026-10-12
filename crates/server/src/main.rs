//! sAIles server: REST API plus the dashboard's static files.

mod auth;
mod config;
mod error;
mod fields;
mod routes;
mod seed;

use std::time::Duration;

use axum::{
    middleware,
    routing::{delete, get, post},
    Router,
};
use sqlx::{postgres::PgPoolOptions, PgPool};
use tower_http::{
    services::{ServeDir, ServeFile},
    trace::TraceLayer,
};
use tracing_subscriber::EnvFilter;

use crate::{config::Config, routes::*};

#[derive(Clone)]
pub struct AppState {
    pub pool: PgPool,
    pub config: Config,
}

fn api_router(state: AppState) -> Router<AppState> {
    let protected = Router::new()
        .route("/auth/me", get(auth::me))
        .route("/dashboard", get(dashboard::summary))
        .route("/search", get(search::search))
        .route("/contacts", get(contacts::list).post(contacts::create))
        .route(
            "/contacts/{id}",
            get(contacts::get)
                .patch(contacts::update)
                .delete(contacts::delete),
        )
        .route("/companies", get(companies::list).post(companies::create))
        .route(
            "/companies/{id}",
            get(companies::get)
                .patch(companies::update)
                .delete(companies::delete),
        )
        .route("/deals", get(deals::list).post(deals::create))
        .route(
            "/deals/{id}",
            get(deals::get).patch(deals::update).delete(deals::delete),
        )
        .route("/deals/{id}/contacts", post(deals::add_contact))
        .route(
            "/deals/{id}/contacts/{contact_id}",
            delete(deals::remove_contact),
        )
        .route("/pipelines", get(deals::pipelines))
        .route(
            "/activities",
            get(activities::timeline).post(activities::create),
        )
        .route("/activities/{id}", delete(activities::delete))
        .route("/tasks", get(tasks::list).post(tasks::create))
        .route(
            "/tasks/{id}",
            axum::routing::patch(tasks::update).delete(tasks::delete),
        )
        .route("/proposals", get(proposals::list).post(proposals::create))
        .route("/proposals/bulk", post(proposals::bulk))
        .route(
            "/proposals/{id}",
            get(proposals::get).patch(proposals::edit),
        )
        .route("/proposals/{id}/approve", post(proposals::approve))
        .route("/proposals/{id}/reject", post(proposals::reject))
        .route("/outbox", get(proposals::outbox))
        .route("/outbox/{id}", delete(proposals::cancel_outbox))
        .route_layer(middleware::from_fn_with_state(state, auth::require_user));

    Router::new()
        .route("/auth/status", get(auth::status))
        .route("/auth/setup", post(auth::setup))
        .route("/auth/login", post(auth::login))
        .route("/auth/logout", post(auth::logout))
        .route("/health", get(|| async { "ok" }))
        .merge(protected)
        .fallback(|| async { error::AppError::NotFound })
}

async fn connect(config: &Config) -> anyhow::Result<PgPool> {
    let mut attempt = 0;
    loop {
        match PgPoolOptions::new()
            .max_connections(10)
            .acquire_timeout(Duration::from_secs(10))
            .connect(&config.database_url)
            .await
        {
            Ok(pool) => return Ok(pool),
            Err(e) if attempt < 30 => {
                attempt += 1;
                tracing::warn!("database not ready ({e}); retrying in 2s");
                tokio::time::sleep(Duration::from_secs(2)).await;
            }
            Err(e) => return Err(e.into()),
        }
    }
}

async fn bootstrap(state: &AppState) -> anyhow::Result<()> {
    sqlx::migrate!("./migrations").run(&state.pool).await?;
    routes::deals::ensure_default_pipeline(&state.pool).await?;

    let c = &state.config;
    if let (Some(email), Some(password)) = (&c.admin_email, &c.admin_password) {
        if auth::user_count(&state.pool).await? == 0 {
            let name = c
                .admin_name
                .clone()
                .unwrap_or_else(|| email.split('@').next().unwrap_or("Admin").to_string());
            let mut conn = state.pool.acquire().await?;
            auth::create_user(&mut conn, email, &name, password)
                .await
                .map_err(|e| anyhow::anyhow!("couldn't create the admin user: {e}"))?;
            tracing::info!(%email, "created admin user from SAILES_ADMIN_EMAIL");
        }
    }
    if c.seed_demo {
        seed::seed_demo(&state.pool, false).await?;
    }
    Ok(())
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let _ = dotenvy::dotenv();
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| "sailes=info,tower_http=info".into()),
        )
        .init();

    let config = Config::from_env()?;
    let pool = connect(&config).await?;
    let state = AppState {
        pool,
        config: config.clone(),
    };

    let command = std::env::args().nth(1).unwrap_or_else(|| "serve".into());
    match command.as_str() {
        "serve" => {}
        "migrate" => {
            sqlx::migrate!("./migrations").run(&state.pool).await?;
            println!("Migrations applied.");
            return Ok(());
        }
        "seed-demo" => {
            sqlx::migrate!("./migrations").run(&state.pool).await?;
            routes::deals::ensure_default_pipeline(&state.pool).await?;
            seed::seed_demo(&state.pool, true).await?;
            println!("Demo data added.");
            return Ok(());
        }
        other => {
            eprintln!("Unknown command '{other}'. Use: serve | migrate | seed-demo");
            std::process::exit(2);
        }
    }

    bootstrap(&state).await?;

    let index = config.web_dir.join("index.html");
    let static_files = ServeDir::new(&config.web_dir).fallback(ServeFile::new(index));

    let app = Router::new()
        .nest("/api", api_router(state.clone()))
        .fallback_service(static_files)
        .layer(TraceLayer::new_for_http())
        .with_state(state);

    let listener = tokio::net::TcpListener::bind(config.bind).await?;
    tracing::info!("sAIles listening on http://{}", config.bind);
    axum::serve(listener, app)
        .with_graceful_shutdown(shutdown())
        .await?;
    Ok(())
}

async fn shutdown() {
    let ctrl_c = async {
        let _ = tokio::signal::ctrl_c().await;
    };
    #[cfg(unix)]
    let terminate = async {
        if let Ok(mut s) = tokio::signal::unix::signal(tokio::signal::unix::SignalKind::terminate())
        {
            s.recv().await;
        }
    };
    #[cfg(not(unix))]
    let terminate = std::future::pending::<()>();
    tokio::select! { _ = ctrl_c => {}, _ = terminate => {} }
}
