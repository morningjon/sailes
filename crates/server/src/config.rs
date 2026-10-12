use std::{env, net::SocketAddr, path::PathBuf};

use anyhow::Context;

#[derive(Clone, Debug)]
pub struct Config {
    pub database_url: String,
    pub bind: SocketAddr,
    pub web_dir: PathBuf,
    /// Set true when served over HTTPS so session cookies are marked Secure.
    pub secure_cookies: bool,
    pub admin_email: Option<String>,
    pub admin_password: Option<String>,
    pub admin_name: Option<String>,
    pub seed_demo: bool,
    /// Shared folder for the bridge (see bridge.rs). Off when unset.
    pub bridge_dir: Option<PathBuf>,
    pub bridge_interval_secs: u64,
}

fn flag(name: &str) -> bool {
    matches!(
        env::var(name)
            .ok()
            .as_deref()
            .map(str::to_ascii_lowercase)
            .as_deref(),
        Some("1" | "true" | "yes")
    )
}

impl Config {
    pub fn from_env() -> anyhow::Result<Self> {
        let database_url = env::var("DATABASE_URL").context("DATABASE_URL is not set")?;
        let bind = env::var("SAILES_BIND")
            .unwrap_or_else(|_| "0.0.0.0:8080".into())
            .parse()
            .context("SAILES_BIND must look like 0.0.0.0:8080")?;
        let web_dir = env::var("SAILES_WEB_DIR")
            .unwrap_or_else(|_| "web/dist".into())
            .into();
        Ok(Self {
            database_url,
            bind,
            web_dir,
            secure_cookies: flag("SAILES_SECURE_COOKIES"),
            admin_email: env::var("SAILES_ADMIN_EMAIL")
                .ok()
                .filter(|s| !s.is_empty()),
            admin_password: env::var("SAILES_ADMIN_PASSWORD")
                .ok()
                .filter(|s| !s.is_empty()),
            admin_name: env::var("SAILES_ADMIN_NAME").ok().filter(|s| !s.is_empty()),
            seed_demo: flag("SAILES_SEED_DEMO"),
            bridge_dir: env::var("SAILES_BRIDGE_DIR")
                .ok()
                .filter(|s| !s.is_empty())
                .map(PathBuf::from),
            bridge_interval_secs: env::var("SAILES_BRIDGE_INTERVAL_SECS")
                .ok()
                .and_then(|s| s.parse().ok())
                .unwrap_or(20)
                .max(2),
        })
    }
}
