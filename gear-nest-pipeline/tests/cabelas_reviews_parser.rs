//! Offline parser test for Cabela's reviews against a post-render snapshot.
//!
//! Shared parser coverage lives in `campsaver_reviews_parser.rs`; this test
//! asserts only the Cabela's-specific seam: the post-render snapshot's
//! Bazaarvoice `@graph` of standalone `Review` nodes (a different shape
//! from the `Product.review[]` array the other clean-HTTP stores embed),
//! plus correct handling of the literal "Anonymous" author the widget
//! emits for guest reviews.

use gear_nest_pipeline::scrapers::jsonld::parse_reviews;

const STORE_ID: &str = "cabelas";
const PRODUCT_ID: &str = "YETI-TUNDRA45-WHT";
const REVIEWS_HTML: &str = include_str!("fixtures/cabelas_reviews.html");

#[test]
fn parses_graph_of_standalone_review_nodes() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    assert_eq!(reviews.len(), 3);
    for r in &reviews {
        assert_eq!(r.store_id, STORE_ID);
        assert_eq!(r.store_product_id, PRODUCT_ID);
        assert!(
            r.source_review_id
                .starts_with("https://bazaarvoice.com/reviews/cabelas/"),
            "expected Bazaarvoice native id, got {}",
            r.source_review_id
        );
    }

    let anon = reviews
        .iter()
        .find(|r| r.body == "Good cooler. Cleans easily.")
        .expect("anon review");
    assert!(
        anon.reviewer_id_hash.is_none(),
        "Bazaarvoice 'Anonymous' guest reviews still must not produce a stable hash"
    );
}
