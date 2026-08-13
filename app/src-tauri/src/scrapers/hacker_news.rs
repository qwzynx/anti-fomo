use anyhow::Result;
use async_trait::async_trait;
use chrono::{TimeZone, Utc};
use serde::Deserialize;

use super::Scraper;
use crate::models::{Item, ItemType};

pub struct HackerNews;

/// Algolia serves the whole front page history under this tag (measured: ~180
/// hits), so 50 is a page of it rather than everything — enough to fill the
/// feed without spending four round trips on a category that is not the point
/// of the app.
const ENDPOINT: &str =
    "https://hn.algolia.com/api/v1/search_by_date?tags=front_page&hitsPerPage=50";

#[derive(Deserialize)]
struct Response {
    #[serde(default)]
    hits: Vec<Hit>,
}

#[derive(Deserialize)]
struct Hit {
    title: Option<String>,
    url: Option<String>,
    #[serde(rename = "objectID")]
    object_id: Option<String>,
    story_text: Option<String>,
    created_at_i: Option<i64>,
}

#[async_trait]
impl Scraper for HackerNews {
    fn source_name(&self) -> &'static str {
        "Hacker News"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        // The Algolia mirror returns clean JSON, which beats parsing HN's HTML.
        let resp: Response = client.get(ENDPOINT).send().await?.json().await?;

        Ok(resp
            .hits
            .into_iter()
            .filter_map(|hit| {
                let title = hit.title?;
                // Ask HN and similar posts carry no external URL; fall back to
                // the discussion thread.
                let url = hit.url.filter(|u| !u.is_empty()).unwrap_or_else(|| {
                    format!(
                        "https://news.ycombinator.com/item?id={}",
                        hit.object_id.clone().unwrap_or_default()
                    )
                });
                let ts = hit
                    .created_at_i
                    .and_then(|s| Utc.timestamp_opt(s, 0).single())
                    .unwrap_or_else(Utc::now);

                Some(
                    Item::new(title, self.source_name(), ItemType::Article, url)
                        .with_content(hit.story_text.unwrap_or_default())
                        .with_timestamp(ts),
                )
            })
            .collect())
    }
}
