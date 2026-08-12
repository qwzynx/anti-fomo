use anyhow::Result;
use async_trait::async_trait;
use scraper::{Html, Selector};
use std::sync::LazyLock;

use super::{collapse_ws, Scraper};
use crate::models::{Item, ItemType};

pub struct HnTopLinks;

const ENDPOINT: &str = "https://hntoplinks.com/";

static STORY: LazyLock<Selector> = LazyLock::new(|| Selector::parse(".story").unwrap());
static TITLE: LazyLock<Selector> = LazyLock::new(|| Selector::parse(".story-title").unwrap());

#[async_trait]
impl Scraper for HnTopLinks {
    fn source_name(&self) -> &'static str {
        "HN Top Links"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let body = client.get(ENDPOINT).send().await?.text().await?;
        let doc = Html::parse_document(&body);

        Ok(doc
            .select(&STORY)
            .filter_map(|story| {
                let el = story.select(&TITLE).next()?;
                let title = collapse_ws(&el.text().collect::<String>());
                let url = el.value().attr("href").unwrap_or_default();
                if title.is_empty() || url.is_empty() {
                    return None;
                }
                Some(Item {
                    discipline: Some("Software Engineering".to_string()),
                    ..Item::new(title, self.source_name(), ItemType::Article, url)
                })
            })
            .collect())
    }
}
