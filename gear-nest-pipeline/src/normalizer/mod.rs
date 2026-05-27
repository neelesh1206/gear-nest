//! Title / brand / category canonicalization.
//!
//! Inputs are messy retailer strings ("Mountain Safety Research Pocket Rocket II
//! Stove (Backpacking, Camping)"). Outputs are stable, deterministic canonical
//! values that feed entity resolution (ADR-007).

use serde_json::json;

use crate::models::{NormalizedProduct, RawProduct};

pub mod brand;
pub mod category;
pub mod text;

pub fn normalize(raw: &RawProduct) -> NormalizedProduct {
    let canonical_brand = brand::canonicalize(raw.brand.as_deref().unwrap_or(""));
    let canonical_brand = if canonical_brand.is_empty() {
        brand::infer_from_title(&raw.title)
    } else {
        canonical_brand
    };

    let cleaned_title = text::clean_title(&raw.title, canonical_brand.as_deref());
    let model_token = text::extract_model_token(&raw.title);
    let canonical_key = text::canonical_key(canonical_brand.as_deref(), model_token.as_deref());
    let slug = text::slugify(&cleaned_title);

    let (category, subcategory) = category::map(&raw.category_path);

    let mut specs = raw.specs.clone();
    if !raw.features.is_empty() {
        if let Some(obj) = specs.as_object_mut() {
            obj.insert("features".into(), json!(raw.features));
        } else {
            specs = json!({ "features": raw.features });
        }
    }

    NormalizedProduct {
        slug,
        name: cleaned_title,
        brand: canonical_brand.unwrap_or_else(|| "unknown".into()),
        category,
        subcategory,
        description: raw.description.clone(),
        specs,
        primary_image: raw.primary_image.clone(),
        gtin: raw.gtin.clone(),
        canonical_key,
    }
}
