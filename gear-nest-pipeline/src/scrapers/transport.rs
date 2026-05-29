//! Anti-bot transport tiers (SPEC §7). A store's tier is selectable without
//! touching parsing or normalization — upgrading a store from clean HTTP to
//! headless is a one-file change. See ADR-013.

use std::env;
use std::time::Duration;

use anyhow::{Context, Result};
use async_trait::async_trait;
use reqwest::header::{HeaderMap, HeaderValue, ACCEPT, ACCEPT_LANGUAGE, USER_AGENT};
use reqwest::Client;

/// The transport tier a store is reached through.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tier {
    /// `reqwest` + cookie jar + browser headers, direct connection.
    CleanHttp,
    /// Same client, routed through `SCRAPE_PROXY_{STORE}` if set.
    Proxy,
    /// `chromiumoxide` browser pool — lands in Phase 2 PR4 (ADR-015).
    Headless,
}

impl Tier {
    /// Build the transport a store of this tier uses. `store_id` selects the
    /// per-store proxy credential for the proxy tier.
    pub fn transport(self, store_id: &str) -> Result<Box<dyn Transport>> {
        match self {
            Self::CleanHttp => Ok(Box::new(HttpTransport::new(None)?)),
            Self::Proxy => Ok(Box::new(HttpTransport::new(proxy_for(store_id))?)),
            Self::Headless => {
                anyhow::bail!("headless transport for {store_id} lands in Phase 2 PR4 (ADR-015)")
            }
        }
    }
}

/// Fetches a URL's body. The unit that parsing depends on, so each tier is
/// swappable behind it.
#[async_trait]
pub trait Transport: Send + Sync {
    async fn get(&self, url: &str) -> Result<String>;
}

/// Clean-HTTP and proxy tiers: a browser-shaped `reqwest` client with a cookie
/// jar and, optionally, a residential proxy.
struct HttpTransport {
    client: Client,
}

impl HttpTransport {
    fn new(proxy: Option<String>) -> Result<Self> {
        let mut builder = Client::builder()
            .timeout(Duration::from_secs(30))
            .cookie_store(true)
            .default_headers(browser_headers());
        if let Some(url) = proxy {
            let proxy = reqwest::Proxy::all(&url).context("invalid SCRAPE_PROXY url")?;
            builder = builder.proxy(proxy);
        }
        Ok(Self {
            client: builder.build().context("building HTTP transport")?,
        })
    }
}

#[async_trait]
impl Transport for HttpTransport {
    async fn get(&self, url: &str) -> Result<String> {
        let resp = self
            .client
            .get(url)
            .send()
            .await
            .with_context(|| format!("GET {url}"))?;
        let status = resp.status();
        let body = resp.text().await.unwrap_or_default();
        if !status.is_success() {
            anyhow::bail!("GET {url} -> HTTP {status}");
        }
        Ok(body)
    }
}

/// `SCRAPE_PROXY_{STORE}` (store id upper-cased) → proxy URL, if set non-empty.
fn proxy_for(store_id: &str) -> Option<String> {
    env::var(format!("SCRAPE_PROXY_{}", store_id.to_uppercase()))
        .ok()
        .filter(|v| !v.is_empty())
}

fn browser_headers() -> HeaderMap {
    // Gzip is negotiated by reqwest's gzip feature; do not set Accept-Encoding
    // manually or reqwest will not transparently decode the body. UA identifies
    // GearNest with a contact, per SPEC §7.
    let mut h = HeaderMap::new();
    h.insert(
        USER_AGENT,
        HeaderValue::from_static(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 \
             (KHTML, like Gecko) Chrome/124.0.0.0 Safari/537.36 \
             GearNest/0.1 (+https://gearnest.io; hello@gearnest.io)",
        ),
    );
    h.insert(
        ACCEPT,
        HeaderValue::from_static("text/html,application/xhtml+xml,application/xml;q=0.9,*/*;q=0.8"),
    );
    h.insert(ACCEPT_LANGUAGE, HeaderValue::from_static("en-US,en;q=0.9"));
    h
}
