use std::env;

use anyhow::{Context, Result};

#[derive(Debug, Clone)]
pub struct Config {
    pub database_url: String,
    pub redis_url: String,
    pub huggingface_token: Option<String>,
    pub huggingface_model: String,
    /// Override for the `HuggingFace` Inference API base URL. Used by tests to
    /// point at a wiremock instance; production code leaves this `None`.
    pub huggingface_base_url: Option<String>,
    pub paapi: PaapiConfig,
}

#[derive(Debug, Clone)]
pub struct PaapiConfig {
    pub access_key: Option<String>,
    pub secret_key: Option<String>,
    pub partner_tag: Option<String>,
    pub host: String,
    pub region: String,
    /// "https" in prod, "http" in test (wiremock).
    pub scheme: String,
}

impl Config {
    pub fn from_env() -> Result<Self> {
        Ok(Self {
            database_url: env::var("DATABASE_URL")
                .context("DATABASE_URL must be set (see .env.example)")?,
            redis_url: env::var("REDIS_URL").unwrap_or_else(|_| "redis://localhost:6379".into()),
            huggingface_token: env::var("HUGGINGFACE_API_KEY").ok(),
            huggingface_model: env::var("HUGGINGFACE_EMBEDDING_MODEL")
                .unwrap_or_else(|_| "BAAI/bge-small-en-v1.5".into()),
            huggingface_base_url: env::var("HUGGINGFACE_BASE_URL").ok(),
            paapi: PaapiConfig {
                access_key: env::var("PAAPI_ACCESS_KEY").ok(),
                secret_key: env::var("PAAPI_SECRET_KEY").ok(),
                partner_tag: env::var("PAAPI_PARTNER_TAG").ok(),
                host: env::var("PAAPI_HOST").unwrap_or_else(|_| "webservices.amazon.com".into()),
                region: env::var("PAAPI_REGION").unwrap_or_else(|_| "us-east-1".into()),
                scheme: env::var("PAAPI_SCHEME").unwrap_or_else(|_| "https".into()),
            },
        })
    }
}
