//! Who the employer is, and what a role there is worth on a resume.
//!
//! Two things live here because they are the same question asked twice: the
//! name normalizer that decides "Alphabet Inc." and "Google" are one employer,
//! and the tier table that says how much weight a posting from that employer
//! carries.
//!
//! The tiers are a judgement call, deliberately so — there is no offline
//! signal that measures "how good this looks on a resume", and a derived proxy
//! (board size, whether they publish pay) measures something else while
//! looking authoritative. A shipped table is at least honest about being an
//! opinion, and the user can overrule any row of it from Settings.
//!
//! Coverage is not limited to the employers in `scrapers::employers`: most
//! postings still arrive through the three GitHub repos and Job Bank, and
//! those need a tier too.

use std::collections::HashMap;
use std::sync::OnceLock;

/// Tier 1 is the strongest resume signal, 4 the weakest one still worth
/// naming. An employer absent from the table scores nothing rather than
/// scoring badly — silence is not evidence.
pub const TIERS: &[(&str, u8)] = &[
    // --- Tier 1: names that carry an interview on their own ---
    ("Google", 1),
    ("Apple", 1),
    ("Microsoft", 1),
    ("Amazon", 1),
    ("Meta", 1),
    ("Netflix", 1),
    ("NVIDIA", 1),
    ("OpenAI", 1),
    ("Anthropic", 1),
    ("DeepMind", 1),
    ("Stripe", 1),
    ("Databricks", 1),
    ("Palantir", 1),
    ("SpaceX", 1),
    ("Waymo", 1),
    ("Jane Street", 1),
    ("Citadel", 1),
    ("Citadel Securities", 1),
    ("Two Sigma", 1),
    ("Hudson River Trading", 1),
    ("Jump Trading", 1),
    ("DRW", 1),
    ("Optiver", 1),
    ("IMC Trading", 1),
    ("Radix Trading", 1),
    ("Tower Research Capital", 1),
    ("Point72", 1),
    ("D. E. Shaw", 1),
    ("Goldman Sachs", 1),
    // --- Tier 2: strong, widely recognised engineering employers ---
    ("Shopify", 2),
    ("Salesforce", 2),
    ("Adobe", 2),
    ("Cisco", 2),
    ("Intel", 2),
    ("AMD", 2),
    ("Qualcomm", 2),
    ("Broadcom", 2),
    ("Arm", 2),
    ("Texas Instruments", 2),
    ("Oracle", 2),
    ("SAP", 2),
    ("VMware", 2),
    ("Dell", 2),
    ("HP", 2),
    ("IBM", 2),
    ("Uber", 2),
    ("Lyft", 2),
    ("Airbnb", 2),
    ("DoorDash", 2),
    ("Instacart", 2),
    ("Pinterest", 2),
    ("Reddit", 2),
    ("Snap", 2),
    ("Spotify", 2),
    ("Roblox", 2),
    ("Unity", 2),
    ("Electronic Arts", 2),
    ("Ubisoft", 2),
    ("Riot Games", 2),
    ("Epic Games", 2),
    ("Coinbase", 2),
    ("Robinhood", 2),
    ("Block", 2),
    ("PayPal", 2),
    ("Visa", 2),
    ("Mastercard", 2),
    ("American Express", 2),
    ("Capital One", 2),
    ("Morgan Stanley", 2),
    ("JPMorgan Chase", 2),
    ("BlackRock", 2),
    ("Snowflake", 2),
    ("Datadog", 2),
    ("Cloudflare", 2),
    ("MongoDB", 2),
    ("Elastic", 2),
    ("Confluent", 2),
    ("HashiCorp", 2),
    ("GitLab", 2),
    ("GitHub", 2),
    ("Atlassian", 2),
    ("LinkedIn", 2),
    ("Twilio", 2),
    ("Figma", 2),
    ("Notion", 2),
    ("Ramp", 2),
    ("Plaid", 2),
    ("Brex", 2),
    ("Affirm", 2),
    ("Discord", 2),
    ("Anduril", 2),
    ("Scale AI", 2),
    ("Perplexity", 2),
    ("Cohere", 2),
    ("Mistral AI", 2),
    ("Hugging Face", 2),
    ("Applied Intuition", 2),
    ("Rivian", 2),
    ("Tesla", 2),
    ("Lockheed Martin", 2),
    ("Boeing", 2),
    ("Raytheon", 2),
    // Canadian anchors — the employers a Toronto co-op term is measured against
    ("RBC", 2),
    ("TD", 2),
    ("CIBC", 2),
    ("BMO", 2),
    ("Scotiabank", 2),
    ("Wealthsimple", 2),
    ("Thomson Reuters", 2),
    ("Telus", 2),
    ("Rogers", 2),
    ("Bell", 2),
    ("Deloitte", 2),
    ("PwC", 2),
    ("KPMG", 2),
    ("EY", 2),
    ("Accenture", 2),
    ("Workday", 2),
    ("Autodesk", 2),
    ("ServiceNow", 2),
    ("Intuit", 2),
    ("Zoom", 2),
    ("Dropbox", 2),
    ("Slack", 2),
    ("Squarespace", 2),
    ("Asana", 2),
    // --- Tier 3: well-regarded, smaller or more specialised ---
    ("Sun Life", 3),
    ("Manulife", 3),
    ("Great-West Life", 3),
    ("OMERS", 3),
    ("CPP Investments", 3),
    ("OTPP", 3),
    ("Canadian Tire", 3),
    ("Loblaw", 3),
    ("Air Canada", 3),
    ("WestJet", 3),
    ("Hydro One", 3),
    ("Ontario Power Generation", 3),
    ("1Password", 3),
    ("Faire", 3),
    ("Vercel", 3),
    ("Supabase", 3),
    ("Replit", 3),
    ("Linear", 3),
    ("Sentry", 3),
    ("Sierra", 3),
    ("Mercury", 3),
    ("Retool", 3),
    ("Rippling", 3),
    ("Deel", 3),
    ("Gusto", 3),
    ("Chime", 3),
    ("Samsara", 3),
    ("Verkada", 3),
    ("Nuro", 3),
    ("Zipline", 3),
    ("Benchling", 3),
    ("Airtable", 3),
    ("Grammarly", 3),
    ("Axon", 3),
    ("Tenstorrent", 3),
    ("Xanadu", 3),
    ("D-Wave", 3),
    ("Untether AI", 3),
    ("Clio", 3),
    ("Jobber", 3),
    ("Hootsuite", 3),
    ("Later", 3),
    ("Benevity", 3),
    ("Neo Financial", 3),
    ("Float", 3),
    ("Wattpad", 3),
    ("Ritual", 3),
    ("Ada", 3),
    ("Coveo", 3),
    ("Kinaxis", 3),
    ("Lightspeed", 3),
    ("Nuvei", 3),
    ("Vidyard", 3),
    ("Ecobee", 3),
    ("Top Hat", 3),
    ("Wave", 3),
    ("Dialogue", 3),
    ("Konrad", 3),
    ("Index Exchange", 3),
    ("StackAdapt", 3),
    ("Cohere Health", 4),
];

