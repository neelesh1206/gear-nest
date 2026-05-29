use anyhow::Result;
use chrono::Utc;
use clap::{Parser, Subcommand};
use tracing::info;
use tracing_subscriber::EnvFilter;

use gear_nest_pipeline::{
    config::Config,
    db,
    embeddings::HuggingFaceEmbedder,
    entity_resolution::Resolver,
    normalizer, price_history, price_sync,
    prices::PriceWriter,
    scrapers::{amazon::AmazonScraper, record_raw, StoreCrawler},
};

#[derive(Parser)]
#[command(
    name = "gear-nest-pipeline",
    about = "GearNest ingestion pipeline (scrape → normalize → resolve → embed → store)",
    version
)]
struct Cli {
    #[command(subcommand)]
    cmd: Cmd,
}

#[derive(Subcommand)]
enum Cmd {
    /// Apply outstanding migrations + idempotent partition DDL.
    Migrate,
    /// Scrape Amazon by ASIN list. Writes through the full pipeline:
    /// raw → normalize → resolve → embed specs → upsert listing.
    ScrapeAmazon {
        /// ASINs (positional, repeatable) or `--from-file path` to read newline-delimited.
        #[arg(required_unless_present = "from_file")]
        asins: Vec<String>,
        #[arg(long)]
        from_file: Option<std::path::PathBuf>,
    },
    /// Ensure the next two months of `price_history` partitions exist.
    EnsurePartitions,
    /// Run one full price sync across all 8 stores, then exit. Scheduling is
    /// external (Cloud Scheduler → one-shot Cloud Run Job), per ADR-0022.
    PriceSync,
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            EnvFilter::try_from_default_env().unwrap_or_else(|_| EnvFilter::new("info")),
        )
        .with_target(false)
        .compact()
        .init();

    let cli = Cli::parse();
    let cfg = Config::from_env()?;

    match cli.cmd {
        Cmd::Migrate => {
            let pool = db::connect(&cfg.database_url).await?;
            let dir = db::locate_migrations_dir()?;
            info!(migrations_dir = %dir.display(), "running migrations");
            db::migrations::run(&pool, &dir).await?;
            price_history::ensure_partitions(&pool, Utc::now()).await?;
        }
        Cmd::EnsurePartitions => {
            let pool = db::connect(&cfg.database_url).await?;
            price_history::ensure_partitions(&pool, Utc::now()).await?;
        }
        Cmd::PriceSync => run_price_sync(&cfg).await?,
        Cmd::ScrapeAmazon { asins, from_file } => {
            let asins = resolve_asins(asins, from_file.as_deref())?;
            info!(count = asins.len(), "scrape-amazon");

            let pool = db::connect(&cfg.database_url).await?;
            price_history::ensure_partitions(&pool, Utc::now()).await?;

            let scraper = AmazonScraper::new(cfg.paapi.clone())?;
            let resolver = Resolver::new(&pool);
            let embedder = HuggingFaceEmbedder::with_base_url(
                cfg.huggingface_token.clone(),
                cfg.huggingface_model.clone(),
                cfg.huggingface_base_url.clone(),
            )?;
            let mut price_writer = PriceWriter::connect(&cfg.redis_url).await?;

            let raws = scraper.fetch_batch(&asins).await?;
            info!(count = raws.len(), "fetched");

            for raw in &raws {
                let _audit_id = record_raw(&pool, raw).await?;
                let norm = normalizer::normalize(raw);
                let resolution = resolver.resolve(raw, &norm).await?;
                info!(
                    asin = raw.store_product_id.as_str(),
                    product_id = %resolution.product_id,
                    confidence = resolution.confidence.as_db_str(),
                    created = resolution.created,
                    "resolved"
                );

                if let Err(e) = gear_nest_pipeline::embeddings::embed_and_insert_product_specs(
                    &embedder,
                    &pool,
                    resolution.product_id,
                    norm.description.as_deref(),
                    norm.specs
                        .get("features")
                        .and_then(|v| v.as_array())
                        .map(|arr| {
                            arr.iter()
                                .filter_map(|x| x.as_str().map(str::to_string))
                                .collect::<Vec<_>>()
                        })
                        .unwrap_or_default()
                        .as_slice(),
                )
                .await
                {
                    tracing::warn!(error = %e, "spec embed failed (continuing)");
                }

                if let Some(price) = raw.price.as_deref() {
                    let listing_id =
                        listing_id_for(&pool, &raw.store_id, &raw.store_product_id).await?;
                    let now = Utc::now();
                    let payload = gear_nest_pipeline::prices::PricePayload {
                        listing_id,
                        price: price.to_string(),
                        in_stock: raw.in_stock,
                        fetched_at: now,
                        jitter_secs: 0,
                    };
                    price_writer
                        .write(resolution.product_id, &raw.store_id, payload)
                        .await?;
                    price_history::append(
                        &pool,
                        &gear_nest_pipeline::models::PriceRecord {
                            listing_id,
                            price: price.to_string(),
                            in_stock: raw.in_stock,
                            fetched_at: now,
                        },
                    )
                    .await?;
                }
            }
        }
    }
    Ok(())
}

async fn run_price_sync(cfg: &Config) -> Result<()> {
    let pool = db::connect(&cfg.database_url).await?;
    price_history::ensure_partitions(&pool, Utc::now()).await?;
    let mut writer = PriceWriter::connect(&cfg.redis_url).await?;
    let report = price_sync::run(cfg, &pool, &mut writer).await?;
    info!(
        synced = report.synced,
        skipped = report.skipped,
        failed = report.failed,
        "price-sync done"
    );
    Ok(())
}

async fn listing_id_for(
    pool: &sqlx::PgPool,
    store_id: &str,
    store_product_id: &str,
) -> Result<uuid::Uuid> {
    let (id,): (uuid::Uuid,) = sqlx::query_as(
        "SELECT id FROM store_listings WHERE store_id = $1 AND store_product_id = $2",
    )
    .bind(store_id)
    .bind(store_product_id)
    .fetch_one(pool)
    .await?;
    Ok(id)
}

fn resolve_asins(
    positional: Vec<String>,
    from_file: Option<&std::path::Path>,
) -> Result<Vec<String>> {
    if let Some(path) = from_file {
        let text = std::fs::read_to_string(path)?;
        Ok(text
            .lines()
            .map(str::trim)
            .filter(|l| !l.is_empty())
            .map(str::to_string)
            .collect())
    } else {
        Ok(positional)
    }
}
