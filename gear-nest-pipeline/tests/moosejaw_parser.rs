//! Offline parser test for Moosejaw (Salesforce Commerce Cloud) against the
//! shared JSON-LD parser. This fixture exercises an `offers` array (sale price)
//! and `ratingCount` (vs `reviewCount`). See `docs/PHASE2.md`.

use gear_nest_pipeline::scrapers::jsonld::{parse_price, parse_product};

const STORE_ID: &str = "moosejaw";
const PRODUCT_HTML: &str = include_str!("fixtures/moosejaw_product.html");
const PRODUCT_URL: &str = "https://www.moosejaw.com/product/black-diamond-spot-400-headlamp";

#[test]
fn parses_core_product_fields() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert_eq!(p.store_id, "moosejaw");
    assert_eq!(p.store_product_id, "BD-SPOT-400-GRAPHITE");
    assert_eq!(p.title, "Black Diamond Spot 400 Headlamp");
    assert_eq!(p.brand.as_deref(), Some("Black Diamond"));
    assert_eq!(p.in_stock, Some(true));
}

#[test]
fn offers_array_yields_sale_price() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");
    assert_eq!(p.price.as_deref(), Some("37.46"));
}

#[test]
fn rating_count_alias_is_read() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");
    assert!((p.store_rating.expect("rating") - 4.7).abs() < 0.001);
    assert_eq!(p.store_review_count, 88);
    assert_eq!(p.gtin, None);
    assert_eq!(p.category_path, vec!["Camping", "Headlamps"]);
}

#[test]
fn fetch_price_reads_first_offer() {
    let update = parse_price(PRODUCT_HTML, STORE_ID, "BD-SPOT-400-GRAPHITE").expect("parse price");
    assert_eq!(update.price.as_deref(), Some("37.46"));
    assert_eq!(update.in_stock, Some(true));
}
