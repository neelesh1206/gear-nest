//! Offline parser tests for `CampSaver` against the shared JSON-LD parser.
//!
//! Runs against a captured product page committed under `tests/fixtures/` so CI
//! is deterministic — no live site, no anti-bot, no network. Refresh the
//! fixture (a small PR) when `CampSaver`'s markup changes. See `docs/PHASE2.md`.

use gear_nest_pipeline::scrapers::jsonld::{parse_listing_urls, parse_price, parse_product};

const STORE_ID: &str = "campsaver";
const PRODUCT_HTML: &str = include_str!("fixtures/campsaver_product.html");
const PRODUCT_URL: &str = "https://www.campsaver.com/nemo-dagger-osmo-2p-tent.html";

#[test]
fn parses_core_product_fields() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert_eq!(p.store_id, "campsaver");
    assert_eq!(p.store_product_id, "NEM-DAGGER-OSMO-2P");
    assert_eq!(p.url, PRODUCT_URL);
    assert_eq!(p.title, "NEMO Dagger OSMO 2-Person Backpacking Tent");
    assert_eq!(p.brand.as_deref(), Some("NEMO Equipment"));
    assert_eq!(p.gtin.as_deref(), Some("0814041025478"));
    assert_eq!(p.price.as_deref(), Some("499.95"));
    assert_eq!(p.in_stock, Some(true));
}

#[test]
fn parses_breadcrumbs_skipping_home() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");
    assert_eq!(
        p.category_path,
        vec!["Tents & Shelters", "Backpacking Tents"]
    );
}

#[test]
fn parses_rating_image_and_features() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert!((p.store_rating.expect("rating") - 4.7).abs() < 0.001);
    assert_eq!(p.store_review_count, 38);
    assert_eq!(
        p.primary_image.as_deref(),
        Some("https://www.campsaver.com/media/catalog/product/n/e/nemo-dagger-osmo-2p.jpg")
    );
    assert!(p.features.iter().any(|f| f == "Capacity: 2-Person"));
    assert_eq!(
        p.specs.get("Floor Area").and_then(|v| v.as_str()),
        Some("31.3 sq ft")
    );
}

#[test]
fn description_whitespace_is_collapsed() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");
    let desc = p.description.expect("description");
    assert!(desc.starts_with("The NEMO Dagger OSMO 2P is an ultralight"));
    assert!(!desc.contains('\n'), "newlines collapsed to single spaces");
    assert!(!desc.contains("  "), "no runs of whitespace");
}

#[test]
fn fetch_price_reads_offer() {
    let update = parse_price(PRODUCT_HTML, STORE_ID, "NEM-DAGGER-OSMO-2P").expect("parse price");
    assert_eq!(update.store_id, "campsaver");
    assert_eq!(update.store_product_id, "NEM-DAGGER-OSMO-2P");
    assert_eq!(update.price.as_deref(), Some("499.95"));
    assert_eq!(update.in_stock, Some(true));
}

#[test]
fn missing_product_json_ld_is_an_error() {
    let html = "<html><head><title>nope</title></head><body></body></html>";
    assert!(parse_product(html, "https://www.campsaver.com/x.html", STORE_ID).is_err());
}

#[test]
fn listing_urls_from_item_list() {
    let html = r#"
      <script type="application/ld+json">
      {
        "@context": "https://schema.org",
        "@type": "ItemList",
        "itemListElement": [
          { "@type": "ListItem", "position": 1, "url": "https://www.campsaver.com/a.html" },
          { "@type": "ListItem", "position": 2, "url": "https://www.campsaver.com/b.html" }
        ]
      }
      </script>"#;
    let urls = parse_listing_urls(html, "https://www.campsaver.com");
    assert_eq!(
        urls,
        vec![
            "https://www.campsaver.com/a.html",
            "https://www.campsaver.com/b.html"
        ]
    );
}

#[test]
fn listing_urls_fall_back_to_anchors() {
    let html = r#"<ul>
      <li><a href="/nemo-dagger-osmo-2p-tent.html">Dagger</a></li>
      <li><a href="https://www.campsaver.com/msr-hubba.html">Hubba</a></li>
      <li><a href="/about">About</a></li>
    </ul>"#;
    let urls = parse_listing_urls(html, "https://www.campsaver.com");
    assert_eq!(
        urls,
        vec![
            "https://www.campsaver.com/nemo-dagger-osmo-2p-tent.html",
            "https://www.campsaver.com/msr-hubba.html"
        ]
    );
}
