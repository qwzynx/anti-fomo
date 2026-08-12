//! Finds the latest TLDR Tech issue from the archive index, then scrapes that
//! issue's article blocks (title + summary), skipping sponsor slots.

use anyhow::{anyhow, Result};
use async_trait::async_trait;
use chrono::{NaiveDate, TimeZone, Utc};
use regex::Regex;
use scraper::{Html, Selector};
use std::sync::LazyLock;

use super::{collapse_ws, Scraper};
use crate::models::{Item, ItemType};

pub struct TldrTech;

const ARCHIVE: &str = "https://tldr.tech/tech/archives";

static ISSUE_HREF: LazyLock<Regex> =
    LazyLock::new(|| Regex::new(r#"href="/tech/(\d{4}-\d{2}-\d{2})""#).unwrap());
static ARTICLE: LazyLock<Selector> = LazyLock::new(|| Selector::parse("article").unwrap());
static HEADING: LazyLock<Selector> = LazyLock::new(|| Selector::parse("h3").unwrap());
static LINK: LazyLock<Selector> = LazyLock::new(|| Selector::parse("a[href]").unwrap());
static SUMMARY: LazyLock<Selector> =
    LazyLock::new(|| Selector::parse("div.newsletter-html").unwrap());

#[async_trait]
impl Scraper for TldrTech {
    fn source_name(&self) -> &'static str {
        "TLDR Tech"
    }

    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>> {
        let archive = client.get(ARCHIVE).send().await?.text().await?;

        // Dates are ISO, so lexical max is the newest issue.
        let latest = ISSUE_HREF
            .captures_iter(&archive)
            .map(|c| c[1].to_string())
            .max()
            .ok_or_else(|| anyhow!("no issues found in archive"))?;

        let issue = client
            .get(format!("https://tldr.tech/tech/{latest}"))
            .send()
            .await?
            .text()
            .await?;

        let issue_date = NaiveDate::parse_from_str(&latest, "%Y-%m-%d")
            .ok()
            .and_then(|d| d.and_hms_opt(0, 0, 0))
            .and_then(|dt| Utc.from_local_datetime(&dt).single())
            .unwrap_or_else(Utc::now);

        let doc = Html::parse_document(&issue);

        Ok(doc
            .select(&ARTICLE)
            .filter_map(|article| {
                let title =
                    collapse_ws(&article.select(&HEADING).next()?.text().collect::<String>());
                if title.to_lowercase().contains("(sponsor)") {
                    return None;
                }
                let href = article.select(&LINK).next()?.value().attr("href")?;

                let summary = article
                    .select(&SUMMARY)
                    .next()
                    .map(|d| collapse_ws(&d.text().collect::<String>()))
                    .unwrap_or_default();

                Some(
                    Item::new(title, self.source_name(), ItemType::Article, href)
                        .with_content(summary)
                        .with_timestamp(issue_date),
                )
            })
            .collect())
    }
}
