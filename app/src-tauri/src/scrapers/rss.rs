//! Straight RSS sources. `feed-rs` handles both RSS and Atom, replacing the
//! BeautifulSoup `xml` parser (and with it the lxml C dependency).

use anyhow::Result;
use async_trait::async_trait;
use chrono::Utc;

use super::{strip_html, Scraper};
use crate::models::{Item, ItemType};

pub struct RssSource {
    pub name: &'static str,
    pub url: &'static str,
    pub limit: usize,
}

pub const PHORONIX: RssSource = RssSource {
    name: "Phoronix",
    url: "https://www.phoronix.com/rss.php",
    limit: 10,
};

// The site 403s generic clients, which the browser User-Agent on the shared
// client handles. Note /news/feed/ is a WordPress *comments* feed; the
// site-wide /feed/ carries the actual news posts.
pub const LASSONDE: RssSource = RssSource {
    name: "Lassonde News",
    url: "https://lassonde.yorku.ca/feed/",
    limit: 15,
};

#[async_trait]
impl Scraper for RssSource {
    fn source_name(&self) -> &'static str {
        self.name
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let body = client.get(self.url).send().await?.bytes().await?;
        let feed = feed_rs::parser::parse(body.as_ref())?;

        Ok(feed
            .entries
            .into_iter()
            .take(self.limit)
            .filter_map(|entry| {
                let title = entry.title.map(|t| t.content)?;
                let link = entry.links.first().map(|l| l.href.clone())?;

                let summary = entry
                    .summary
                    .map(|s| s.content)
                    .or_else(|| entry.content.and_then(|c| c.body))
                    .unwrap_or_default();

                Some(
                    Item::new(title.trim(), self.name, ItemType::Article, link.trim())
                        .with_content(strip_html(&summary))
                        .with_timestamp(entry.published.or(entry.updated).unwrap_or_else(Utc::now)),
                )
            })
            .collect())
    }
}
