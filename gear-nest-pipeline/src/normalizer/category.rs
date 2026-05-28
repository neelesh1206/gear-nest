//! Category-tree to (category, subcategory) projector.
//!
//! Different retailers describe the same product with different breadcrumb
//! conventions ("Sports & Outdoors > Camping & Hiking > Tents" vs
//! "Camping > Shelter > Tents"). We project to a small canonical taxonomy.

const CANONICAL: &[(&[&str], &str, Option<&str>)] = &[
    (&["tent"], "shelter", Some("tent")),
    (&["tarp"], "shelter", Some("tarp")),
    (&["bivy", "bivouac"], "shelter", Some("bivy")),
    (
        &["sleeping bag", "sleeping bags"],
        "sleep",
        Some("sleeping-bag"),
    ),
    (
        &["sleeping pad", "sleeping pads"],
        "sleep",
        Some("sleeping-pad"),
    ),
    (&["quilt"], "sleep", Some("quilt")),
    (
        &["backpack", "backpacks", "packs"],
        "packs",
        Some("backpack"),
    ),
    (&["daypack", "daypacks"], "packs", Some("daypack")),
    (&["hydration pack"], "packs", Some("hydration")),
    (&["stove", "stoves"], "cooking", Some("stove")),
    (&["cookware", "pot", "pan"], "cooking", Some("cookware")),
    (
        &["water filter", "water purifier", "filtration"],
        "cooking",
        Some("water-treatment"),
    ),
    (&["jacket", "jackets"], "apparel", Some("jacket")),
    (&["pants"], "apparel", Some("pants")),
    (&["shirt", "shirts", "tee"], "apparel", Some("shirt")),
    (&["base layer", "baselayer"], "apparel", Some("base-layer")),
    (&["socks"], "apparel", Some("socks")),
    (&["boots", "boot"], "footwear", Some("boots")),
    (
        &["trail running", "trail runners"],
        "footwear",
        Some("trail-running"),
    ),
    (&["running shoes"], "footwear", Some("running-shoes")),
    (&["climbing shoes"], "footwear", Some("climbing-shoes")),
    (&["headlamp", "headlight"], "lighting", Some("headlamp")),
    (&["lantern"], "lighting", Some("lantern")),
    (&["gps", "watch"], "electronics", Some("watch")),
    (
        &["fitness", "strength", "barbell", "dumbbell"],
        "fitness",
        None,
    ),
    (&["yoga"], "fitness", Some("yoga")),
];

/// Map a breadcrumb path to (category, subcategory). Falls back to `"misc"`
/// when nothing matches — we'd rather see "misc" in the DB than guess.
pub fn map(path: &[String]) -> (String, Option<String>) {
    let joined = path.join(" > ").to_lowercase();
    for (needles, cat, sub) in CANONICAL {
        for needle in *needles {
            if joined.contains(needle) {
                return ((*cat).to_string(), sub.map(str::to_string));
            }
        }
    }
    ("misc".into(), None)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn maps_tent_path() {
        let path = vec![
            "Sports & Outdoors".to_string(),
            "Camping & Hiking".to_string(),
            "Tents".to_string(),
        ];
        assert_eq!(map(&path), ("shelter".into(), Some("tent".into())));
    }

    #[test]
    fn maps_trail_running_path() {
        let path = vec!["Footwear".to_string(), "Trail Running".to_string()];
        assert_eq!(
            map(&path),
            ("footwear".into(), Some("trail-running".into()))
        );
    }

    #[test]
    fn unknown_falls_back_to_misc() {
        let path = vec!["Pet Supplies".to_string()];
        assert_eq!(map(&path), ("misc".into(), None));
    }
}
