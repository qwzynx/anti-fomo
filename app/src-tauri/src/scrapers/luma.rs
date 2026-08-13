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

/// One scraper covering several city pages rather than one registered scraper
/// per city: the source filter and the health list should show a single "Luma"
/// entry, and the events already carry their city in `location`.
///
/// The three Canadian cities come first because that is where this app's user
/// actually is; the US hubs are here because a remote-friendly event is worth
/// seeing wherever it is hosted. `lu.ma/waterloo` exists but is consistently
/// empty, so it is left out rather than costing a request per refresh.
const CITY_PAGES: &[&str] = &[
    "https://lu.ma/toronto",
    "https://lu.ma/vancouver",
    "https://lu.ma/montreal",
    "https://lu.ma/sf",
    "https://lu.ma/nyc",
    "https://lu.ma/seattle",
    "https://lu.ma/boston",
    "https://lu.ma/la",
];
/// Per city, not overall. A city page embeds 20 upcoming events in its
/// `__NEXT_DATA__` blob and no more, so this takes all of them.
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
        // Concurrent, like the scraper runner one level up: eight city pages
        // at ~50 KB each is the slowest source in the pipeline if walked in
        // sequence, and they share nothing with one another.
        let results =
            futures::future::join_all(CITY_PAGES.iter().map(|page| self.fetch_city(client, page)))
                .await;

        let mut all = Vec::new();
        let mut last_err = None;

        for (page, result) in CITY_PAGES.iter().zip(results) {
            match result {
                Ok(items) => all.extend(items),
                // One city failing shouldn't cost the others, mirroring the
                // per-source degradation contract one level down.
                Err(e) => {
                    log::warn!("Luma: {page} failed: {e}");
                    last_err = Some(e);
                }
            }
        }

        match last_err {
            Some(e) if all.is_empty() => Err(e),
            _ => Ok(all),
        }
    }
}

impl Luma {
    async fn fetch_city(&self, client: &reqwest::Client, page: &str) -> Result<Vec<Item>> {
        let body = client.get(page).send().await?.text().await?;

        let blob = NEXT_DATA
            .captures(&body)
            .map(|c| c[1].to_string())
            .ok_or_else(|| anyhow!("__NEXT_DATA__ not found on {page}"))?;

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
