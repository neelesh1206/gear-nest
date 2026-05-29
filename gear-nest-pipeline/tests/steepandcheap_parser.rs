//! Offline parser test for Steep & Cheap (Salesforce Commerce Cloud) against the
//! shared JSON-LD parser. This fixture exercises flash-sale `LimitedAvailability`
//! (still purchasable) and a product with no `aggregateRating`. See
//! `docs/PHASE2.md`.

use gear_nest_pipeline::scrapers::jsonld::parse_product;

const STORE_ID: &str = "steepandcheap";
const PRODUCT_HTML: &str = include_str!("fixtures/steepandcheap_product.html");
const PRODUCT_URL: &str = "https://www.steepandcheap.com/marmot-precip-eco-jacket-mens";

#[test]
fn parses_core_product_fields() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");

    assert_eq!(p.store_id, "steepandcheap");
    assert_eq!(p.store_product_id, "MAR-PRECIP-ECO-BLU-M");
    assert_eq!(p.title, "Marmot PreCip Eco Rain Jacket - Men's");
    assert_eq!(p.brand.as_deref(), Some("Marmot"));
    assert_eq!(p.price.as_deref(), Some("67.48"));
    assert_eq!(p.category_path, vec!["Men's", "Rain Shells"]);
}

#[test]
fn limited_availability_counts_as_in_stock() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");
    assert_eq!(p.in_stock, Some(true));
}

#[test]
fn missing_aggregate_rating_is_none() {
    let p = parse_product(PRODUCT_HTML, PRODUCT_URL, STORE_ID).expect("parse product");
    assert_eq!(p.store_rating, None);
    assert_eq!(p.store_review_count, 0);
}

#[test]
fn out_of_stock_availability_is_false() {
    let html = r#"
      <script type="application/ld+json">
      { "@context": "http://schema.org", "@type": "Product", "name": "Sold Out Deal",
        "offers": { "@type": "Offer", "price": "12.00", "availability": "http://schema.org/OutOfStock" } }
      </script>"#;
    let p =
        parse_product(html, "https://www.steepandcheap.com/x", STORE_ID).expect("parse product");
    assert_eq!(p.in_stock, Some(false));
}
