//! Workday tenants, through the same `cxs` API the detail handler already uses.
//!
//! This is the source that was missing. Workday is what CIBC, RBC, TD, BMO and
//! most large Canadian employers run, and none of them syndicate campus roles
//! to the GitHub repos — so a co-op posting could sit on cibc.wd3 for a week
//! and never reach the feed. `scrapers::details` could already *read* a
//! Workday posting; nothing could *find* one.
//!
//! The posting URL is built in the shape `details::workday` parses, so every
//! item this produces routes into the existing handler for free, and picks up
//! its `endDate` — which is where the application deadline comes from.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Duration, Utc};
use serde::Deserialize;
use std::collections::HashSet;

use super::super::employers::{Board, Employer};
use super::fan_out;
use crate::models::{Item, ItemType};
use crate::scrapers::Scraper;

/// Workday caps a page at 20 regardless of what `limit` asks for.
const PAGE: usize = 20;

/// A board at or under this many postings is walked in full. Above it, walking
/// is wasteful — PwC is 4,338 postings and CIBC 543, almost none of it
/// engineering — so we sweep by keyword instead and let ranking sort the rest.
const SMALL_BOARD: usize = 220;

/// Pages fetched per keyword on a large board. Workday orders by relevance, so
/// the third page of "intern" is already off-topic.
const PAGES_PER_TERM: usize = 2;

/// Ceiling per employer, whichever path was taken. A single tenant should not
/// be able to contribute two thousand rows to a cache the whole app has to
/// rank in memory.
const MAX_PER_EMPLOYER: usize = 260;

/// What we ask a large board for. Workday's search is fuzzy and ORs the words,
/// so these are nets rather than filters — "intern" also returns "Internal
/// Event Specialist". The local classifier and the seniority term sort that
/// out; the point here is to reach the roles at all.
const TERMS: &[&str] = &[
    "software engineer",
    "developer",
    "intern",
    "co-op student",
    "new graduate",
    "data",
    "technology analyst",
    "security",
];

pub struct WorkdayBoards;

#[derive(Deserialize)]
struct SearchResponse {
    #[serde(default)]
    total: usize,
    #[serde(default, rename = "jobPostings")]
    job_postings: Vec<Posting>,
}

#[derive(Deserialize)]
struct Posting {
    #[serde(default)]
    title: String,
    #[serde(default, rename = "externalPath")]
    external_path: String,
    #[serde(default, rename = "locationsText")]
    locations_text: String,
    #[serde(default, rename = "postedOn")]
    posted_on: String,
}

#[async_trait]
impl Scraper for WorkdayBoards {
    fn source_name(&self) -> &'static str {
        "Workday"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        Ok(fan_out(
            client,
            |b| matches!(b, Board::Workday { .. }),
            |e, c| Box::pin(fetch_board(e, c)),
        )
        .await)
    }
}

async fn fetch_board(employer: &'static Employer, client: reqwest::Client) -> Result<Vec<Item>> {
    let Board::Workday { host, tenant, site } = employer.board else {
        return Ok(Vec::new());
    };
    let endpoint =
        format!("https://{tenant}.{host}.myworkdayjobs.com/wday/cxs/{tenant}/{site}/jobs");
    let base = format!("https://{tenant}.{host}.myworkdayjobs.com/{site}");

    // The first request doubles as the size probe: it tells us `total` and
    // hands back page one either way, so learning which strategy to use costs
    // nothing.
    let first = search(&client, &endpoint, "", 0).await?;
    let mut seen: HashSet<String> = HashSet::new();
    let mut items: Vec<Item> = Vec::new();

    let take = |postings: Vec<Posting>, items: &mut Vec<Item>, seen: &mut HashSet<String>| {
        for posting in postings {
            if posting.external_path.is_empty() || posting.title.trim().is_empty() {
                continue;
            }
            if !seen.insert(posting.external_path.clone()) {
                continue;
            }
            items.push(to_item(employer.name, &base, posting));
        }
    };

    if first.total <= SMALL_BOARD {
        // A small board is entirely worth having — RBC's early-talent site is
        // 57 postings and every one of them is a co-op or analyst programme.
        take(first.job_postings, &mut items, &mut seen);
        let mut offset = PAGE;
        while offset < first.total && items.len() < MAX_PER_EMPLOYER {
            let page = search(&client, &endpoint, "", offset).await?;
            if page.job_postings.is_empty() {
                break;
            }
            let short = page.job_postings.len() < PAGE;
            take(page.job_postings, &mut items, &mut seen);
            if short {
                break;
            }
            offset += PAGE;
        }
        return Ok(items);
    }

    for term in TERMS {
        for page_index in 0..PAGES_PER_TERM {
            if items.len() >= MAX_PER_EMPLOYER {
                return Ok(items);
            }
            // A term that fails after the first one ends that term's walk with
            // what it has, rather than the whole employer's.
            let page = match search(&client, &endpoint, term, page_index * PAGE).await {
                Ok(p) => p,
                Err(e) if page_index > 0 => {
                    log::debug!("{} page {page_index} of {term:?}: {e}", employer.name);
                    break;
                }
                Err(e) => return Err(e),
            };
            let short = page.job_postings.len() < PAGE;
            take(page.job_postings, &mut items, &mut seen);
            if short {
                break;
            }
        }
    }
    Ok(items)
}

