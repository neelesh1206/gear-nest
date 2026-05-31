//! Offline parser test for Amazon reviews.
//!
//! Amazon is the only store where PA-API contributes nothing to individual
//! reviews — PA-API 5.0 returns only the aggregate `CustomerReviews.Count`
//! / `StarRating`, which `fetch_batch` already folds into `RawProduct`.
//! Individual reviews come from scraping `amazon.com/product-reviews/{ASIN}/`.
//! Shared parser coverage lives in `campsaver_reviews_parser.rs`; this test
//! asserts only the Amazon-specific seam (Amazon's `R{XXXX}` native review
//! id form and the literal `Amazon Customer` placeholder name).

use gear_nest_pipeline::scrapers::jsonld::parse_reviews;

const STORE_ID: &str = "amazon";
const PRODUCT_ID: &str = "B004E4AVOY";
const REVIEWS_HTML: &str = include_str!("fixtures/amazon_reviews.html");

#[test]
fn parses_reviews_with_amazon_native_review_ids() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    assert_eq!(reviews.len(), 3);
    for r in &reviews {
        assert_eq!(r.store_id, STORE_ID);
        assert_eq!(r.store_product_id, PRODUCT_ID);
        assert!(
            r.source_review_id
                .starts_with("https://www.amazon.com/gp/customer-reviews/R"),
            "expected Amazon native review id, got {}",
            r.source_review_id
        );
    }
}

#[test]
fn amazon_customer_placeholder_still_hashes() {
    // Amazon's "Amazon Customer" is a placeholder when a reviewer has no
    // public profile name. It is NOT the literal "Anonymous" the parser
    // suppresses, so it hashes like any other display name — multiple
    // distinct reviewers under the same placeholder will collide. The
    // dedup grouping `(product_id, reviewer_id_hash)` keeps the blast
    // radius narrow (one product), and a verified-purchase tiebreak
    // resolves the rare double.
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    let placeholder = reviews
        .iter()
        .find(|r| r.body.starts_with("Pitched it in the rain"))
        .expect("amazon-customer review");
    assert!(
        placeholder.reviewer_id_hash.is_some(),
        "'Amazon Customer' is a real display name, not the literal 'Anonymous' filter"
    );
}
