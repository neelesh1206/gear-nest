use std::path::{Path, PathBuf};
use std::time::Duration;

use anyhow::{Context, Result};
use sqlx::postgres::{PgPool, PgPoolOptions};
use tracing::{info, warn};

pub mod migrations;

/// Reasonable pool size for a batch ingestion job: enough concurrency to keep
/// the scrapers fed without monopolizing connections from the API service that
/// shares this Postgres in deployed environments.
const DEFAULT_MAX_CONNECTIONS: u32 = 8;
const ACQUIRE_TIMEOUT: Duration = Duration::from_secs(10);

pub async fn connect(database_url: &str) -> Result<PgPool> {
    let pool = PgPoolOptions::new()
        .max_connections(DEFAULT_MAX_CONNECTIONS)
        .acquire_timeout(ACQUIRE_TIMEOUT)
        .connect(database_url)
        .await
        .with_context(|| format!("connecting to Postgres at {}", redact(database_url)))?;

    let row: (i32,) = sqlx::query_as("SELECT 1").fetch_one(&pool).await?;
    debug_assert_eq!(row.0, 1);
    info!("postgres pool established (max {DEFAULT_MAX_CONNECTIONS})");
    Ok(pool)
}

/// Hide the password component of a libpq-style URL for logging.
fn redact(url: &str) -> String {
    if let Some((scheme_user, rest)) = url.split_once("://") {
        if let Some((creds, host)) = rest.split_once('@') {
            if let Some((user, _pw)) = creds.split_once(':') {
                return format!("{scheme_user}://{user}:***@{host}");
            }
        }
    }
    url.to_string()
}

/// Locate the workspace `supabase/migrations` directory by walking up from
/// `CARGO_MANIFEST_DIR`. We don't bake the path in because the migrations live
/// in the monorepo root, not inside this crate.
pub fn locate_migrations_dir() -> Result<PathBuf> {
    let manifest_dir = std::env::var("CARGO_MANIFEST_DIR")
        .map(PathBuf::from)
        .unwrap_or_else(|_| std::env::current_dir().unwrap_or_else(|_| PathBuf::from(".")));

    let mut cursor: Option<&Path> = Some(&manifest_dir);
    while let Some(dir) = cursor {
        let candidate = dir.join("supabase").join("migrations");
        if candidate.is_dir() {
            return Ok(candidate);
        }
        cursor = dir.parent();
    }
    warn!(
        "could not find supabase/migrations from {}",
        manifest_dir.display()
    );
    anyhow::bail!(
        "supabase/migrations directory not found anywhere above {}",
        manifest_dir.display()
    )
}