/// Names that mean the same employer. Left side is what a source might say,
/// right side is the key in [`TIERS`].
const ALIASES: &[(&str, &str)] = &[
    ("alphabet", "Google"),
    ("google llc", "Google"),
    ("facebook", "Meta"),
    ("meta platforms", "Meta"),
    ("apple inc", "Apple"),
    ("amazon web services", "Amazon"),
    ("aws", "Amazon"),
    ("microsoft corporation", "Microsoft"),
    ("nvidia corporation", "NVIDIA"),
    ("x corp", "Twitter"),
    ("square", "Block"),
    ("block inc", "Block"),
    ("royal bank of canada", "RBC"),
    ("rbc royal bank", "RBC"),
    ("toronto dominion bank", "TD"),
    ("td bank", "TD"),
    ("td bank group", "TD"),
    ("canadian imperial bank of commerce", "CIBC"),
    ("bank of montreal", "BMO"),
    ("bmo financial group", "BMO"),
    ("bank of nova scotia", "Scotiabank"),
    ("sun life financial", "Sun Life"),
    ("manulife financial", "Manulife"),
    ("cppib", "CPP Investments"),
    ("canada pension plan investment board", "CPP Investments"),
    ("ontario teachers pension plan", "OTPP"),
    ("pricewaterhousecoopers", "PwC"),
    ("ernst young", "EY"),
    ("ernst and young", "EY"),
    ("jpmorgan", "JPMorgan Chase"),
    ("jp morgan", "JPMorgan Chase"),
    ("chase", "JPMorgan Chase"),
    ("american express company", "American Express"),
    ("amex", "American Express"),
    ("space exploration technologies", "SpaceX"),
    ("advanced micro devices", "AMD"),
    ("hewlett packard", "HP"),
    ("hp inc", "HP"),
    ("international business machines", "IBM"),
    ("ea", "Electronic Arts"),
    ("de shaw", "D. E. Shaw"),
    ("d e shaw", "D. E. Shaw"),
    ("hrt", "Hudson River Trading"),
    ("imc", "IMC Trading"),
    ("imc financial markets", "IMC Trading"),
    ("scale", "Scale AI"),
    ("openai inc", "OpenAI"),
    ("anthropic pbc", "Anthropic"),
    ("thomson reuters corporation", "Thomson Reuters"),
    ("telus communications", "Telus"),
    ("bell canada", "Bell"),
    ("bce", "Bell"),
    ("rogers communications", "Rogers"),
    ("shopify inc", "Shopify"),
    ("1password agilebits", "1Password"),
    ("agilebits", "1Password"),
];

