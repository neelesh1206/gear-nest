//! Offline tests for the REI / CJ affiliate client and the supplement merge.
//!
//! The CJ GraphQL API is stubbed with `wiremock`, so no live CJ creds or network
//! are needed — deterministic and CI-green, like the Amazon PA-API integration
//! test. A captured CJ response lives in `tests/fixtures/cj_rei_products.json`.

use serde_json::{json, Value};
use wiremock::matchers::{header, method};
use wiremock::{Mock, MockServer, ResponseTemplate};

use gear_nest_pipeline::config::CjConfig;
use gear_nest_pipeline::models::RawProduct;
use gear_nest_pipeline::scrapers::rei::{merge_supplement, parse_cj_response, CjClient};

const CJ_FIXTURE: &str = include_str!("fixtures/cj_rei_products.json");

#[test]
fn parses_cj_resultlist_with_sale_price_precedence() {
    let products = parse_cj_response(CJ_FIXTURE).expect("parse CJ response");
    assert_eq!(products.len(), 2);

    let tent = &products[0];
    assert_eq!(tent.store_id, "rei");
    assert_eq!(tent.store_product_id, "REI-1098765");
    assert_eq!(tent.title, "REI Co-op Half Dome SL 2+ Tent");
    assert_eq!(tent.brand.as_deref(), Some("REI Co-op"));
    assert_eq!(tent.gtin.as_deref(), Some("0099887766551"));
    // salePrice (209.93) takes precedence over the list price (279.00).
    assert_eq!(tent.price.as_deref(), Some("209.93"));
    assert_eq!(tent.in_stock, Some(true));
    assert_eq!(
        tent.category_path,
        vec!["Camp & Hike", "Tents", "Backpacking Tents"]
    );

    let bag = &products[1];
    assert_eq!(bag.price.as_deref(), Some("99.95"));
    assert_eq!(bag.in_stock, Some(false));
}

#[tokio::test]
async fn cj_client_posts_authorized_query_and_maps_results() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(header("authorization", "Bearer test-cj-key"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::from_str::<Value>(CJ_FIXTURE).unwrap()),
        )
        .mount(&server)
        .await;

    let client = CjClient::new(CjConfig {
        api_key: Some("test-cj-key".into()),
        website_id: Some("7654321".into()),
        advertiser_id: Some("1234567".into()),
        endpoint: format!("{}/query", server.uri()),
    })
    .expect("cj client");

    let products = client.search("tent").await.expect("cj search");
    assert_eq!(products.len(), 2);
    assert_eq!(products[0].store_product_id, "REI-1098765");
    assert_eq!(products[0].price.as_deref(), Some("209.93"));
}

#[tokio::test]
async fn cj_search_without_credentials_errors() {
    let client = CjClient::new(CjConfig {
        api_key: None,
        website_id: None,
        advertiser_id: None,
        endpoint: "http://127.0.0.1:1/query".into(),
    })
    .expect("cj client");
    assert!(client.search("tent").await.is_err());
}

#[test]
fn merge_keeps_cj_commerce_fields_and_fills_blanks_from_scrape() {
    let cj = RawProduct {
        store_id: "rei".into(),
        store_product_id: "REI-1098765".into(),
        url: "https://www.rei.com/product/1098765/x".into(),
        title: "REI Co-op Half Dome SL 2+ Tent".into(),
        brand: Some("REI Co-op".into()),
        category_path: vec![],
        description: None,
        features: vec![],
        specs: json!({}),
        primary_image: Some("https://www.rei.com/media/1098765.jpg".into()),
        gtin: Some("0099887766551".into()),
        price: Some("209.93".into()),
        in_stock: Some(true),
        store_rating: None,
        store_review_count: 0,
        raw_payload: json!({"source": "cj"}),
    };
    let scraped = RawProduct {
        store_id: "rei".into(),
        store_product_id: "ignored".into(),
        url: "ignored".into(),
        title: "ignored".into(),
        brand: Some("ignored".into()),
        category_path: vec!["Camp & Hike".into(), "Tents".into()],
        description: Some("Long scraped description.".into()),
        features: vec!["Capacity: 2-Person".into()],
        specs: json!({"Minimum Weight": "3 lbs 14 oz"}),
        primary_image: Some("ignored.jpg".into()),
        gtin: Some("ignored".into()),
        price: Some("999.99".into()),
        in_stock: Some(false),
        store_rating: Some(4.5),
        store_review_count: 87,
        raw_payload: json!({"source": "scrape"}),
    };

    let merged = merge_supplement(cj, scraped);

    // CJ wins for commerce + identity fields.
    assert_eq!(merged.price.as_deref(), Some("209.93"));
    assert_eq!(merged.in_stock, Some(true));
    assert_eq!(merged.gtin.as_deref(), Some("0099887766551"));
    assert_eq!(merged.store_product_id, "REI-1098765");
    assert_eq!(merged.raw_payload, json!({"source": "cj"}));
    // Scrape fills what CJ left blank.
    assert_eq!(
        merged.description.as_deref(),
        Some("Long scraped description.")
    );
    assert_eq!(merged.category_path, vec!["Camp & Hike", "Tents"]);
    assert_eq!(merged.features, vec!["Capacity: 2-Person"]);
    assert_eq!(merged.specs, json!({"Minimum Weight": "3 lbs 14 oz"}));
    assert_eq!(merged.store_rating, Some(4.5));
    assert_eq!(merged.store_review_count, 87);
}

#[test]
fn merge_does_not_overwrite_present_supplement_fields() {
    // Sanity: when CJ already has a description, the scrape must not clobber it.
    let mut cj = sample_cj();
    cj.description = Some("CJ description".into());
    let mut scraped = sample_cj();
    scraped.description = Some("scraped description".into());
    let merged = merge_supplement(cj, scraped);
    assert_eq!(merged.description.as_deref(), Some("CJ description"));
}

#[test]
fn scraped_gtin_is_never_adopted() {
    // GTIN drives Tier-1 entity resolution (ADR-007); REI trusts only CJ's GTIN
    // (ADR-0023). A scraped GTIN must not be used even when CJ omits one.
    let mut cj = sample_cj();
    cj.gtin = None;
    let mut scraped = sample_cj();
    scraped.gtin = Some("0000000000000".into());
    let merged = merge_supplement(cj, scraped);
    assert_eq!(merged.gtin, None);
}

fn sample_cj() -> RawProduct {
    RawProduct {
        store_id: "rei".into(),
        store_product_id: "REI-1".into(),
        url: "https://www.rei.com/product/1/x".into(),
        title: "Sample".into(),
        brand: None,
        category_path: vec![],
        description: None,
        features: vec![],
        specs: json!({}),
        primary_image: None,
        gtin: None,
        price: Some("10.00".into()),
        in_stock: Some(true),
        store_rating: None,
        store_review_count: 0,
        raw_payload: Value::Null,
    }
}
