//! Title cleanup + canonical-key construction.

use deunicode::deunicode;
use once_cell::sync::Lazy;
use regex::Regex;
use unicode_normalization::UnicodeNormalization;

/// Strip leading brand prefix and trailing marketing parentheticals from a
/// noisy retailer title.
pub fn clean_title(title: &str, canonical_brand: Option<&str>) -> String {
    let nfc: String = title.nfc().collect();
    let mut s = deunicode(&nfc);

    // Drop trailing parenthesized marketing chunks: "...Stove (Backpacking, Camping)".
    s = PARENS_RE.replace_all(&s, "").to_string();
    // Collapse extra whitespace and trim.
    s = WHITESPACE_RE.replace_all(s.trim(), " ").to_string();

    if let Some(canon) = canonical_brand {
        let needle = canon.replace('-', " ");
        let lower = s.to_lowercase();
        if lower.starts_with(&needle) {
            s = s[needle.len()..]
                .trim_start_matches([':', '—', '-', ' '])
                .to_string();
        }
    }
    s
}

/// Extract a probable model token by collecting every word that contains
/// at least one digit and joining them with hyphens. Model numbers reliably
/// contain digits ("UL2", "X-Drive7", "PocketRocket2", "BR-3000"); pure
/// letter sequences are too ambiguous to be model identifiers and fall
/// through to Tier 3 embedding similarity.
pub fn extract_model_token(title: &str) -> Option<String> {
    let nfc: String = title.nfc().collect();
    let s = deunicode(&nfc);
    let tokens: Vec<String> = MODEL_RE
        .find_iter(&s)
        .map(|m| m.as_str().to_lowercase())
        .collect();
    if tokens.is_empty() {
        None
    } else {
        Some(tokens.join("-"))
    }
}

/// Stable canonical key for Tier-2 entity resolution. Two listings with the
/// same key are the same product with high confidence.
///
/// Format: `<brand-slug>:<model-token>` — both lowercase, both required.
/// Returns an empty string if either is missing; callers fall back to Tier 3.
pub fn canonical_key(brand: Option<&str>, model_token: Option<&str>) -> String {
    match (brand, model_token) {
        (Some(b), Some(m)) if !b.is_empty() && !m.is_empty() => format!("{b}:{m}"),
        _ => String::new(),
    }
}

/// URL-safe slug from a cleaned title.
pub fn slugify(input: &str) -> String {
    let lower = deunicode(input).to_lowercase();
    let cleaned = SLUG_RE.replace_all(&lower, "-");
    cleaned.trim_matches('-').to_string()
}

static PARENS_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s*\([^)]*\)\s*$").unwrap());
static WHITESPACE_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"\s+").unwrap());
static MODEL_RE: Lazy<Regex> = Lazy::new(|| {
    // Hyphenated alphanum with a digit (BR-3000) or any word containing
    // at least one digit (PocketRocket2, UL2, X005). Letter-only tokens
    // are intentionally excluded — they are too ambiguous to identify a model.
    Regex::new(r"\b(?:[A-Za-z]+-\d+|[A-Za-z]*\d+[A-Za-z0-9]*)\b").unwrap()
});
static SLUG_RE: Lazy<Regex> = Lazy::new(|| Regex::new(r"[^a-z0-9]+").unwrap());

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_title_strips_brand_prefix_and_parens() {
        let out = clean_title(
            "MSR PocketRocket 2 Stove (Backpacking, Camping)",
            Some("msr"),
        );
        assert_eq!(out, "PocketRocket 2 Stove");
    }

    #[test]
    fn extract_model_token_collects_digit_words() {
        assert_eq!(
            extract_model_token("MSR PocketRocket 2 Stove"),
            Some("2".into())
        );
        assert_eq!(
            extract_model_token("Big Agnes Copper Spur HV UL2 Tent"),
            Some("ul2".into())
        );
        assert_eq!(
            extract_model_token("Garmin Fenix 7 Pro Sapphire GPS Watch"),
            Some("7".into())
        );
        assert_eq!(
            extract_model_token("Test Item 3 Model X003"),
            Some("3-x003".into())
        );
        assert_eq!(extract_model_token("Just A Letter Title"), None);
    }

    #[test]
    fn canonical_key_skips_when_missing() {
        assert_eq!(canonical_key(Some("msr"), Some("2")), "msr:2");
        assert_eq!(canonical_key(None, Some("2")), "");
        assert_eq!(canonical_key(Some("msr"), None), "");
    }

    #[test]
    fn slugify_produces_url_safe_lowercase() {
        assert_eq!(slugify("MSR PocketRocket 2"), "msr-pocketrocket-2");
    }
}
