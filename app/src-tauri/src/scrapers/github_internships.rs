//! The three community job repos (Pitt CSC, Simplify, New Grad Positions).
//!
//! These used to be parsed out of the HTML `<table>` blocks in each README.
//! They are read from `.github/scripts/listings.json` instead, which the same
//! repos publish beside it, because the JSON is better in every way that
//! matters here:
//!
//! - **Real `date_posted` timestamps.** The README only carries a relative
//!   "Age" cell (`13d`, `1mo`), and anything unparseable fell back to
//!   `Utc::now()`. Measured against the JSON, that was stamping Pitt CSC's
//!   dormant listings — newest genuinely posted 2026-06-01 — as posted today,
//!   handing a stale repo the full recency bonus. Exactly the failure mode
//!   `CLAUDE.md` warns about.
//! - **`active` / `is_visible` flags**, so closed listings are dropped rather
//!   than ranked.
//! - **Clean apply URLs**, with no `?utm_source=Simplify&ref=Simplify` suffix.
//! - **The simplify.jobs posting `id`**, which is what lets the enrichment
//!   pass fetch real requirements and perks for these listings.
//!
//! The files are ~12 MB raw but ~1.5 MB on the wire (the client has gzip and
//! brotli enabled) and are only fetched on an actual refresh, which is not on
//! a timer.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, TimeZone, Utc};
use serde::Deserialize;

use super::Scraper;
use crate::models::{Item, ItemType};

/// Newest active listings kept per repo.
///
/// Measured on a live fetch: 1,946 / 1,364 / 3,162 active rows, of which
/// 1,458 / 0 / 2,343 fall inside the 60-day retention window that
/// `prune_older_than` enforces anyway. 600 keeps a full window's worth of the
/// two live repos without letting one source contribute several thousand rows:
/// round-robin diversification protects the first page but not the size of
/// the cache.
const MAX_LISTINGS: usize = 600;

/// Rows the repos attribute to Simplify itself carry a posting id that
/// resolves on simplify.jobs; community-contributed rows 404 there. Measured
/// coverage: 98.0% / 90.9% / 99.5%. Checking the field costs nothing and saves
/// a doomed request per community row.
const SIMPLIFY_SOURCE: &str = "Simplify";

#[derive(Clone, Copy)]
pub enum TitleStyle {
    /// "Internship at {company}" — Pitt CSC
    CompanyOnly,
    /// "{role} at {company}" — Simplify
    RoleAtCompany,
}

pub struct GithubInternships {
    pub name: &'static str,
    pub url: &'static str,
    pub title_style: TitleStyle,
    pub item_type: ItemType,
}

pub const PITT_CSC: GithubInternships = GithubInternships {
    name: "Pitt CSC Repo",
    url: "https://raw.githubusercontent.com/pittcsc/Summer2026-Internships/dev/.github/scripts/listings.json",
    title_style: TitleStyle::CompanyOnly,
    item_type: ItemType::Internship,
};

pub const SIMPLIFY: GithubInternships = GithubInternships {
    name: "Simplify",
    url: "https://raw.githubusercontent.com/SimplifyJobs/Summer2026-Internships/dev/.github/scripts/listings.json",
    title_style: TitleStyle::RoleAtCompany,
    item_type: ItemType::Internship,
};

/// Same maintainers and the same file shape as [`SIMPLIFY`], but full-time
/// graduate roles rather than internships — the postings a final-year student
/// is actually applying to.
pub const NEW_GRAD: GithubInternships = GithubInternships {
    name: "New Grad Positions",
    url: "https://raw.githubusercontent.com/SimplifyJobs/New-Grad-Positions/dev/.github/scripts/listings.json",
    title_style: TitleStyle::RoleAtCompany,
    item_type: ItemType::Job,
};

/// One row of `listings.json`. Only the fields we use; the file also carries
/// `company_url`, `date_updated`, `terms` and a few others.
#[derive(Deserialize)]
struct Listing {
    #[serde(default)]
    id: Option<String>,
    #[serde(default)]
    title: Option<String>,
    #[serde(default)]
    company_name: Option<String>,
    #[serde(default)]
    url: Option<String>,
    #[serde(default)]
    locations: Vec<String>,
    #[serde(default)]
    terms: Vec<String>,
    #[serde(default)]
    degrees: Vec<String>,
    #[serde(default)]
    category: Option<String>,
    #[serde(default)]
    sponsorship: Option<String>,
    #[serde(default)]
    source: Option<String>,
    #[serde(default)]
    date_posted: i64,
    #[serde(default)]
    active: bool,
    #[serde(default)]
    is_visible: bool,
}

impl Listing {
    /// A row is usable only if it is open and actually links somewhere.
    fn is_open(&self) -> bool {
        self.active && self.is_visible && self.url.as_deref().is_some_and(|u| !u.is_empty())
    }

    fn timestamp(&self) -> Option<DateTime<Utc>> {
        Utc.timestamp_opt(self.date_posted, 0).single()
    }
}

/// Postings list every office they are open in; some name more than fifty.
/// The pipe separator is what `location::clean_location` and
/// `normalize_location` already expect.
fn join_locations(locations: &[String]) -> Option<String> {
    let joined = locations
        .iter()
        .map(|l| l.trim())
        .filter(|l| !l.is_empty())
        .collect::<Vec<_>>()
        .join(" | ");
    (!joined.is_empty()).then_some(joined)
}