async fn search(
    client: &reqwest::Client,
    endpoint: &str,
    search_text: &str,
    offset: usize,
) -> Result<SearchResponse> {
    let body = serde_json::json!({
        "appliedFacets": {},
        "limit": PAGE,
        "offset": offset,
        "searchText": search_text,
    });
    let response = client
        .post(endpoint)
        .header("Content-Type", "application/json")
        // Without this a Canadian tenant answers in French, and every title,
        // location and "Posted N Days Ago" arrives unparseable.
        .header("Accept-Language", "en-US")
        .json(&body)
        .send()
        .await?
        .error_for_status()?;
    Ok(response.json::<SearchResponse>().await?)
}

fn to_item(company: &str, base: &str, posting: Posting) -> Item {
    let url = format!("{base}{}", posting.external_path);
    let location =
        (!posting.locations_text.trim().is_empty()).then(|| posting.locations_text.clone());
    Item::new(posting.title.trim(), "Workday", ItemType::Job, url)
        .with_company(company)
        .with_location(location)
        .with_timestamp(posted_on(&posting.posted_on, Utc::now()))
        .with_content(format!("{} · {}", company, posting.locations_text))
}

/// Turns Workday's relative "Posted 2 Days Ago" into a timestamp.
///
/// "30+ Days Ago" is genuinely unknown age, and the honest floor is 30 days —
/// stamping `now` on it would hand every dormant posting on a 4,000-row bank
/// board the full recency bonus, which is exactly the failure `CLAUDE.md`
/// calls out. An unrecognised string gets the same treatment for the same
/// reason.
fn posted_on(raw: &str, now: DateTime<Utc>) -> DateTime<Utc> {
    let text = raw.to_lowercase();
    if text.contains("today") || text.contains("just posted") {
        return now;
    }
    if text.contains("yesterday") {
        return now - Duration::days(1);
    }
    let days: Option<i64> = text
        .split_whitespace()
        .find_map(|word| word.trim_end_matches('+').parse::<i64>().ok());
    match days {
        Some(n) if text.contains("hour") => now - Duration::hours(n),
        Some(n) if text.contains("month") => now - Duration::days(n * 30),
        Some(n) => now - Duration::days(n),
        None => now - Duration::days(30),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn at(year: i32, month: u32, day: u32) -> DateTime<Utc> {
        chrono::TimeZone::with_ymd_and_hms(&Utc, year, month, day, 12, 0, 0).unwrap()
    }

    #[test]
    fn reads_relative_posted_dates() {
        let now = at(2026, 8, 23);
        assert_eq!(posted_on("Posted Today", now), now);
        assert_eq!(posted_on("Posted Yesterday", now), now - Duration::days(1));
        assert_eq!(posted_on("Posted 2 Days Ago", now), now - Duration::days(2));
        assert_eq!(
            posted_on("Posted 5 Hours Ago", now),
            now - Duration::hours(5)
        );
    }

    #[test]
    fn unknown_age_floors_at_thirty_days_rather_than_now() {
        // The whole point: an undated row must never out-rank a fresh one.
        let now = at(2026, 8, 23);
        assert_eq!(
            posted_on("Posted 30+ Days Ago", now),
            now - Duration::days(30)
        );
        assert_eq!(posted_on("", now), now - Duration::days(30));
        assert_eq!(
            posted_on("Publié il y a 3 jours", now),
            now - Duration::days(3)
        );
    }

    #[test]
    fn builds_a_url_the_detail_handler_can_route() {
        let item = to_item(
            "CIBC",
            "https://cibc.wd3.myworkdayjobs.com/search",
            Posting {
                title: "Software Developer Co-op".into(),
                external_path: "/job/Toronto-ON/Software-Developer-Co-op_2613909".into(),
                locations_text: "Toronto, ON".into(),
                posted_on: "Posted 2 Days Ago".into(),
            },
        );
        assert_eq!(
            item.url,
            "https://cibc.wd3.myworkdayjobs.com/search/job/Toronto-ON/Software-Developer-Co-op_2613909"
        );
        assert_eq!(item.company.as_deref(), Some("CIBC"));
        // The chain in `scrapers::details` must claim it, or the deadline and
        // the description never arrive.
        let chain = crate::scrapers::details::route(&item.url, None);
        assert!(!chain.is_empty(), "workday url did not route");
    }
}
