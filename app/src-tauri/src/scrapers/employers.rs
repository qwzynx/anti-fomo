//! Where to fetch each employer's own job board.
//!
//! The three GitHub repos and Job Bank are aggregators, and aggregators decide
//! what to carry. A CIBC co-op posting exists on CIBC's Workday tenant and
//! nowhere else, which is why it never reached the feed. This table is the
//! list of boards we read directly.
//!
//! **Every row here was verified against the live endpoint**, not guessed.
//! Guessing does not work: `rbc.wd3` is a real tenant whose site is
//! `RBCGLOBAL1`, `scotiabank` is not on Workday at all, and `shopify.wd3`
//! answers 401. `cargo run --bin employer_check` re-runs that verification and
//! is the gate on adding a row.
//!
//! Prestige is deliberately *not* here — it lives in `crate::companies::TIERS`
//! keyed by `name`, so an employer we scrape and an employer that arrives
//! through Simplify cannot end up with two different tiers.

/// Which public API serves this employer's board, and the coordinates it needs.
#[derive(Clone, Copy, Debug)]
pub enum Board {
    /// `https://{tenant}.{host}.myworkdayjobs.com/wday/cxs/{tenant}/{site}/jobs`.
    /// `host` is the `wdN` shard, which differs per tenant and cannot be
    /// derived — DNS is a wildcard, so only the API answer distinguishes them.
    Workday {
        host: &'static str,
        tenant: &'static str,
        site: &'static str,
    },
    Greenhouse(&'static str),
    Lever(&'static str),
    Ashby(&'static str),
    SmartRecruiters(&'static str),
}

pub struct Employer {
    /// Canonical display name. Must match a `companies::TIERS` key to score.
    pub name: &'static str,
    pub board: Board,
}

const fn wd(name: &'static str, host: &'static str, tenant: &'static str, site: &'static str) -> Employer {
    Employer {
        name,
        board: Board::Workday { host, tenant, site },
    }
}
const fn gh(name: &'static str, slug: &'static str) -> Employer {
    Employer { name, board: Board::Greenhouse(slug) }
}
const fn lv(name: &'static str, slug: &'static str) -> Employer {
    Employer { name, board: Board::Lever(slug) }
}
const fn ab(name: &'static str, slug: &'static str) -> Employer {
    Employer { name, board: Board::Ashby(slug) }
}
const fn sr(name: &'static str, slug: &'static str) -> Employer {
    Employer { name, board: Board::SmartRecruiters(slug) }
}

pub const EMPLOYERS: &[Employer] = &[
    // --- Workday: Canadian banks, insurers and the big enterprise boards ---
    // These are general-purpose boards carrying every role the company has, so
    // the Workday scraper queries them by keyword rather than walking all of
    // them. CIBC alone is 543 postings, PwC is 4,338, and almost none of that
    // is engineering.
    wd("CIBC", "wd3", "cibc", "search"),
    wd("RBC", "wd3", "rbc", "RBCGLOBAL1"),
    // RBC keeps early-talent roles on a second site. It is small and it is
    // entirely co-op, internship and analyst-programme postings — exactly what
    // the aggregators miss.
    wd("RBC", "wd3", "rbc", "RBCEARLYTALENT1"),
    wd("TD", "wd3", "td", "TD_Bank_Careers"),
    wd("BMO", "wd3", "bmo", "External"),
    wd("Sun Life", "wd3", "sunlife", "Experienced"),
    wd("OMERS", "wd3", "omers", "omers_External"),
    wd("Thomson Reuters", "wd5", "thomsonreuters", "External_Career_Site"),
    wd("PwC", "wd3", "pwc", "Global_Experienced_Careers"),
    wd("Accenture", "wd103", "accenture", "accentureCareers"),
    wd("Morgan Stanley", "wd5", "ms", "External"),
    wd("Capital One", "wd12", "capitalone", "Capital_One"),
    wd("Visa", "wd5", "visa", "Visa"),
    wd("Mastercard", "wd1", "mastercard", "Campus"),
    wd("NVIDIA", "wd5", "nvidia", "NVIDIAExternalCareerSite"),
    wd("Salesforce", "wd12", "salesforce", "External_Career_Site"),
    wd("Cisco", "wd5", "cisco", "cisco_Careers"),
    wd("Intel", "wd1", "intel", "External"),
    wd("Adobe", "wd5", "adobe", "External_Experienced"),
    wd("HP", "wd5", "hp", "ExternalCareerSite"),
    wd("Autodesk", "wd1", "autodesk", "Ext"),
    wd("Workday", "wd5", "workday", "workday_Jobs"),

    // --- Greenhouse ---
    gh("Stripe", "stripe"),
    gh("Databricks", "databricks"),
    gh("Anthropic", "anthropic"),
    gh("Figma", "figma"),
    gh("Airbnb", "airbnb"),
    gh("DoorDash", "doordashusa"),
    gh("Instacart", "instacart"),
    gh("Discord", "discord"),
    gh("Reddit", "reddit"),
    gh("Pinterest", "pinterest"),
    gh("Roblox", "roblox"),
    gh("Coinbase", "coinbase"),
    gh("Robinhood", "robinhood"),
    gh("Affirm", "affirm"),
    gh("Brex", "brex"),
    gh("Mercury", "mercury"),
    gh("Chime", "chime"),
    gh("Gusto", "gusto"),
    gh("Datadog", "datadog"),
    gh("Cloudflare", "cloudflare"),
    gh("MongoDB", "mongodb"),
    gh("Elastic", "elastic"),
    gh("GitLab", "gitlab"),
    gh("Twilio", "twilio"),
    gh("Asana", "asana"),
    gh("Airtable", "airtable"),
    gh("Squarespace", "squarespace"),
    gh("Vercel", "vercel"),
    gh("Faire", "faire"),
    gh("Samsara", "samsara"),
    gh("Verkada", "verkada"),
    gh("Nuro", "nuro"),
    gh("Waymo", "waymo"),
    gh("Anduril", "andurilindustries"),
    gh("SpaceX", "spacex"),
    gh("Axon", "axon"),
    gh("Scale AI", "scaleai"),
    gh("Tenstorrent", "tenstorrent"),
    gh("Hootsuite", "hootsuite"),
    gh("Later", "later"),
    gh("Ritual", "ritual"),
    gh("LinkedIn", "LinkedIn"),
    // Quant: small boards, and the reason a student checks a job feed daily.
    gh("Jane Street", "janestreet"),
    gh("Jump Trading", "jumptrading"),
    gh("DRW", "drweng"),
    gh("Optiver", "optiverus"),
    gh("IMC Trading", "imc"),

    // --- SmartRecruiters ---
    sr("Ubisoft", "Ubisoft2"),

    // --- Lever ---
    lv("Palantir", "palantir"),
    lv("Wattpad", "wattpad"),

    // --- Ashby ---
    ab("OpenAI", "openai"),
    ab("Cohere", "cohere"),
    ab("Wealthsimple", "wealthsimple"),
    ab("1Password", "1password"),
    ab("Notion", "notion"),
    ab("Ramp", "ramp"),
    ab("Plaid", "plaid"),
    ab("Snowflake", "snowflake"),
    ab("Perplexity", "perplexity"),
    ab("Linear", "linear"),
    ab("Sentry", "sentry"),
    ab("Confluent", "confluent"),
    ab("Supabase", "supabase"),
    ab("Replit", "replit"),
    ab("Sierra", "sierra"),
    ab("Benchling", "benchling"),
    ab("Jobber", "jobber"),
    ab("Benevity", "benevity"),
    ab("Neo Financial", "neofinancial"),
    ab("Float", "float"),
];

/// The employers served by one ATS family, for the scraper that owns it.
pub fn of_kind(kind: fn(&Board) -> bool) -> impl Iterator<Item = &'static Employer> {
    EMPLOYERS.iter().filter(move |e| kind(&e.board))
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::collections::HashMap;

    #[test]
    fn every_employer_has_a_tier() {
        // A row here that `companies::TIERS` does not know would scrape fine
        // and then score zero on prestige, silently — the two tables drifting
        // apart is exactly what keying them both by `name` is meant to stop.
        let none = HashMap::new();
        let missing: Vec<_> = EMPLOYERS
            .iter()
            .map(|e| e.name)
            .filter(|n| crate::companies::tier(n, &none).is_none())
            .collect();
        assert!(missing.is_empty(), "employers with no tier: {missing:?}");
    }

    #[test]
    fn board_coordinates_are_not_empty() {
        for e in EMPLOYERS {
            match e.board {
                Board::Workday { host, tenant, site } => {
                    assert!(!host.is_empty() && !tenant.is_empty() && !site.is_empty(), "{}", e.name);
                }
                Board::Greenhouse(s) | Board::Lever(s) | Board::Ashby(s) | Board::SmartRecruiters(s) => {
                    assert!(!s.is_empty(), "{}", e.name);
                }
            }
        }
    }
}
