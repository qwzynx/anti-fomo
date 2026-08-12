//! lu.ma city pages are Next.js and ship the upcoming-events list inside the
//! `__NEXT_DATA__` script tag, so no headless browser is needed.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use regex::Regex;
use std::sync::LazyLock;

use super::Scraper;
use crate::models::{Item, ItemType};

pub struct Luma;

const CITY_PAGE: &str = "https://lu.ma/toronto";
const LIMIT: usize = 20;

static NEXT_DATA: LazyLock<Regex> = LazyLock::new(|| {
    Regex::new(r#"(?s)<script id="__NEXT_DATA__" type="application/json">(.*?)</script>"#).unwrap()
});

#[async_trait]
impl Scraper for Luma {
    fn source_name(&self) -> &'static str {
        "Luma"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let body = client.get(CITY_PAGE).send().await?.text().await?;

        let blob = NEXT_DATA
            .captures(&body)
            .map(|c| c[1].to_string())
            .ok_or_else(|| anyhow!("__NEXT_DATA__ not found on {CITY_PAGE}"))?;

        let data: serde_json::Value = serde_json::from_str(&blob)?;
        let events = data
            .pointer("/props/pageProps/initialData/data/events")
            .and_then(|v| v.as_array())
            .ok_or_else(|| anyhow!("no events array in __NEXT_DATA__"))?;

        Ok(events
            .iter()
            .take(LIMIT)
            .filter_map(|entry| {
                let ev = entry.get("event")?;
                let name = ev.get("name")?.as_str().filter(|s| !s.is_empty())?;

                // Fall back to "Online" for anything not explicitly in-person.
                let where_ = ev
                    .pointer("/geo_address_info/city_state")
                    .and_then(|v| v.as_str())
                    .map(str::to_string)
                    .unwrap_or_else(|| match ev.get("location_type").and_then(|v| v.as_str()) {
                        Some("offline") => String::new(),
                        _ => "Online".to_string(),
                    });

                let start = ev.get("start_at").and_then(|v| v.as_str());
                let ts = start
                    .and_then(|s| DateTime::parse_from_rfc3339(s).ok())
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);

                let content = format!("{where_} — starts {}", start.unwrap_or("TBA"));

                Some(
                    Item::new(
                        name,
                        self.source_name(),
                        ItemType::Event,
                        format!(
                            "https://lu.ma/{}",
                            ev.get("url").and_then(|v| v.as_str()).unwrap_or_default()
                        ),
                    )
                    .with_content(content.trim_matches(|c| c == ' ' || c == '—').to_string())
                    .with_timestamp(ts)
                    .with_location(Some(where_)),
                )
            })
            .collect())
    }
}