/// Suffixes that are corporate boilerplate rather than identity. Stripped from
/// the end only — "Apple Inc." is Apple, but "Incorporated Analytics" is not.
const SUFFIXES: &[&str] = &[
    "inc", "inc.", "incorporated", "llc", "l.l.c", "ltd", "ltd.", "limited",
    "corp", "corp.", "corporation", "co", "co.", "company", "plc", "gmbh",
    "sa", "nv", "ag", "pbc", "lp", "llp", "group", "holdings", "technologies",
    "technology", "labs", "software", "systems",
];

/// Casefolds, drops punctuation, and strips corporate suffixes, so the same
/// employer written three ways by three sources lands on one key.
///
/// Suffix stripping is iterative — "Shopify Technologies Inc." has two — but
/// never empties the name: "Systems Ltd" keeps "systems" rather than becoming
/// nothing and matching every unknown employer.
fn normalize(name: &str) -> String {
    let cleaned: String = name
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();

    let mut words: Vec<&str> = cleaned.split_whitespace().collect();
    while words.len() > 1 {
        let last = words[words.len() - 1];
        if SUFFIXES.iter().any(|s| s.trim_end_matches('.') == last) {
            words.pop();
        } else {
            break;
        }
    }
    words.join(" ")
}

/// The two tables above, inverted once into normalized-key lookups.
///
/// Both were scanned linearly, re-normalizing every row on every call —
/// `normalize` allocates three times, so one `tier()` cost roughly 1,300
/// allocations and `personalize` called it once per posting. Against the real
/// cache (17,739 postings carrying a company) that is ~23 million allocations
/// and it dominated the whole ranking pass. The tables are `const`, so the
/// answer is the same every time and is worth building exactly once.
struct Lookups {
    /// Normalized name → canonical display name, from [`ALIASES`] and [`TIERS`].
    display: HashMap<String, &'static str>,
    /// Normalized display name → tier.
    tiers: HashMap<String, u8>,
}

