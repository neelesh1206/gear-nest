//! Offline parser tests for Garage Grown Gear reviews.
//!
//! The shared parser is already exercised by `campsaver_reviews_parser.rs`;
//! these tests cover the GGG-specific seam: Shopify URL-fragment review IDs
//! (`#review-N`), the store-seed slug typo (`garagerowngear`, migration
//! 0001), and a cross-store reviewer-hash collision with `CampSaver`'s fixture
//! that proves the SPEC §13 Stage-1 dedup precondition holds end-to-end
//! between two store scrapers.

use gear_nest_pipeline::scrapers::jsonld::parse_reviews;

const STORE_ID: &str = "garagerowngear";
const PRODUCT_ID: &str = "SMD-LUNAR-SOLO-GRN";
const REVIEWS_HTML: &str = include_str!("fixtures/garagegrowngear_reviews.html");
const CAMPSAVER_REVIEWS_HTML: &str = include_str!("fixtures/campsaver_reviews.html");

#[test]
fn parses_three_reviews_with_shopify_fragment_ids() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    assert_eq!(reviews.len(), 3);
    for r in &reviews {
        assert_eq!(r.store_id, STORE_ID);
        assert_eq!(r.store_product_id, PRODUCT_ID);
        assert!(
            r.source_review_id.contains("#review-"),
            "expected Shopify fragment id, got {}",
            r.source_review_id
        );
    }
}

#[test]
fn same_reviewer_across_garagegrowngear_and_campsaver_collides() {
    // Both fixtures carry a "Sarah K." review. Stage-1 cross-store dedup
    // (PR 7) groups by (product_id, reviewer_id_hash), so the hashes from
    // two different scrapers must match.
    let ggg = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    let cs = parse_reviews(CAMPSAVER_REVIEWS_HTML, "campsaver", "NEM-DAGGER-OSMO-2P");
    let sarah_ggg = ggg
        .iter()
        .find(|r| r.body.starts_with("Light and roomy"))
        .expect("sarah review in GGG fixture");
    let sarah_cs = cs
        .iter()
        .find(|r| r.body.starts_with("Used this on a week-long"))
        .expect("sarah review in CampSaver fixture");
    assert!(sarah_ggg.reviewer_id_hash.is_some());
    assert_eq!(
        sarah_ggg.reviewer_id_hash, sarah_cs.reviewer_id_hash,
        "Sarah K. on GGG must hash identically to Sarah K. on CampSaver — \
         otherwise Stage-1 cross-store dedup cannot collapse them"
    );
}
