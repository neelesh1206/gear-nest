//! Offline parser tests for `CampSaver` reviews against the shared JSON-LD parser.
//!
//! Same offline-fixture discipline as `campsaver_parser.rs`: a captured
//! reviews page lives in `tests/fixtures/campsaver_reviews.html` so CI is
//! deterministic. See `docs/PHASE3.md`.

use gear_nest_pipeline::scrapers::jsonld::parse_reviews;

const STORE_ID: &str = "campsaver";
const PRODUCT_ID: &str = "NEM-DAGGER-OSMO-2P";
const REVIEWS_HTML: &str = include_str!("fixtures/campsaver_reviews.html");

#[test]
fn parses_three_unique_reviews_dropping_duplicate_source_id() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    assert_eq!(reviews.len(), 3, "duplicate @id should collapse to one row");
    let ids: Vec<&str> = reviews
        .iter()
        .map(|r| r.source_review_id.as_str())
        .collect();
    let unique: std::collections::HashSet<_> = ids.iter().copied().collect();
    assert_eq!(unique.len(), 3);
}

#[test]
fn carries_store_and_product_context_per_review() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    for r in &reviews {
        assert_eq!(r.store_id, STORE_ID);
        assert_eq!(r.store_product_id, PRODUCT_ID);
    }
}

#[test]
fn uses_native_id_when_present() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    let sarah = reviews
        .iter()
        .find(|r| r.title.as_deref() == Some("Best 2P backpacking tent I've owned"))
        .expect("sarah review present");
    assert_eq!(
        sarah.source_review_id,
        "https://www.campsaver.com/review/r-101"
    );
}

#[test]
fn falls_back_to_content_hash_when_native_id_missing() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    let anon = reviews
        .iter()
        .find(|r| r.body.starts_with("Good tent."))
        .expect("anon review present");
    assert!(
        anon.source_review_id.starts_with("sha256:"),
        "missing @id should fall back to content hash, got {}",
        anon.source_review_id
    );
}

#[test]
fn rating_is_clamped_and_parsed_from_string_or_number() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    let by_body: std::collections::HashMap<&str, i16> = reviews
        .iter()
        .map(|r| (r.body.as_str(), r.rating))
        .collect();
    assert_eq!(
        by_body
            .iter()
            .find(|(b, _)| b.starts_with("Used this on a week-long"))
            .map(|(_, &r)| r),
        Some(5)
    );
    assert_eq!(
        by_body
            .iter()
            .find(|(b, _)| b.starts_with("Pitches fast"))
            .map(|(_, &r)| r),
        Some(4)
    );
    assert_eq!(
        by_body
            .iter()
            .find(|(b, _)| b.starts_with("Good tent"))
            .map(|(_, &r)| r),
        Some(5)
    );
}

#[test]
fn hashes_named_reviewer_else_null() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);

    let sarah = reviews
        .iter()
        .find(|r| r.body.starts_with("Used this on a week-long"))
        .expect("sarah review");
    let mike = reviews
        .iter()
        .find(|r| r.body.starts_with("Pitches fast"))
        .expect("mike review");
    let anon = reviews
        .iter()
        .find(|r| r.body.starts_with("Good tent"))
        .expect("anon review");

    assert!(
        sarah.reviewer_id_hash.is_some(),
        "named author yields a hash"
    );
    assert!(
        mike.reviewer_id_hash.is_some(),
        "string author yields a hash"
    );
    assert_ne!(
        sarah.reviewer_id_hash, mike.reviewer_id_hash,
        "distinct authors hash distinctly"
    );
    assert!(
        anon.reviewer_id_hash.is_none(),
        "literal 'Anonymous' is not a stable identifier"
    );
}

/// The whole point of `reviews.reviewer_id_hash` per SPEC §13 Stage 1 is
/// to collapse the same reviewer across stores. The hash must therefore
/// be store-independent.
#[test]
fn same_reviewer_collides_across_stores_for_dedup() {
    let campsaver = parse_reviews(REVIEWS_HTML, "campsaver", PRODUCT_ID);
    let rei = parse_reviews(REVIEWS_HTML, "rei", PRODUCT_ID);
    let sarah_cs = campsaver
        .iter()
        .find(|r| r.body.starts_with("Used this on a week-long"))
        .unwrap();
    let sarah_rei = rei
        .iter()
        .find(|r| r.body.starts_with("Used this on a week-long"))
        .unwrap();
    assert_eq!(
        sarah_cs.reviewer_id_hash, sarah_rei.reviewer_id_hash,
        "same author name across stores must hash identically — \
         otherwise Stage-1 cross-store dedup (PR 7) cannot match"
    );
}

#[test]
fn parses_dates_from_bare_and_timestamped_iso() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    let sarah = reviews
        .iter()
        .find(|r| r.body.starts_with("Used this on a week-long"))
        .unwrap();
    let mike = reviews
        .iter()
        .find(|r| r.body.starts_with("Pitches fast"))
        .unwrap();
    assert_eq!(sarah.review_date.unwrap().to_string(), "2025-08-14");
    assert_eq!(
        mike.review_date.unwrap().to_string(),
        "2025-07-02",
        "T-suffixed timestamps narrow to the date"
    );
}

#[test]
fn body_whitespace_is_collapsed() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    let sarah = reviews
        .iter()
        .find(|r| r.title.as_deref() == Some("Best 2P backpacking tent I've owned"))
        .unwrap();
    assert!(
        !sarah.body.contains("  "),
        "runs of whitespace collapsed: {:?}",
        sarah.body
    );
}

#[test]
fn schema_org_defaults_for_unsupported_retailer_extensions() {
    let reviews = parse_reviews(REVIEWS_HTML, STORE_ID, PRODUCT_ID);
    for r in &reviews {
        assert!(!r.verified_purchase);
        assert_eq!(r.helpful_votes, 0);
    }
}

#[test]
fn empty_or_malformed_page_yields_no_reviews() {
    assert!(parse_reviews("<html></html>", STORE_ID, PRODUCT_ID).is_empty());
    let no_reviews = r#"
      <script type="application/ld+json">
      { "@context": "https://schema.org", "@type": "Product", "name": "Just a product" }
      </script>"#;
    assert!(parse_reviews(no_reviews, STORE_ID, PRODUCT_ID).is_empty());
}
