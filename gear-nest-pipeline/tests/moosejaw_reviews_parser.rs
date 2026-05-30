//! Offline parser test for Moosejaw reviews. Shared parser coverage lives in
//! `campsaver_reviews_parser.rs`; this test asserts the Moosejaw-specific
//! wiring + content-hash fallback when a Review node omits `@id` (the SFCC
//! pattern when a review came from the older Bazaarvoice migration without
//! a stable URL).

use gear_nest_pipeline::scrapers::jsonld::parse_reviews;

const STORE_ID: &str = "moosejaw";
const PRODUCT_ID: &str = "TNF-CATSMEOW-20-LH";
const REVIEWS_HTML: &str = include_str!("fixtures/moosejaw_reviews.html");

#[test]
fn parses_mixed_id_and_content_hash_reviews() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    assert_eq!(reviews.len(), 2);
    for r in &reviews {
        assert_eq!(r.store_id, STORE_ID);
        assert_eq!(r.store_product_id, PRODUCT_ID);
    }

    let with_id = reviews
        .iter()
        .find(|r| r.body.starts_with("Used it down to 25F"))
        .expect("pete review");
    assert_eq!(
        with_id.source_review_id,
        "https://www.moosejaw.com/reviews/m-44502"
    );

    let id_less = reviews
        .iter()
        .find(|r| r.body.starts_with("Heavier than I expected"))
        .expect("anna review");
    assert!(
        id_less.source_review_id.starts_with("sha256:"),
        "missing @id should fall back to content hash, got {}",
        id_less.source_review_id
    );
}
