//! Scrapes the most recent TLDR Tech issues for their article blocks (title +
//! summary), skipping sponsor slots.
//!
//! Issues live at a dated URL, `/tech/YYYY-MM-DD`. Finding the current one used
//! to mean reading `/tech/archives` and taking the newest date listed there,
//! but that index lags the site badly — measured, it topped out at an issue two
//! weeks older than the one already published — so the app was serving
//! fortnight-old newsletter items as today's news. Dates are guessed directly
//! instead, walking back from today, with the archive kept only as a fallback
//! for the day the URL scheme changes.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Duration, NaiveDate, TimeZone, Utc};
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

use super::{collapse_ws, Scraper};
use crate::models::{Item, ItemType};

pub struct TldrTech;

const ARCHIVE: &str = "https://tldr.tech/tech/archives";
/// How far back to look for published issues. TLDR skips weekends and
/// holidays, so a week of candidates reliably contains at least three issues.
const LOOKBACK_DAYS: i64 = 7;
/// How many issues to keep. One issue is ~15 stories; three is a few days of
/// catching up without the newsletter crowding out every other news source.
const ISSUES: usize = 3;

static ISSUE_HREF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"href="/tech/(\d{4}-\d{2}-\d{2})""#).unwrap());
static ARTICLE: LazyLock<Selector> = LazyLock::new(|| Selector::parse("article").unwrap());
static HEADING: LazyLock<Selector> = LazyLock::new(|| Selector::parse("h3").unwrap());
static LINK: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").unwrap());
static SUMMARY: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.newsletter-html").unwrap());

#[async_trait]
impl Scraper for TldrTech {
    fn source_name(&self) -> &'static str {
        "TLDR Tech"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let today = Utc::now().date_naive();
        let candidates: Vec<NaiveDate> = (0..LOOKBACK_DAYS)
            .filter_map(|n| today.checked_sub_signed(Duration::days(n)))
            .collect();

        // Concurrent: a day with no issue still costs a request, and walking
        // seven of them in sequence to find three issues is the difference
        // between one round trip and seven.
        let results = futures::future::join_all(
            candidates
                .iter()
                .map(|date| self.fetch_issue(client, *date)),
        )
        .await;

        let mut items = Vec::new();
        let mut kept = 0;

        for (date, result) in candidates.iter().zip(results) {
            match result {
                // A date with no issue answers 200 with an empty page rather
                // than 404, so "did it parse to anything" is the existence test.
                Ok(issue) if issue.is_empty() => continue,
                Ok(issue) => {
                    items.extend(issue);
                    kept += 1;
                    if kept == ISSUES {
                        break;
                    }
                }
                Err(e) => log::warn!("TLDR: {date} failed: {e}"),
            }
        }

        if !items.is_empty() {
            return Ok(items);
        }

        // Nothing at any dated URL means the scheme moved. The archive index is
        // stale but it links real issues, so it still beats returning nothing.
        log::warn!(
            "TLDR: no issue found in the last {LOOKBACK_DAYS} days, falling back to archive"
        );
        let latest = self.latest_from_archive(client).await?;
        self.fetch_issue(client, latest).await
    }
}

impl TldrTech {
    /// Parses one issue. An unpublished date yields an empty vector, not an error.
    async fn fetch_issue(&self, client: &reqwest::Client, date: NaiveDate) -> Result<Vec<Item>> {
        let body = client
            .get(format!(
                "https://tldr.tech/tech/{}",
                date.format("%Y-%m-%d")
            ))
            .send()
            .await?
            .text()
            .await?;

        let published = issue_timestamp(date);
        let doc = Html::parse_document(&body);

        Ok(doc
            .select(&ARTICLE)
            .filter_map(|article| {
                let title =
                    collapse_ws(&article.select(&HEADING).next()?.text().collect::<String>());
                if title.to_lowercase().contains("(sponsor)") {
                    return None;
                }
                let href = article.select(&LINK).next()?.value().attr("href")?;

                let summary = article
                    .select(&SUMMARY)
                    .next()
                    .map(|d| collapse_ws(&d.text().collect::<String>()))
                    .unwrap_or_default();

                Some(
                    Item::new(title, self.source_name(), ItemType::Article, href)
                        .with_content(summary)
                        .with_timestamp(published),
                )
            })
            .collect())
    }

    /// Newest issue the archive index knows about. Dates are ISO, so the
    /// lexical maximum is the newest.
    async fn latest_from_archive(&self, client: &reqwest::Client) -> Result<NaiveDate> {
        let archive = client.get(ARCHIVE).send().await?.text().await?;

        ISSUE_HREF
            .captures_iter(&archive)
            .filter_map(|c| NaiveDate::parse_from_str(&c[1], "%Y-%m-%d").ok())
            .max()
            .ok_or_else(|| anyhow!("no issues found in archive"))
    }
}

/// Every story in an issue carries that issue's publication date. Midday rather
/// than midnight so a reader's local timezone can't shift it onto the day
/// before, which would make this morning's issue look a day stale.
fn issue_timestamp(date: NaiveDate) -> DateTime<Utc> {
    date.and_hms_opt(12, 0, 0)
        .map(|dt| Utc.from_utc_datetime(&dt))
        .unwrap_or_else(Utc::now)
}

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Datelike;

    #[test]
    fn an_issue_is_stamped_with_its_own_date() {
        let date = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let ts = issue_timestamp(date);
        assert_eq!((ts.year(), ts.month(), ts.day()), (2026, 8, 12));
    }

    #[test]
    fn the_lookback_window_starts_today_and_walks_back() {
        let today = NaiveDate::from_ymd_opt(2026, 8, 12).unwrap();
        let candidates: Vec<NaiveDate> = (0..LOOKBACK_DAYS)
            .filter_map(|n| today.checked_sub_signed(Duration::days(n)))
            .collect();

        // Today first, so the freshest issue is the one that fills the quota.
        assert_eq!(candidates.first(), Some(&today));
        assert_eq!(candidates.len(), LOOKBACK_DAYS as usize);
        assert_eq!(
            candidates.last(),
            Some(&NaiveDate::from_ymd_opt(2026, 8, 6).unwrap())
        );
        // A week of candidates has to be able to cover the issue quota.
        assert!(LOOKBACK_DAYS as usize > ISSUES);
    }
}
