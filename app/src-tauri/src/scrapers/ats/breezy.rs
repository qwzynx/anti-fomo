//! Breezy HR career pages.
//!
//! `https://{slug}.breezy.hr/json` lists the open positions: title, type,
//! location (with a structured country and state), a salary *string* and the
//! publish date. No description — the posting page carries a schema.org
//! `JobPosting`, which `scrapers::details` reads for the ones that rank well.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::super::employers::{Board, Employer};
use super::{fan_out, keep_best};
use crate::models::{Item, ItemType};
use crate::scrapers::Scraper;

const MAX_PER_EMPLOYER: usize = 300;

pub struct BreezyBoards;

/// Breezy's shapes vary by tenant — a `location` that is an object on most
/// boards is a bare string on some and null on others, and `type` likewise —
/// so the fields that vary are read as loose JSON and picked apart by hand
/// rather than failing the whole board over one of them.
#[derive(Deserialize)]
struct Position {
    #[serde(default)]
    name: String,
    #[serde(default)]
    url: String,
    #[serde(default)]
    published_date: Option<String>,
    #[serde(default, rename = "type")]
    kind: serde_json::Value,
    #[serde(default)]
    location: serde_json::Value,
    #[serde(default)]
    salary: Option<String>,
}

/// `{"id": "contract", "name": "Contract"}` → ("contract", "Contract").
fn named(value: &serde_json::Value) -> (String, String) {
    let text = |key: &str| {
        value
            .get(key)
            .and_then(|v| v.as_str())
            .unwrap_or_default()
            .to_string()
    };
    match value {
        serde_json::Value::String(s) => (s.clone(), s.clone()),
        _ => (text("id"), text("name")),
    }
}

struct Place {
    name: String,
    is_remote: bool,
    state: String,
    country: String,
}

fn place(value: &serde_json::Value) -> Option<Place> {
    match value {
        serde_json::Value::String(s) if !s.trim().is_empty() => Some(Place {
            name: s.clone(),
            is_remote: false,
            state: String::new(),
            country: String::new(),
        }),
        serde_json::Value::Object(_) => Some(Place {
            name: value
                .get("name")
                .and_then(|v| v.as_str())
                .unwrap_or_default()
                .to_string(),
            is_remote: value
                .get("is_remote")
                .and_then(|v| v.as_bool())
                .unwrap_or(false),
            state: named(value.get("state").unwrap_or(&serde_json::Value::Null)).1,
            country: named(value.get("country").unwrap_or(&serde_json::Value::Null)).1,
        }),
        _ => None,
    }
}

#[async_trait]
impl Scraper for BreezyBoards {
    fn source_name(&self) -> &'static str {
        "Breezy"
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
            |b| matches!(b, Board::Breezy(_)),
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
    let Board::Breezy(slug) = employer.board else {
        return Ok(Vec::new());
    };
    let url = format!("https://{slug}.breezy.hr/json");
    let positions: Vec<Position> = super::get_json(&client, &url).await?;
    let items = positions
        .into_iter()
        .filter(|p| !p.url.is_empty() && !p.name.trim().is_empty())
        .map(|p| to_item(employer.name, p))
        .collect();
    Ok(keep_best(items, MAX_PER_EMPLOYER))
}

/// "Toronto, Ontario, Canada" from the structured place, falling back to its
/// own display name.
fn place_of(place: &Place) -> Option<String> {
    let mut seen: Vec<&str> = Vec::new();
    for part in [&place.name, &place.state, &place.country] {
        let part = part.trim();
        if !part.is_empty() && !seen.iter().any(|s| s.contains(part) || part.contains(s)) {
            seen.push(part);
        }
    }
    (!seen.is_empty()).then(|| seen.join(", "))
}

fn to_item(company: &str, position: Position) -> Item {
    let location = place(&position.location).and_then(|p| {
        place_of(&p).map(|l| {
            if p.is_remote {
                format!("{l} | Remote")
            } else {
                l
            }
        })
    });
    let timestamp = position
        .published_date
        .as_deref()
        .and_then(|d| DateTime::parse_from_rfc3339(d).ok())
        .map(|d| d.with_timezone(&Utc))
        .unwrap_or_else(|| Utc::now() - chrono::Duration::days(30));
    let (kind_id, kind_name) = named(&position.kind);
    let intern = kind_id.to_ascii_lowercase().contains("intern")
        || kind_name.to_ascii_lowercase().contains("intern");
    let mut content = company.to_string();
    if let Some(l) = &location {
        content = format!("{content} · {l}");
    }
    if !kind_name.is_empty() {
        content = format!("{content} · {kind_name}");
    }
    if let Some(s) = position.salary.as_deref().filter(|s| !s.trim().is_empty()) {
        content = format!("{content} · {s}");
    }

    Item::new(
        position.name.trim(),
        "Breezy",
        if intern {
            ItemType::Internship
        } else {
            ItemType::Job
        },
        position.url,
    )
    .with_company(company)
    .with_location(location)
    .with_timestamp(timestamp)
    .with_content(content)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_breezy_position_reads_its_place_date_and_remote_flag() {
        let json = r#"[{
            "name": "Data Associate (Winter/Spring 2027)",
            "url": "https://lumerate.breezy.hr/p/baf-data-associate",
            "published_date": "2026-11-13T13:00:55.204Z",
            "type": {"id": "contract", "name": "Contract"},
            "location": {"name": "Canada", "is_remote": true,
                         "country": {"name": "Canada", "id": "CA"},
                         "state": {"name": "Ontario", "id": "ON"}},
            "salary": "$3,500 / month"
        }]"#;
        let positions: Vec<Position> = serde_json::from_str(json).unwrap();
        let item = to_item("Lumerate", positions.into_iter().next().unwrap());
        assert_eq!(item.location.as_deref(), Some("Canada, Ontario | Remote"));
        assert_eq!(item.timestamp.format("%Y-%m-%d").to_string(), "2026-11-13");
        assert!(item.content_text.contains("$3,500 / month"));
        assert_eq!(item.item_type, ItemType::Job);
    }

    #[test]
    fn a_tenant_with_a_string_location_or_nulls_still_parses() {
        let json = r#"[
            {"name": "A", "url": "https://x.breezy.hr/p/a", "location": "Metro Manila", "type": null},
            {"name": "B", "url": "https://x.breezy.hr/p/b", "location": null,
             "type": {"id": "internship", "name": "Internship"}}
        ]"#;
        let positions: Vec<Position> = serde_json::from_str(json).unwrap();
        let items: Vec<Item> = positions.into_iter().map(|p| to_item("X", p)).collect();
        assert_eq!(items[0].location.as_deref(), Some("Metro Manila"));
        assert_eq!(items[1].item_type, ItemType::Internship);
    }
}
