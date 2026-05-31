//! Offline parser test for REI reviews.
//!
//! REI is the one store where reviews are scrape-only (the CJ Affiliate API
//! has no review endpoint). The fixture is a REI product page snapshot;
//! this test asserts only the REI-specific seam — REI's product-URL native
//! `@id` form and that the scraper stamps `store_id = "rei"` even though
//! the rest of the REI scraper rides on the CJ-primary path.
//!
//! The async URL-or-CJ-lookup dispatch in `ReiScraper::fetch_reviews` is
//! the same shape as `fetch_price` and is covered by its existing
//! wiremock test (`rei_cj.rs`).

use gear_nest_pipeline::scrapers::jsonld::parse_reviews;

const STORE_ID: &str = "rei";
const PRODUCT_ID: &str = "BA-COPPERSPUR-HV-UL2";
const REVIEWS_HTML: &str = include_str!("fixtures/rei_reviews.html");

#[test]
fn parses_reviews_with_rei_native_ids() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    assert_eq!(reviews.len(), 2);
    for r in &reviews {
        assert_eq!(r.store_id, STORE_ID);
        assert_eq!(r.store_product_id, PRODUCT_ID);
        assert!(
            r.source_review_id
                .starts_with("https://www.rei.com/product/123456/reviews/r-"),
            "expected REI native review id, got {}",
            r.source_review_id
        );
        assert!(r.reviewer_id_hash.is_some());
    }
}
