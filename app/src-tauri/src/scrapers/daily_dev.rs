//! Daily.dev used to be the one source that needed a headless browser: the old
//! Python scraper drove Playwright to app.daily.dev and read links out of the
//! rendered DOM. Their public GraphQL endpoint serves the same tag feed
//! anonymously, which removes the ~150 MB Chromium dependency entirely — and
//! it is the reason this app can run on Android at all.

use anyhow::Result;
use async_trait::async_trait;
use chrono::{DateTime, Utc};
use serde::Deserialize;

use super::Scraper;
use crate::models::{Item, ItemType};

pub struct DailyDev;

const ENDPOINT: &str = "https://api.daily.dev/graphql";
const TAG: &str = "software-engineering";
const LIMIT: usize = 15;

const QUERY: &str = r#"
query TagFeed($first: Int, $tag: String!) {
  page: tagFeed(first: $first, tag: $tag, ranking: TIME) {
    edges { node { title permalink createdAt summary } }
  }
}"#;

#[derive(Deserialize)]
struct Response {
    data: Option<Data>,
}

#[derive(Deserialize)]
struct Data {
    page: Option<Page>,
}

#[derive(Deserialize)]
struct Page {
    #[serde(default)]
    edges: Vec<Edge>,
}

#[derive(Deserialize)]
struct Edge {
    node: Node,
}

#[derive(Deserialize)]
struct Node {
    title: Option<String>,
    permalink: Option<String>,
    #[serde(rename = "createdAt")]
    created_at: Option<String>,
    summary: Option<String>,
}

#[async_trait]
impl Scraper for DailyDev {
    fn source_name(&self) -> &'static str {
        "Daily.dev"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let resp: Response = client
            .post(ENDPOINT)
            .json(&serde_json::json!({
                "query": QUERY,
                "variables": { "first": LIMIT, "tag": TAG },
            }))
            .send()
            .await?
            .json()
            .await?;

        let edges = resp
            .data
            .and_then(|d| d.page)
            .map(|p| p.edges)
            .unwrap_or_default();

        Ok(edges
            .into_iter()
            .filter_map(|edge| {
                let node = edge.node;
                let title = node.title.filter(|t| !t.is_empty())?;
                let url = node.permalink.filter(|u| !u.is_empty())?;
                let ts = node
                    .created_at
                    .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
                    .map(|d| d.with_timezone(&Utc))
                    .unwrap_or_else(Utc::now);

                Some(
                    Item {
                        // The feed is tag-filtered to software engineering, so
                        // the keyword classifier has nothing to add.
                        discipline: Some("Software Engineering".to_string()),
                        ..Item::new(title, self.source_name(), ItemType::Article, url)
                    }
                    // The API hands us an AI summary the Playwright scraper
                    // never had access to.
                    .with_content(node.summary.unwrap_or_default())
                    .with_timestamp(ts),
                )
            })
            .collect())
    }
}
