//! Greenhouse job boards.
//!
//! The list endpoint is deliberately fetched *without* `?content=true`: the
//! descriptions inflate one board to 4 MB, and `scrapers::details` already has
//! a Greenhouse handler that fetches a single posting when the enrichment pass
//! gets to it. What the list does carry, and nothing else in the pipeline had,
//! is `application_deadline`.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::super::employers::{Board, Employer};
use super::fan_out;
use crate::models::{Item, ItemType};
use crate::scrapers::Scraper;

/// Anduril and SpaceX each publish over two thousand roles. Newest first, so
/// the cap keeps the part a reader would ever reach.
const MAX_PER_EMPLOYER: usize = 400;

pub struct GreenhouseBoards;

#[derive(Deserialize)]
struct BoardResponse {
    #[serde(default)]
    jobs: Vec<Job>,
}

#[derive(Deserialize)]
struct Job {
    #[serde(default)]
    title: String,
    #[serde(default)]
    absolute_url: String,
    #[serde(default)]
    location: Option<Location>,
    #[serde(default)]
    updated_at: Option<String>,
    #[serde(default)]
    first_published: Option<String>,
    /// Present and usually null, but the campus and new-grad postings that do
    /// set it are exactly the ones with a hard cut-off.
    #[serde(default)]
    application_deadline: Option<String>,
}

#[derive(Deserialize)]
struct Location {
    #[serde(default)]
    name: String,
}

#[async_trait]
impl Scraper for GreenhouseBoards {
    fn source_name(&self) -> &'static str {
        "Greenhouse"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        Ok(fan_out(
            client,
            |b| matches!(b, Board::Greenhouse(_)),
            |e, c| Box::pin(fetch_board(e, c)),
        )
        .await)
    }
}

async fn fetch_board(employer: &'static Employer, client: reqwest::Client) -> Result<Vec<Item>> {
    let Board::Greenhouse(slug) = employer.board else {
        return Ok(Vec::new());
    };
    let url = format!("https://boards-api.greenhouse.io/v1/boards/{slug}/jobs");
    let board: BoardResponse = client
        .get(&url)
        .send()
        .await?
        .error_for_status()?
        .json()
        .await?;

    let mut jobs = board.jobs;
    // The API returns no useful order, so sort before capping rather than
    // truncating an arbitrary slice. Cached key, not `sort_by`: `posted`
    // parses a date, and a comparator would re-parse both sides on every
    // comparison instead of once per job.
    jobs.sort_by_cached_key(|job| std::cmp::Reverse(posted(job)));
    jobs.truncate(MAX_PER_EMPLOYER);

    Ok(jobs
        .into_iter()
        .filter(|j| !j.absolute_url.is_empty() && !j.title.trim().is_empty())
        .map(|job| to_item(employer.name, job))
        .collect())
}

/// Greenhouse stamps `updated_at` on edits, so `first_published` is the real
/// posted date wherever it exists. Falling back to `updated_at` beats falling
/// back to `now`, which would make an edited two-year-old req look fresh.
fn posted(job: &Job) -> DateTime<Utc> {
    job.first_published
        .as_deref()
        .or(job.updated_at.as_deref())
        .and_then(parse_date)
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30))
}

fn parse_date(raw: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(raw)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

fn to_item(company: &str, job: Job) -> Item {
    let timestamp = posted(&job);
    let location = job.location.as_ref().map(|l| l.name.clone());
    let closes_at = job.application_deadline.as_deref().and_then(parse_date);
    let content = match &location {
        Some(l) if !l.trim().is_empty() => format!("{company} · {l}"),
        _ => company.to_string(),
    };
    Item::new(
        job.title.trim(),
        "Greenhouse",
        ItemType::Job,
        job.absolute_url.clone(),
    )
    .with_company(company)
    .with_location(location)
    .with_timestamp(timestamp)
    .with_closes_at(closes_at)
    .with_content(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(first: Option<&str>, updated: Option<&str>, deadline: Option<&str>) -> Job {
        Job {
            title: "Engineer".into(),
            absolute_url: "https://example.com/1".into(),
            location: Some(Location {
                name: "Toronto".into(),
            }),
            updated_at: updated.map(str::to_string),
            first_published: first.map(str::to_string),
            application_deadline: deadline.map(str::to_string),
        }
    }

    #[test]
    fn prefers_first_published_over_updated_at() {
        let j = job(
            Some("2026-01-05T00:00:00Z"),
            Some("2026-08-20T00:00:00Z"),
            None,
        );
        assert_eq!(posted(&j), parse_date("2026-01-05T00:00:00Z").unwrap());
    }

    #[test]
    fn carries_the_application_deadline() {
        let item = to_item(
            "Stripe",
            job(
                Some("2026-08-01T00:00:00Z"),
                None,
                Some("2026-09-30T00:00:00Z"),
            ),
        );
        assert_eq!(item.closes_at, parse_date("2026-09-30T00:00:00Z"));
        assert_eq!(item.company.as_deref(), Some("Stripe"));
    }

    #[test]
    fn undated_rows_do_not_read_as_fresh() {
        assert!(posted(&job(None, None, None)) < Utc::now() - chrono::Duration::days(29));
    }
}
