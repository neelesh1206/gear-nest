//! Offline parser test for Backcountry reviews. Shared parser coverage lives
//! in `campsaver_reviews_parser.rs`; this test asserts only the
//! Backcountry-specific wiring (store id stamp + SFCC native review id form).

use gear_nest_pipeline::scrapers::jsonld::parse_reviews;

const STORE_ID: &str = "backcountry";
const PRODUCT_ID: &str = "PAT-HOUDINI-M-BLK";
const REVIEWS_HTML: &str = include_str!("fixtures/backcountry_reviews.html");

#[test]
fn parses_reviews_with_sfcc_native_ids() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    assert_eq!(reviews.len(), 2);
    for r in &reviews {
        assert_eq!(r.store_id, STORE_ID);
        assert_eq!(r.store_product_id, PRODUCT_ID);
        assert!(
            r.source_review_id
                .starts_with("https://www.backcountry.com/reviews/r/"),
            "expected SFCC native id, got {}",
            r.source_review_id
        );
        assert!(r.reviewer_id_hash.is_some(), "named author yields a hash");
    }
}
