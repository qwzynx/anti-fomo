//! SmartRecruiters job boards.
//!
//! The list endpoint carries no description — `scrapers::details` already has
//! a SmartRecruiters handler, and that one labels its own sections, which
//! beats any heading guess — so this is a listing scraper only.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::super::employers::{Board, Employer};
use super::fan_out;
use crate::models::{Item, ItemType};
use crate::scrapers::Scraper;

/// The API's own page cap.
const PAGE: usize = 100;
const MAX_PER_EMPLOYER: usize = 300;

pub struct SmartRecruitersBoards;

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    content: Vec<Posting>,
}

#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
struct Posting {
    #[serde(default)]
    id: String,
    #[serde(default)]
    name: String,
    #[serde(default)]
    location: Option<Location>,
    #[serde(default)]
    released_date: Option<String>,
}

#[derive(Deserialize)]
struct Location {
    #[serde(default)]
    city: Option<String>,
    #[serde(default)]
    region: Option<String>,
    #[serde(default)]
    country: Option<String>,
    #[serde(default)]
    remote: Option<bool>,
}

impl Location {
    fn label(&self) -> String {
        let mut parts: Vec<&str> = Vec::new();
        if self.remote.unwrap_or(false) {
            parts.push("Remote");
        }
        for part in [&self.city, &self.region, &self.country] {
            if let Some(p) = part.as_deref().filter(|p| !p.trim().is_empty()) {
                parts.push(p);
            }
        }
        parts.join(", ")
    }
}

#[async_trait]
impl Scraper for SmartRecruitersBoards {
    fn source_name(&self) -> &'static str {
        "SmartRecruiters"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        Ok(fan_out(
            client,
            |b| matches!(b, Board::SmartRecruiters(_)),
            |e, c| Box::pin(fetch_board(e, c)),
        )
        .await)
    }
}

async fn fetch_board(employer: &'static Employer, client: reqwest::Client) -> Result<Vec<Item>> {
    let Board::SmartRecruiters(slug) = employer.board else {
        return Ok(Vec::new());
    };
    let mut items = Vec::new();
    let mut offset = 0;

    while items.len() < MAX_PER_EMPLOYER {
        let url = format!(
            "https://api.smartrecruiters.com/v1/companies/{slug}/postings?limit={PAGE}&offset={offset}"
        );
        // A failing page after the first ends the walk with what it has; a
        // failing first page is a real error, same rule as Job Bank.
        let page: Page = match client
            .get(&url)
            .send()
            .await
            .and_then(|r| r.error_for_status())
        {
            Ok(response) => response.json().await?,
            Err(e) if offset > 0 => {
                log::debug!("{} page at offset {offset}: {e}", employer.name);
                break;
            }
            Err(e) => return Err(e.into()),
        };
        let short = page.content.len() < PAGE;
        for posting in page.content {
            if posting.id.is_empty() || posting.name.trim().is_empty() {
                continue;
            }
            items.push(to_item(employer.name, slug, posting));
        }
        if short {
            break;
        }
        offset += PAGE;
    }
    items.truncate(MAX_PER_EMPLOYER);
    Ok(items)
}

fn to_item(company: &str, slug: &str, posting: Posting) -> Item {
    let url = format!("https://jobs.smartrecruiters.com/{slug}/{}", posting.id);
    let location = posting
        .location
        .as_ref()
        .map(Location::label)
        .filter(|l| !l.is_empty());
    let timestamp = posting
        .released_date
        .as_deref()
        .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));

    let content = match &location {
        Some(l) => format!("{company} · {l}"),
        None => company.to_string(),
    };
    Item::new(posting.name.trim(), "SmartRecruiters", ItemType::Job, url)
        .with_company(company)
        .with_location(location)
        .with_timestamp(timestamp)
        .with_content(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn builds_a_url_the_detail_handler_can_route() {
        let item = to_item(
            "Ubisoft",
            "Ubisoft2",
            Posting {
                id: "744000012345678".into(),
                name: "Gameplay Programmer Intern".into(),
                location: Some(Location {
                    city: Some("Toronto".into()),
                    region: Some("Ontario".into()),
                    country: Some("Canada".into()),
                    remote: Some(false),
                }),
                released_date: Some("2026-08-01T00:00:00.000Z".into()),
            },
        );
        assert_eq!(item.location.as_deref(), Some("Toronto, Ontario, Canada"));
        assert!(!crate::scrapers::details::route(&item.url, None).is_empty());
    }

    #[test]
    fn a_remote_posting_says_so_in_its_location() {
        let location = Location {
            city: None,
            region: None,
            country: Some("Canada".into()),
            remote: Some(true),
        };
        assert_eq!(location.label(), "Remote, Canada");
    }
}
