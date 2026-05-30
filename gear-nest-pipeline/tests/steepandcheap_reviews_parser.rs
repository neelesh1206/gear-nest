//! Offline parser test for Steep & Cheap reviews. Shared parser coverage
//! lives in `campsaver_reviews_parser.rs`; this test asserts only the
//! `steepandcheap` wiring (store id stamp + short flash-sale-style reviews
//! still extract cleanly).

use gear_nest_pipeline::scrapers::jsonld::parse_reviews;

const STORE_ID: &str = "steepandcheap";
const PRODUCT_ID: &str = "MAR-PRECIPECO-M-NVY";
const REVIEWS_HTML: &str = include_str!("fixtures/steepandcheap_reviews.html");

#[test]
fn parses_short_flash_sale_reviews() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    assert_eq!(reviews.len(), 2);
    for r in &reviews {
        assert_eq!(r.store_id, STORE_ID);
        assert_eq!(r.store_product_id, PRODUCT_ID);
        // Flash-sale reviews tend to be terse — under the 150-char Stage-2
        // dedup threshold from ADR-011. We still extract them; Stage-1
        // (PR 7) covers the same-reviewer collision, Stage-2 (Phase 4)
        // simply skips them by design.
        assert!(r.body.len() < 150);
        assert!(r.reviewer_id_hash.is_some());
    }
}
