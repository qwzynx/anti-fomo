//! The scraping pipeline. Every source implements [`Scraper`]; they run
//! concurrently and a failing source logs and yields nothing rather than
//! taking the whole refresh down — the same degradation contract the Python
//! pipeline had, where each scraper wrapped its body in try/except.

use anyhow::Result;
use async_trait::async_trait;
use std::collections::HashSet;
use std::time::Duration;

use crate::location::normalize_location;
use crate::models::Item;
use crate::rank::classify_item;

pub mod daily_dev;
pub mod github_internships;
pub mod hacker_news;
pub mod hn_top_links;
pub mod levels_fyi;
pub mod luma;
pub mod rss;
pub mod tldr;

/// Several sources (notably the Lassonde WordPress feed) 403 anything that
/// doesn't look like a browser.
pub const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

#[async_trait]
pub trait Scraper: Send + Sync {
    fn source_name(&self) -> &'static str;
    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>>;
}

pub fn build_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?)
}

pub fn all_scrapers() -> Vec<Box<dyn Scraper>> {
    vec![
        Box::new(hacker_news::HackerNews),
        Box::new(github_internships::PITT_CSC),
        Box::new(rss::PHORONIX),
        Box::new(github_internships::SIMPLIFY),
        Box::new(rss::LASSONDE),
        Box::new(luma::Luma),
        Box::new(levels_fyi::LevelsFyi),
        Box::new(tldr::TldrTech),
        Box::new(hn_top_links::HnTopLinks),
        Box::new(daily_dev::DailyDev),
    ]
}

/// Runs every scraper concurrently, then dedupes by URL, backfills the
/// discipline classification, and derives location tags.
pub async fn fetch_all() -> Vec<Item> {
    let client = match build_client() {
        Ok(c) => c,
        Err(e) => {
            log::error!("could not build HTTP client: {e}");
            return Vec::new();
        }
    };

    let results = futures::future::join_all(all_scrapers().into_iter().map(|s| {
        let client = client.clone();
        async move {
            let name = s.source_name();
            match s.fetch(&client).await {
                Ok(items) => {
                    log::info!("{name}: {} items", items.len());
                    items
                }
                Err(e) => {
                    log::error!("Error scraping {name}: {e}");
                    Vec::new()
                }
            }
        }
    }))
    .await;

    let mut seen: HashSet<String> = HashSet::new();
    let mut deduped = Vec::new();

    for mut item in results.into_iter().flatten() {
        if item.url.is_empty() || !seen.insert(item.url.clone()) {
            continue;
        }
        if item.discipline.is_none() {
            item.discipline = Some(classify_item(&item));
        }
        item.location_tags = normalize_location(item.location.as_deref());
        deduped.push(item);
    }

    deduped
}

// --- shared parsing helpers ---

/// Flattens an HTML fragment to its visible text, collapsing whitespace.
/// Equivalent to BeautifulSoup's `get_text(" ", strip=True)`.
pub fn strip_html(html: &str) -> String {
    let fragment = scraper::Html::parse_fragment(html);
    let text: String = fragment.root_element().text().collect::<Vec<_>>().join(" ");
    collapse_ws(&text)
}

pub fn collapse_ws(s: &str) -> String {
    s.split_whitespace().collect::<Vec<_>>().join(" ")
}
