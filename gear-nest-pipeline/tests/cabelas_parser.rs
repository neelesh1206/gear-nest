//! Offline parser test for Cabela's against the shared JSON-LD parser.
//!
//! The fixture is the page's DOM *as captured after a headless render* — that's
//! the only difference from the other tiers, since Cabela's injects its product
//! JSON-LD client-side. Parsing is identical and runs offline so CI stays
//! deterministic; the live browser pool is exercised only at runtime. See
//! `docs/PHASE2.md`.

use gear_nest_pipeline::scrapers::jsonld::{parse_price, parse_product};

const STORE_ID: &str = "cabelas";
const PRODUCT_HTML: &str = include_str!("fixtures/cabelas_product.html");
const PRODUCT_URL: &str = "https://www.cabelas.com/shop/en/yeti-tundra-45-hard-cooler";

#[test]
fn parses_core_product_fields() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert_eq!(p.store_id, "cabelas");
    assert_eq!(p.store_product_id, "CAB-YETI-T45-WHT");
    assert_eq!(p.title, "YETI Tundra 45 Hard Cooler");
    assert_eq!(p.brand.as_deref(), Some("YETI"));
    assert_eq!(p.gtin.as_deref(), Some("0888830000458"));
    assert_eq!(p.price.as_deref(), Some("325.00"));
    assert_eq!(p.in_stock, Some(true));
}

#[test]
fn parses_breadcrumbs_rating_and_features() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert_eq!(p.category_path, vec!["Camping", "Coolers"]);
    assert!((p.store_rating.expect("rating") - 4.9).abs() < 0.001);
    assert_eq!(p.store_review_count, 1342);
    assert!(p
        .features
        .iter()
        .any(|f| f == "Capacity: 28 cans (2:1 ice ratio)"));
}

#[test]
fn fetch_price_reads_offer() {
    let update = parse_price(PRODUCT_HTML, STORE_ID, "CAB-YETI-T45-WHT").expect("parse price");
    assert_eq!(update.price.as_deref(), Some("325.00"));
    assert_eq!(update.in_stock, Some(true));
}
