//! Hackathons from Devpost's public JSON API — the same endpoint their own
//! listing page calls, so no HTML parsing and no browser. Events were the
//! thinnest category in the feed; this is the source that fills them out.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::Deserialize;

use super::Scraper;
use crate::models::{Item, ItemType};

pub struct Devpost;

/// `status[]=open` drops the hundreds of finished hackathons in the archive.
const ENDPOINT: &str = "https://devpost.com/api/hackathons?status[]=open&per_page=40";
/// `per_page` is clamped server-side at 40 however large a value is sent, so
/// reaching the rest of the open hackathons means paging. Three passes covers
/// the ~66 that are typically open with room to spare.
const MAX_PAGES: usize = 3;
/// What the server actually returns per page; a short page means the last one.
const PAGE_SIZE: usize = 40;

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    hackathons: Vec<Hackathon>,
}

#[derive(Deserialize)]
struct Hackathon {
    title: Option<String>,
    url: Option<String>,
    displayed_location: Option<Location>,
    submission_period_dates: Option<String>,
    organization_name: Option<String>,
    prize_amount: Option<String>,
    registrations_count: Option<u64>,
    #[serde(default)]
    themes: Vec<Theme>,
}

#[derive(Deserialize)]
struct Location {
    location: Option<String>,
}

#[derive(Deserialize)]
struct Theme {
    name: Option<String>,
}

/// Devpost renders prize amounts as HTML (`$<span data-currency-value>5,000</span>`).
fn strip_prize(raw: &str) -> String {
    super::strip_html(raw).replace(" ", "")
}

/// Parses the closing date out of Devpost's display range.
///
/// Two shapes occur: `"May 19 - Aug 17, 2026"` spells out both months, while
/// `"Aug 04 - 31, 2026"` omits the repeated month on the right. The deadline is
/// what matters for ranking — the recency term measures distance from now, so a
/// hackathon closing this week outranks one closing in three months.
fn parse_deadline(range: &str) -> Option<DateTime<Utc>> {
    let (left, right) = range.split_once(" - ")?;
    let right = right.trim();

    // "Aug 17, 2026" — a full date on the right.
    let date = NaiveDate::parse_from_str(right, "%b %d, %Y")
        .or_else(|_| NaiveDate::parse_from_str(right, "%B %d, %Y"))
        .ok()
        .or_else(|| {
            // "31, 2026" — borrow the month from the left half of the range.
            let (day, year) = right.split_once(", ")?;
            let month = left.split_whitespace().next()?;
            NaiveDate::parse_from_str(&format!("{month} {day}, {year}"), "%b %d, %Y")
                .or_else(|_| NaiveDate::parse_from_str(&format!("{month} {day}, {year}"), "%B %d, %Y"))
                .ok()
        })?;

    Utc.from_utc_datetime(&date.and_hms_opt(23, 59, 0)?).into()
}

#[async_trait]
impl Scraper for Devpost {
    fn source_name(&self) -> &'static str {
        "Devpost"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let mut all = Vec::new();

        for page in 1..=MAX_PAGES {
            let hackathons = match self.fetch_page(client, page).await {
                Ok(h) => h,
                // Page 1 failing is a dead source; a later page failing just
                // ends the walk with what we already have.
                Err(e) if page == 1 => return Err(e),
                Err(e) => {
                    log::warn!("Devpost: page {page} failed: {e}");
                    break;
                }
            };
            let short_page = hackathons.len() < PAGE_SIZE;
            all.extend(hackathons);
            if short_page {
                break;
            }
        }

        Ok(all)
    }
}

impl Devpost {
    async fn fetch_page(&self, client: &reqwest::Client, page: usize) -> Result<Vec<Item>> {
        let resp: Response = client
            .get(format!("{ENDPOINT}&page={page}"))
            .send()
            .await?
            .json()
            .await?;

        Ok(resp
            .hackathons
            .into_iter()
            .filter_map(|h| {
                let title = h.title.filter(|t| !t.is_empty())?;
                let url = h.url.filter(|u| !u.is_empty())?;

                let where_ = h
                    .displayed_location
                    .and_then(|l| l.location)
                    .unwrap_or_else(|| "Online".to_string());

                // Everything a student decides on before clicking: when it
                // closes, what it's worth, how crowded it is, what it's about.
                let mut parts = Vec::new();
                if let Some(org) = h.organization_name.filter(|o| !o.is_empty()) {
                    parts.push(format!("Hosted by {org}"));
                }
                if let Some(dates) = h.submission_period_dates.as_deref() {
                    parts.push(dates.to_string());
                }
                if let Some(prize) = h.prize_amount.as_deref().map(strip_prize) {
                    if !prize.is_empty() && prize != "$0" {
                        parts.push(format!("{prize} in prizes"));
                    }
                }
                if let Some(n) = h.registrations_count.filter(|n| *n > 0) {
                    parts.push(format!("{n} registered"));
                }
                let themes: Vec<String> = h.themes.into_iter().filter_map(|t| t.name).collect();
                if !themes.is_empty() {
                    parts.push(themes.join(", "));
                }

                let ts = h
                    .submission_period_dates
                    .as_deref()
                    .and_then(parse_deadline)
                    .unwrap_or_else(Utc::now);

                Some(
                    Item::new(title, self.source_name(), ItemType::Event, url)
                        .with_content(parts.join(" · "))
                        .with_timestamp(ts)
                        .with_location(Some(where_)),
                )
            })
            .collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn parses_a_range_with_two_months() {
        let d = parse_deadline("May 19 - Aug 17, 2026").unwrap();
        assert_eq!((d.year(), d.month(), d.day()), (2026, 8, 17));
    }

    #[test]
    fn parses_a_range_that_omits_the_repeated_month() {
        let d = parse_deadline("Aug 04 - 31, 2026").unwrap();
        assert_eq!((d.year(), d.month(), d.day()), (2026, 8, 31));
    }

    #[test]
    fn parses_a_range_spanning_the_new_year() {
        let d = parse_deadline("Dec 01 - Jan 15, 2027").unwrap();
        assert_eq!((d.year(), d.month(), d.day()), (2027, 1, 15));
    }

    #[test]
    fn unparseable_ranges_yield_none_rather_than_a_wrong_date() {
        assert!(parse_deadline("").is_none());
        assert!(parse_deadline("Rolling deadline").is_none());
        assert!(parse_deadline("Aug 04 - tomorrow").is_none());
    }

    #[test]
    fn prize_html_becomes_a_plain_amount() {
        assert_eq!(
            strip_prize("$<span data-currency-value>2,000,000</span>"),
            "$2,000,000"
        );
    }
}
