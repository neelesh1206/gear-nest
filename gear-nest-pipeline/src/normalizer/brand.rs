//! Brand alias table — Tier-2 input for entity resolution (ADR-007).
//!
//! Each retailer prints brand names inconsistently ("Mountain Safety Research"
//! vs "MSR" vs "M.S.R."). We map every observed variant to a stable canonical
//! slug used by [`crate::entity_resolution`] when building the canonical key.

use std::collections::HashMap;

/// Canonical brand slug for a raw brand string. Returns `Some("msr")` for
/// any of "Mountain Safety Research", "M.S.R.", "MSR", etc. Empty input
/// returns an empty string so callers can fall back to title inference.
pub fn canonicalize(raw: &str) -> Option<String> {
    let key = normalize_lookup(raw);
    if key.is_empty() {
        return Some(String::new());
    }
    ALIASES
        .get(key.as_str())
        .copied()
        .map(str::to_string)
        .or(Some(key))
}

/// Heuristic: scan a product title for any aliased brand variant.
/// Used when the scraped record has no explicit brand field.
pub fn infer_from_title(title: &str) -> Option<String> {
    let lower = title.to_lowercase();
    // Prefer longer matches (e.g. "mountain safety research" over "msr").
    let mut best: Option<(&str, usize)> = None;
    for (alias, canon) in ALIASES.iter() {
        if lower.contains(alias) {
            let len = alias.len();
            if best.is_none_or(|(_, l)| len > l) {
                best = Some((canon, len));
            }
        }
    }
    best.map(|(c, _)| c.to_string())
}

fn normalize_lookup(s: &str) -> String {
    s.trim()
        .to_lowercase()
        .replace('.', "")
        .replace(['-', '_'], " ")
        .split_whitespace()
        .collect::<Vec<_>>()
        .join(" ")
}

/// Curated ~outdoor + fitness brand aliases. Coverage is intentionally narrow:
/// the long tail goes through Tier-3 embedding similarity (ADR-007 §trade-off).
static ALIASES: std::sync::LazyLock<HashMap<&'static str, &'static str>> =
    std::sync::LazyLock::new(|| {
        let pairs: &[(&str, &str)] = &[
            // MSR / Mountain Safety Research
            ("msr", "msr"),
            ("mountain safety research", "msr"),
            // Patagonia
            ("patagonia", "patagonia"),
            // Arc'teryx — note the curly apostrophe variants
            ("arcteryx", "arcteryx"),
            ("arc teryx", "arcteryx"),
            ("arc'teryx", "arcteryx"),
            // The North Face
            ("the north face", "the-north-face"),
            ("north face", "the-north-face"),
            ("tnf", "the-north-face"),
            // Black Diamond
            ("black diamond", "black-diamond"),
            ("bd", "black-diamond"),
            // Big Agnes
            ("big agnes", "big-agnes"),
            // Nemo Equipment
            ("nemo", "nemo"),
            ("nemo equipment", "nemo"),
            // Therm-a-Rest
            ("thermarest", "therm-a-rest"),
            ("therm a rest", "therm-a-rest"),
            // REI Co-op
            ("rei", "rei-co-op"),
            ("rei co op", "rei-co-op"),
            ("rei coop", "rei-co-op"),
            // Salomon
            ("salomon", "salomon"),
            // Hoka / Hoka One One
            ("hoka", "hoka"),
            ("hoka one one", "hoka"),
            // On / On Running
            ("on", "on-running"),
            ("on running", "on-running"),
            // Garmin
            ("garmin", "garmin"),
            // Suunto
            ("suunto", "suunto"),
            // Coros
            ("coros", "coros"),
            // Yeti
            ("yeti", "yeti"),
            ("yeti coolers", "yeti"),
            // Hydro Flask
            ("hydro flask", "hydro-flask"),
            ("hydroflask", "hydro-flask"),
            // Osprey
            ("osprey", "osprey"),
            ("osprey packs", "osprey"),
            // Deuter
            ("deuter", "deuter"),
            // Gregory
            ("gregory", "gregory"),
            ("gregory mountain products", "gregory"),
            // La Sportiva
            ("la sportiva", "la-sportiva"),
            // Scarpa
            ("scarpa", "scarpa"),
            // Merrell
            ("merrell", "merrell"),
            // Smartwool
            ("smartwool", "smartwool"),
            // Darn Tough
            ("darn tough", "darn-tough"),
            ("darn tough vermont", "darn-tough"),
            // Outdoor Research
            ("outdoor research", "outdoor-research"),
            ("or", "outdoor-research"),
            // Mountain Hardwear
            ("mountain hardwear", "mountain-hardwear"),
            // Marmot
            ("marmot", "marmot"),
            // Kelty
            ("kelty", "kelty"),
            // CamelBak
            ("camelbak", "camelbak"),
            // GoalZero
            ("goal zero", "goal-zero"),
            ("goalzero", "goal-zero"),
            // BioLite
            ("biolite", "biolite"),
            // Jetboil
            ("jetboil", "jetboil"),
            // Petzl
            ("petzl", "petzl"),
            // Fjallraven
            ("fjallraven", "fjallraven"),
            ("fjall raven", "fjallraven"),
            // Mammut
            ("mammut", "mammut"),
            // Sea to Summit
            ("sea to summit", "sea-to-summit"),
            // Klean Kanteen
            ("klean kanteen", "klean-kanteen"),
            // Rab
            ("rab", "rab"),
            // Helly Hansen
            ("helly hansen", "helly-hansen"),
            // Salewa
            ("salewa", "salewa"),
            // Rogue Fitness
            ("rogue", "rogue-fitness"),
            ("rogue fitness", "rogue-fitness"),
            // Concept2
            ("concept2", "concept2"),
            ("concept 2", "concept2"),
            // Garage Grown Gear in-house brands captured in raw form go through the
            // long tail; they are intentionally absent.
        ];
        pairs.iter().copied().collect()
    });

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn canonicalize_handles_punctuation_variants() {
        assert_eq!(canonicalize("M.S.R."), Some("msr".into()));
        assert_eq!(canonicalize("Mountain Safety Research"), Some("msr".into()));
        assert_eq!(canonicalize("Arc'teryx"), Some("arcteryx".into()));
        assert_eq!(
            canonicalize("THE NORTH FACE"),
            Some("the-north-face".into())
        );
    }

    #[test]
    fn unknown_brand_returns_normalized_passthrough() {
        assert_eq!(
            canonicalize("Some Cottage Brand"),
            Some("some cottage brand".into())
        );
    }

    #[test]
    fn empty_input_returns_empty_string() {
        assert_eq!(canonicalize(""), Some(String::new()));
    }

    #[test]
    fn infer_from_title_picks_longest_match() {
        assert_eq!(
            infer_from_title("Mountain Safety Research PocketRocket 2"),
            Some("msr".into())
        );
    }
}
