//! Location normalization, ported from the Python pipeline's location engine.
//! Raw location cells scraped from README tables get mapped to the structured
//! tags the UI filters on: modality, hub city, and region.

use regex::Regex;
use std::sync::LazyLock;

const MODALITY_KEYWORDS: &[(&str, &[&str])] = &[("Remote", &["remote"]), ("Hybrid", &["hybrid"])];

const CITY_KEYWORDS: &[(&str, &[&str])] = &[
    ("Toronto", &["toronto"]),
    ("Vancouver", &["vancouver"]),
    ("Waterloo", &["waterloo", "kitchener"]),
    (
        "San Francisco",
        &[
            "san francisco",
            "bay area",
            "mountain view",
            "palo alto",
            "sunnyvale",
            "san jose",
            "menlo park",
            "cupertino",
        ],
    ),
    ("New York", &["new york", "nyc", "brooklyn", "manhattan"]),
    ("Seattle", &["seattle", "bellevue", "redmond"]),
    ("London", &["london"]),
];

const CANADA_KEYWORDS: &[&str] = &[
    "canada",
    "toronto",
    "vancouver",
    "waterloo",
    "kitchener",
    "montreal",
    "ottawa",
    "calgary",
    "edmonton",
    ", on",
    ", bc",
    ", qc",
    ", ab",
    "ontario",
    "british columbia",
    "quebec",
    "alberta",
];

const USA_KEYWORDS: &[&str] = &[
    "usa",
    "united states",
    "u.s.",
    "san francisco",
    "bay area",
    "new york",
    "nyc",
    "seattle",
    "austin",
    "boston",
    "chicago",
    "mountain view",
    "palo alto",
    "sunnyvale",
    "san jose",
    "redmond",
    "bellevue",
    ", ca",
    ", ny",
    ", wa",
    ", tx",
    ", ma",
    ", il",
    ", ga",
    ", nc",
    ", va",
    ", pa",
    ", co",
    ", ut",
    ", az",
    ", fl",
];

static RE_BR: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)</?\s*br\s*/?>").unwrap());
static RE_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"<[^>]+>").unwrap());
static RE_WS: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"\s+").unwrap());

/// Collapses HTML break tags and whitespace in raw location cells.
/// `<br>` becomes a ` | ` separator because multi-location cells are common in
/// the internship repos and the UI splits on that pipe.
pub fn clean_location(raw: &str) -> String {
    let text = RE_BR.replace_all(raw, " | ");
    let text = RE_TAG.replace_all(&text, " ");
    RE_WS
        .replace_all(&text, " ")
        .trim_matches(|c: char| c.is_whitespace() || c == '|')
        .to_string()
}

/// Maps a raw location string to the tags the UI filters on:
/// modality (Remote/Hybrid/On-site), hub cities, and region.
pub fn normalize_location(raw: Option<&str>) -> Vec<String> {
    let Some(raw) = raw else {
        return Vec::new();
    };
    if raw.is_empty() {
        return Vec::new();
    }

    let text = raw.to_lowercase();
    let mut tags: Vec<String> = Vec::new();

    for (tag, kws) in MODALITY_KEYWORDS {
        if kws.iter().any(|kw| text.contains(kw)) {
            tags.push((*tag).to_string());
        }
    }
    // Anything that doesn't announce itself as remote or hybrid is on-site.
    if tags.is_empty() {
        tags.push("On-site".to_string());
    }

    for (city, kws) in CITY_KEYWORDS {
        if kws.iter().any(|kw| text.contains(kw)) {
            tags.push((*city).to_string());
        }
    }

    let in_canada = CANADA_KEYWORDS.iter().any(|kw| text.contains(kw));
    let in_usa = USA_KEYWORDS.iter().any(|kw| text.contains(kw));
    if in_canada {
        tags.push("Canada".to_string());
    }
    if in_usa {
        tags.push("USA".to_string());
    }
    // Two or more pipes means the cell listed several offices.
    if text.contains("multiple") || (in_canada && in_usa) || text.matches('|').count() >= 2 {
        tags.push("Global / Multi-region".to_string());
    }

    tags
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn cleans_br_tags_into_pipes() {
        assert_eq!(
            clean_location("Toronto, ON<br>New York, NY"),
            "Toronto, ON | New York, NY"
        );
        assert_eq!(clean_location("  <b>Remote</b>  "), "Remote");
        assert_eq!(clean_location("<br>Seattle<br>"), "Seattle");
    }

    #[test]
    fn tags_modality_city_and_region() {
        assert_eq!(
            normalize_location(Some("Toronto, ON | Remote")),
            vec!["Remote", "Toronto", "Canada"]
        );
    }

    #[test]
    fn defaults_to_on_site() {
        assert_eq!(
            normalize_location(Some("Seattle, WA")),
            vec!["On-site", "Seattle", "USA"]
        );
    }

    #[test]
    fn flags_multi_region_when_both_countries_match() {
        let tags = normalize_location(Some("Toronto, ON | New York, NY"));
        assert!(tags.contains(&"Canada".to_string()));
        assert!(tags.contains(&"USA".to_string()));
        assert!(tags.contains(&"Global / Multi-region".to_string()));
    }

    #[test]
    fn flags_multi_region_on_three_or_more_offices() {
        // Three USA offices: no cross-country match, but the pipe count trips it.
        let tags = normalize_location(Some("Seattle, WA | Austin, TX | Boston, MA"));
        assert!(tags.contains(&"Global / Multi-region".to_string()));
    }

    #[test]
    fn empty_and_missing_produce_no_tags() {
        assert!(normalize_location(None).is_empty());
        assert!(normalize_location(Some("")).is_empty());
    }

    #[test]
    fn hybrid_and_remote_can_coexist() {
        let tags = normalize_location(Some("Hybrid or Remote, Waterloo"));
        assert_eq!(tags[0], "Remote");
        assert_eq!(tags[1], "Hybrid");
        assert!(tags.contains(&"Waterloo".to_string()));
    }
}