fn lookups() -> &'static Lookups {
    static LOOKUPS: OnceLock<Lookups> = OnceLock::new();
    LOOKUPS.get_or_init(|| {
        let mut display = HashMap::new();
        // TIERS first, then ALIASES, so an alias wins the key it shares with a
        // table row — the same precedence the linear scan had.
        for (name, _) in TIERS {
            display.insert(normalize(name), *name);
        }
        for (alias, target) in ALIASES {
            display.insert(normalize(alias), *target);
        }
        let tiers = TIERS
            .iter()
            .map(|(name, tier)| (normalize(name), *tier))
            .collect();
        Lookups { display, tiers }
    })
}

/// Canonical display name for an employer, or the input trimmed when we have
/// never heard of them. Used so the employer filter and the A-Z sort do not
/// show "Shopify" and "Shopify Inc." as two companies.
pub fn canonical(name: &str) -> String {
    let key = normalize(name);
    if key.is_empty() {
        return name.trim().to_string();
    }
    match lookups().display.get(&key) {
        Some(display) => (*display).to_string(),
        None => name.trim().to_string(),
    }
}

/// The tier for an employer, with the user's overrides taking precedence over
/// the shipped table. `None` means "not in the table" — scored as zero, never
/// as a penalty, because an unlisted employer is unknown rather than bad.
///
/// Overrides are keyed by canonical display name, which is what Settings shows
/// and what [`canonical`] returns.
pub fn tier(name: &str, overrides: &HashMap<String, u8>) -> Option<u8> {
    let key = normalize(name);
    if key.is_empty() {
        return overrides.get(name.trim()).copied();
    }
    let lookups = lookups();
    let display = lookups.display.get(&key).copied();
    // The override is keyed by display name, which for an unknown employer is
    // the trimmed input rather than anything in the table.
    if let Some(t) = overrides.get(display.unwrap_or_else(|| name.trim())) {
        return Some(*t);
    }
    // An alias resolves to a display name that has its own normalized key.
    match display {
        Some(display) => lookups.tiers.get(&normalize(display)).copied(),
        None => None,
    }
}

/// What one tier is worth, as a 0..=1 multiplier for the prestige term.
/// Tier 4 is deliberately not zero: being a named employer at all beats an
/// anonymous one.
pub fn tier_value(tier: Option<u8>) -> f64 {
    match tier {
        Some(1) => 1.0,
        Some(2) => 0.7,
        Some(3) => 0.4,
        Some(4) => 0.2,
        _ => 0.0,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn strips_corporate_suffixes() {
        assert_eq!(normalize("Shopify Inc."), "shopify");
        assert_eq!(normalize("Apple Inc"), "apple");
        assert_eq!(normalize("Stripe, Inc."), "stripe");
    }

    #[test]
    fn suffix_stripping_never_empties_a_name() {
        // Otherwise every unknown employer would collapse onto one key.
        assert_eq!(normalize("Systems"), "systems");
        assert_eq!(normalize("Group"), "group");
    }

    #[test]
    fn aliases_resolve_to_one_employer() {
        let none = HashMap::new();
        assert_eq!(tier("Royal Bank of Canada", &none), tier("RBC", &none));
        assert_eq!(tier("Facebook", &none), tier("Meta", &none));
        assert_eq!(canonical("Alphabet"), "Google");
    }

    #[test]
    fn unknown_employers_score_nothing_rather_than_badly() {
        let none = HashMap::new();
        assert_eq!(tier("Some Local Consultancy", &none), None);
        assert_eq!(tier_value(None), 0.0);
    }

    #[test]
    fn user_overrides_beat_the_shipped_table() {
        let mut overrides = HashMap::new();
        overrides.insert("CIBC".to_string(), 1);
        assert_eq!(tier("CIBC", &overrides), Some(1));
        // And they apply through an alias, since Settings stores the canonical
        // name and a source may not use it.
        assert_eq!(tier("Canadian Imperial Bank of Commerce", &overrides), Some(1));
    }

    #[test]
    fn tier_ordering_is_monotonic() {
        assert!(tier_value(Some(1)) > tier_value(Some(2)));
        assert!(tier_value(Some(2)) > tier_value(Some(3)));
        assert!(tier_value(Some(4)) > tier_value(None));
    }
}
