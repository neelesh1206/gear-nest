//! Lightweight migration runner.
//!
//! We do not use `sqlx::migrate!()` because the migrations live outside this
//! crate (`../supabase/migrations`, Session-0 owned) and we want the pipeline
//! to be able to apply them in deployed environments where the source tree
//! may not be present. Instead we read the directory at runtime, hash each
//! file's contents, and record applied migrations in `_gn_migrations`.
//!
//! Filenames follow Session 0's convention: `NNNN_<slug>.sql` where `NNNN`
//! is a zero-padded integer ordering key.

use std::path::{Path, PathBuf};

use anyhow::{Context, Result};
use sha2::{Digest, Sha256};
use sqlx::postgres::PgPool;
use tracing::{info, warn};

#[derive(Debug)]
struct Migration {
    version: i64,
    name: String,
    sql: String,
    checksum: String,
}

pub async fn run(pool: &PgPool, dir: &Path) -> Result<()> {
    sqlx::query(
        r#"
        CREATE TABLE IF NOT EXISTS _gn_migrations (
            version     BIGINT PRIMARY KEY,
            name        TEXT NOT NULL,
            checksum    TEXT NOT NULL,
            applied_at  TIMESTAMPTZ NOT NULL DEFAULT NOW()
        )
        "#,
    )
    .execute(pool)
    .await
    .context("creating _gn_migrations table")?;

    let migrations =
        discover(dir).with_context(|| format!("scanning migrations in {}", dir.display()))?;

    if migrations.is_empty() {
        warn!("no migration files found in {}", dir.display());
        return Ok(());
    }

    for m in migrations {
        let row: Option<(String,)> =
            sqlx::query_as("SELECT checksum FROM _gn_migrations WHERE version = $1")
                .bind(m.version)
                .fetch_optional(pool)
                .await?;

        if let Some((existing,)) = row {
            if existing != m.checksum {
                anyhow::bail!(
                    "migration {} ({}) checksum drift: stored={}, on-disk={}. \
                     Migrations are immutable — add a new numbered file instead.",
                    m.version,
                    m.name,
                    existing,
                    m.checksum
                );
            }
            continue;
        }

        info!(version = m.version, name = %m.name, "applying migration");
        let mut tx = pool.begin().await?;
        sqlx::raw_sql(&m.sql)
            .execute(&mut *tx)
            .await
            .with_context(|| format!("executing migration {} ({})", m.version, m.name))?;
        sqlx::query("INSERT INTO _gn_migrations (version, name, checksum) VALUES ($1, $2, $3)")
            .bind(m.version)
            .bind(&m.name)
            .bind(&m.checksum)
            .execute(&mut *tx)
            .await?;
        tx.commit().await?;
    }

    Ok(())
}

fn discover(dir: &Path) -> Result<Vec<Migration>> {
    let mut entries: Vec<PathBuf> = std::fs::read_dir(dir)?
        .filter_map(std::result::Result::ok)
        .map(|e| e.path())
        .filter(|p| p.extension().and_then(|e| e.to_str()) == Some("sql"))
        .collect();
    entries.sort();

    entries
        .into_iter()
        .map(|path| {
            let stem = path
                .file_stem()
                .and_then(|s| s.to_str())
                .with_context(|| format!("bad migration filename: {}", path.display()))?;
            let (ver, name) = stem
                .split_once('_')
                .with_context(|| format!("migration {stem} must start with NNNN_"))?;
            let version: i64 = ver
                .parse()
                .with_context(|| format!("migration {stem} version is not an integer"))?;
            let sql = std::fs::read_to_string(&path)
                .with_context(|| format!("reading {}", path.display()))?;
            let mut hasher = Sha256::new();
            hasher.update(sql.as_bytes());
            let checksum = hex::encode(hasher.finalize());
            Ok(Migration {
                version,
                name: name.to_string(),
                sql,
                checksum,
            })
        })
        .collect()
}
