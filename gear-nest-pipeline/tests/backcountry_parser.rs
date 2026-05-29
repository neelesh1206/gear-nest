//! Offline parser test for Backcountry (Salesforce Commerce Cloud) against the
//! shared JSON-LD parser. Captured product page under `tests/fixtures/` keeps CI
//! deterministic; the proxy transport is only used on live runs. See
//! `docs/PHASE2.md`.

use gear_nest_pipeline::scrapers::jsonld::{parse_price, parse_product};

const STORE_ID: &str = "backcountry";
const PRODUCT_HTML: &str = include_str!("fixtures/backcountry_product.html");
const PRODUCT_URL: &str = "https://www.backcountry.com/patagonia-down-sweater-hoody-mens";

#[test]
fn parses_core_product_fields() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert_eq!(p.store_id, "backcountry");
    assert_eq!(p.store_product_id, "PAT0123-BLK-L");
    assert_eq!(p.title, "Patagonia Down Sweater Hoody - Men's");
    assert_eq!(p.brand.as_deref(), Some("Patagonia"));
    assert_eq!(p.price.as_deref(), Some("279.00"));
    assert_eq!(p.in_stock, Some(true));
}

#[test]
fn has_gtin_for_tier1_resolution() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");
    assert_eq!(p.gtin.as_deref(), Some("0193080000017"));
}

#[test]
fn parses_breadcrumbs_rating_and_single_image() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert_eq!(p.category_path, vec!["Men's Clothing", "Jackets"]);
    assert!((p.store_rating.expect("rating") - 4.6).abs() < 0.001);
    assert_eq!(p.store_review_count, 210);
    assert_eq!(
        p.primary_image.as_deref(),
        Some("https://content.backcountry.com/images/items/large/PAT/PAT0123/BLK.jpg")
    );
    assert!(p.features.iter().any(|f| f == "Insulation: 800-Fill Down"));
}

#[test]
fn fetch_price_reads_offer() {
    let update = parse_price(PRODUCT_HTML, STORE_ID, "PAT0123-BLK-L").expect("parse price");
    assert_eq!(update.price.as_deref(), Some("279.00"));
    assert_eq!(update.in_stock, Some(true));
}
