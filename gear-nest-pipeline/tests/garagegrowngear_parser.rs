//! Offline parser tests for Garage Grown Gear (Shopify) against the shared
//! JSON-LD parser.
//!
//! Captured product page committed under `tests/fixtures/` so CI is
//! deterministic — no live site, no anti-bot, no network. This fixture exercises
//! Shopify-shaped JSON-LD: a variant-`Offer`-level `sku`, a plain-string
//! `brand`, and no GTIN (a cottage brand). See `docs/PHASE2.md`.

use gear_nest_pipeline::scrapers::jsonld::{parse_listing_urls, parse_price, parse_product};

const STORE_ID: &str = "garagerowngear";
const PRODUCT_HTML: &str = include_str!("fixtures/garagegrowngear_product.html");
const PRODUCT_URL: &str = "https://www.garagegrowngear.com/products/lunar-solo-tent";

#[test]
fn parses_core_product_fields() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert_eq!(p.store_id, "garagerowngear");
    assert_eq!(p.store_product_id, "SMD-LUNAR-SOLO-GRN");
    assert_eq!(p.title, "Six Moon Designs Lunar Solo Tent");
    assert_eq!(p.brand.as_deref(), Some("Six Moon Designs"));
    assert_eq!(p.price.as_deref(), Some("260.00"));
    assert_eq!(p.in_stock, Some(true));
}

#[test]
fn cottage_brand_has_no_gtin() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");
    assert_eq!(
        p.gtin, None,
        "no structural identifier — Tier-2 resolution territory"
    );
}

#[test]
fn store_product_id_falls_back_to_url_slug_without_sku() {
    // A product page with no sku anywhere → store_product_id is the URL slug.
    let html = r#"
      <script type="application/ld+json">
      { "@context": "https://schema.org/", "@type": "Product", "name": "Gatewood Cape",
        "offers": { "@type": "Offer", "price": "165.00", "availability": "InStock" } }
      </script>"#;
    let p = parse_product(
        html,
        "https://www.garagegrowngear.com/products/gatewood-cape",
        STORE_ID,
    )
    .expect("parse product");
    assert_eq!(p.store_product_id, "gatewood-cape");
}

#[test]
fn parses_breadcrumbs_rating_and_features() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert_eq!(p.category_path, vec!["Shelters", "Tents"]);
    assert!((p.store_rating.expect("rating") - 4.8).abs() < 0.001);
    assert_eq!(p.store_review_count, 52);
    assert!(p.features.iter().any(|f| f == "Trail Weight: 26 oz"));
    assert!(p
        .primary_image
        .as_deref()
        .is_some_and(|u| u.starts_with("https://")));
}

#[test]
fn fetch_price_reads_variant_offer() {
    let update = parse_price(PRODUCT_HTML, STORE_ID, "SMD-LUNAR-SOLO-GRN").expect("parse price");
    assert_eq!(update.store_id, "garagerowngear");
    assert_eq!(update.price.as_deref(), Some("260.00"));
    assert_eq!(update.in_stock, Some(true));
}

#[test]
fn listing_urls_dedupe_shopify_product_anchors() {
    let html = r#"
      <a href="/products/lunar-solo-tent"><img src="x.jpg"></a>
      <a href="/products/lunar-solo-tent">Lunar Solo</a>
      <a href="/collections/tents/products/gatewood-cape">Gatewood Cape</a>
      <a href="/pages/about-us">About</a>"#;
    let urls = parse_listing_urls(html, "https://www.garagegrowngear.com");
    assert_eq!(
        urls,
        vec![
            "https://www.garagegrowngear.com/products/lunar-solo-tent",
            "https://www.garagegrowngear.com/collections/tents/products/gatewood-cape"
        ]
    );
}