#[async_trait]
impl Scraper for GithubInternships {
    fn source_name(&self) -> &'static str {
        self.name
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let listings: Vec<Listing> = client.get(self.url).send().await?.json().await?;

        let mut open: Vec<Listing> = listings.into_iter().filter(Listing::is_open).collect();
        // The file is not in date order, so the cap has to sort first or it
        // would keep an arbitrary slice rather than the freshest one.
        open.sort_by_key(|l| std::cmp::Reverse(l.date_posted));
        open.truncate(MAX_LISTINGS);

        Ok(open
            .into_iter()
            .filter_map(|listing| {
                let url = listing.url.clone()?;
                let timestamp = listing.timestamp()?;
                let company = listing.company_name.clone().unwrap_or_else(|| "Unknown".into());
                let role = listing.title.clone().unwrap_or_else(|| "Intern".into());
                let location = join_locations(&listing.locations);

                let title = match self.title_style {
                    TitleStyle::CompanyOnly => format!("Internship at {company}"),
                    TitleStyle::RoleAtCompany => format!("{role} at {company}"),
                };

                // Richer than the old table could offer, and it feeds skill
                // extraction until the enrichment pass fills in the real
                // description.
                let mut parts = Vec::new();
                if matches!(self.title_style, TitleStyle::CompanyOnly) {
                    parts.push(format!("Role: {role}"));
                }
                if let Some(loc) = location.as_deref() {
                    parts.push(format!("Location: {loc}"));
                }
                if !listing.terms.is_empty() {
                    parts.push(listing.terms.join(", "));
                }
                if let Some(category) = listing.category.as_deref().filter(|c| !c.is_empty()) {
                    parts.push(format!("Category: {category}"));
                }
                if !listing.degrees.is_empty() {
                    parts.push(format!("Degrees: {}", listing.degrees.join(", ")));
                }
                if let Some(s) = listing.sponsorship.as_deref().filter(|s| !s.is_empty()) {
                    parts.push(s.to_string());
                }

                Some(
                    Item {
                        // These repos are software-only, so skip the keyword
                        // classifier and label them directly.
                        discipline: Some("Software Engineering".to_string()),
                        // The repos state the employer outright. It used to be
                        // formatted into the title and dropped, which left the
                        // employer filter and the prestige term with nothing
                        // but a string to guess from.
                        company: (company != "Unknown").then(|| company.clone()),
                        simplify_id: listing
                            .id
                            .filter(|id| !id.is_empty())
                            .filter(|_| listing.source.as_deref() == Some(SIMPLIFY_SOURCE)),
                        ..Item::new(title, self.name, self.item_type, url)
                    }
                    .with_content(parts.join(" · "))
                    .with_timestamp(timestamp)
                    .with_location(location),
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn listing(json: &str) -> Listing {
        serde_json::from_str(json).expect("listing parses")
    }

    #[test]
    fn parses_a_real_row() {
        let l = listing(
            r#"{"date_updated":1766764298,"sponsorship":"U.S. Citizenship is Required",
                "active":true,"degrees":["Bachelor's"],"url":"https://example.com/job",
                "company_name":"AeroVironment","title":"Software Engineering Intern",
                "locations":["Germantown, MD"],"terms":["Summer 2026"],
                "category":"Software Engineering","source":"Simplify",
                "id":"3697b474-d02a-477a-949b-63c212baa9a1","date_posted":1766764298,
                "company_url":"","is_visible":true}"#,
        );
        assert!(l.is_open());
        assert_eq!(l.company_name.as_deref(), Some("AeroVironment"));
        assert_eq!(l.timestamp().unwrap().timestamp(), 1766764298);
    }

    #[test]
    fn closed_and_hidden_and_linkless_rows_are_dropped() {
        let base = r#""url":"https://e.com/j","date_posted":1,"#;
        assert!(!listing(&format!("{{{base}\"active\":false,\"is_visible\":true}}")).is_open());
        assert!(!listing(&format!("{{{base}\"active\":true,\"is_visible\":false}}")).is_open());
        assert!(!listing(
            r#"{"url":"","date_posted":1,"active":true,"is_visible":true}"#
        )
        .is_open());
    }

    #[test]
    fn unknown_fields_do_not_break_the_parse() {
        // The maintainers add columns without warning; a new one must not take
        // the whole source down.
        let l = listing(
            r#"{"url":"https://e.com/j","date_posted":1,"active":true,"is_visible":true,
                "some_brand_new_field":{"nested":[1,2,3]}}"#,
        );
        assert!(l.is_open());
    }

    #[test]
    fn locations_join_with_the_separator_the_tagger_expects() {
        assert_eq!(
            join_locations(&["Remote in USA".into(), "Toronto, ON".into()]).unwrap(),
            "Remote in USA | Toronto, ON"
        );
        assert_eq!(join_locations(&[]), None);
        assert_eq!(join_locations(&["".into(), "  ".into()]), None);
    }

    #[test]
    fn only_simplify_sourced_rows_claim_a_posting_id() {
        // Community ids 404 on simplify.jobs, so carrying one would buy a
        // guaranteed-failed request per row.
        let keep = |src: &str| {
            listing(&format!(
                r#"{{"url":"https://e.com/j","date_posted":1,"active":true,
                    "is_visible":true,"id":"abc","source":"{src}"}}"#
            ))
        };
        let simplify = keep("Simplify");
        assert_eq!(simplify.source.as_deref(), Some("Simplify"));
        let community = keep("AlmondCroffle");
        assert_ne!(community.source.as_deref(), Some(SIMPLIFY_SOURCE));
    }
}
