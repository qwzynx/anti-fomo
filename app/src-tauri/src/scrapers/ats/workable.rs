//! Workable account boards.
//!
//! `https://apply.workable.com/api/v1/widget/accounts/{slug}` is the feed
//! Workable's own careers widget reads: public, no key, every open job. It
//! carries no description, deadline or pay, which is what the enrichment pass
//! is for — `scrapers::details` already has a Workable handler, but it needs
//! the *account* in the URL (`/{slug}/j/{shortcode}`), and the widget's own
//! `url` leaves it out. So the link is rebuilt rather than copied.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, NaiveDate, TimeZone, Utc};
use serde::Deserialize;

use super::super::employers::{Board, Employer};
use super::{fan_out, keep_best};
use crate::models::{Item, ItemType};
use crate::scrapers::Scraper;

const MAX_PER_EMPLOYER: usize = 400;

pub struct WorkableBoards;

#[derive(Deserialize)]
struct Widget {
    #[serde(default)]
    jobs: Vec<Job>,
}

#[derive(Deserialize)]
struct Job {
    #[serde(default)]
    title: String,
    #[serde(default)]
    shortcode: String,
    #[serde(default)]
    employment_type: Option<String>,
    #[serde(default)]
    telecommuting: bool,
    #[serde(default)]
    published_on: Option<String>,
    #[serde(default)]
    created_at: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    state: Option<String>,
}

#[async_trait]
impl Scraper for WorkableBoards {
    fn source_name(&self) -> &'static str {
        "Workable"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        crate::scrapers::collect_batches(self, client).await
    }

    async fn fetch_batches(
        &self,
        client: &reqwest::Client,
        sink: crate::scrapers::Sink<'_>,
    ) -> Result<()> {
        fan_out(
            client,
            |b| matches!(b, Board::Workable(_)),
            |e, c| Box::pin(fetch_board(e, c)),
            super::CONCURRENCY,
            sink,
        )
        .await;
        Ok(())
    }
}

pub async fn fetch_board(
    employer: &'static Employer,
    client: reqwest::Client,
) -> Result<Vec<Item>> {
    let Board::Workable(slug) = employer.board else {
        return Ok(Vec::new());
    };
    let url = format!("https://apply.workable.com/api/v1/widget/accounts/{slug}");
    let widget: Widget = super::get_json(&client, &url).await?;
    let items = widget
        .jobs
        .into_iter()
        .filter(|j| !j.shortcode.is_empty() && !j.title.trim().is_empty())
        .map(|job| to_item(employer.name, slug, job))
        .collect();
    Ok(keep_best(items, MAX_PER_EMPLOYER))
}

/// Workable dates are `2026-05-29`; a few accounts send a full timestamp.
fn parse_date(raw: &str) -> Option<DateTime<Utc>> {
    if let Ok(full) = DateTime::parse_from_rfc3339(raw) {
        return Some(full.with_timezone(&Utc));
    }
    let day = NaiveDate::parse_from_str(raw.get(..10)?, "%Y-%m-%d").ok()?;
    Utc.from_local_datetime(&day.and_hms_opt(12, 0, 0)?)
        .single()
}

fn to_item(company: &str, slug: &str, job: Job) -> Item {
    let place = [job.city, job.state, job.country]
        .into_iter()
        .flatten()
        .filter(|p| !p.trim().is_empty())
        .collect::<Vec<_>>()
        .join(", ");
    let location = match (place.is_empty(), job.telecommuting) {
        (true, true) => Some("Remote".to_string()),
        (true, false) => None,
        (false, true) => Some(format!("{place} | Remote")),
        (false, false) => Some(place),
    };
    let timestamp = job
        .published_on
        .as_deref()
        .or(job.created_at.as_deref())
        .and_then(parse_date)
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));
    let content = match (&location, &job.employment_type) {
        (Some(l), Some(t)) if !t.is_empty() => format!("{company} · {l} · {t}"),
        (Some(l), _) => format!("{company} · {l}"),
        _ => company.to_string(),
    };
    let intern = job
        .employment_type
        .as_deref()
        .is_some_and(|t| t.to_ascii_lowercase().contains("intern"));

    Item::new(
        job.title.trim(),
        "Workable",
        if intern {
            ItemType::Internship
        } else {
            ItemType::Job
        },
        format!("https://apply.workable.com/{slug}/j/{}/", job.shortcode),
    )
    .with_company(company)
    .with_location(location)
    .with_timestamp(timestamp)
    .with_content(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn job(shortcode: &str) -> Job {
        Job {
            title: "Software Engineer".into(),
            shortcode: shortcode.into(),
            employment_type: Some("Full-time".into()),
            telecommuting: false,
            published_on: Some("2026-05-29".into()),
            created_at: None,
            country: Some("France".into()),
            city: Some("Paris".into()),
            state: None,
        }
    }

    #[test]
    fn the_link_names_the_account_so_the_detail_handler_can_route_it() {
        let item = to_item("Acme", "acme", job("81B46579FE"));
        assert_eq!(item.url, "https://apply.workable.com/acme/j/81B46579FE/");
        assert!(
            crate::scrapers::details::route(&item.url, None)
                .iter()
                .any(|_| true),
            "no detail handler for the rebuilt link"
        );
    }

    #[test]
    fn remote_and_place_both_survive_and_the_date_is_read() {
        let mut j = job("X");
        j.telecommuting = true;
        let item = to_item("Acme", "acme", j);
        assert_eq!(item.location.as_deref(), Some("Paris, France | Remote"));
        assert_eq!(item.timestamp.format("%Y-%m-%d").to_string(), "2026-05-29");
    }

    #[test]
    fn an_internship_type_is_an_internship() {
        let mut j = job("X");
        j.employment_type = Some("Internship".into());
        assert_eq!(to_item("Acme", "acme", j).item_type, ItemType::Internship);
    }
}
