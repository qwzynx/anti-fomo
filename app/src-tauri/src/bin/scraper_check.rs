//! Per-source health check, plus an end-to-end pass over the real pipeline.
//! Run with `cargo run --bin scraper_check`.
//!
//! Two phases: every scraper is hit individually so a broken selector is
//! attributable, then `fetch_all` runs the genuine dedupe/classify/rank path so
//! the composition of the finished feed is visible.

use std::collections::BTreeMap;

use anti_fomo_lib::rank::{personalize, Profile, DEFAULT_MAJOR};
use anti_fomo_lib::scrapers;

#[tokio::main]
async fn main() {
    let client = scrapers::build_client().expect("http client");

    println!("{:<22} {:>6}  STATUS", "SOURCE", "ITEMS");
    println!("{}", "-".repeat(70));

    let mut failed = 0usize;

    for scraper in scrapers::all_scrapers() {
        let name = scraper.source_name();
        let started = std::time::Instant::now();
        match scraper.fetch(&client).await {
            Ok(items) => {
                let status = if items.is_empty() {
                    failed += 1;
                    "EMPTY — check selectors".to_string()
                } else {
                    format!("ok ({}ms)", started.elapsed().as_millis())
                };
                println!("{name:<22} {:>6}  {status}", items.len());
                if let Some(first) = items.first() {
                    let title: String = first.title.chars().take(52).collect();
                    println!("{:<22} {:>6}  └ {title}", "", "");
                }
            }
            Err(e) => {
                failed += 1;
                println!("{name:<22} {:>6}  ERROR: {e}", 0);
            }
        }
    }

    // Phase two: the real pipeline, so dedupe and ranking are exercised too.
    println!("{}", "-".repeat(70));
    let items = scrapers::fetch_all().await;

    let mut by_type: BTreeMap<&str, usize> = BTreeMap::new();
    let mut undated = 0usize;
    for item in &items {
        *by_type.entry(item.item_type.as_str()).or_insert(0) += 1;
        if item.content_text.trim().is_empty() {
            undated += 1;
        }
    }

    println!("after dedupe: {} items", items.len());
    for (kind, count) in &by_type {
        println!("  {kind:<12} {count:>5}");
    }
    println!("  {:<12} {undated:>5}", "no summary");

    // A feed where one source owns the whole first page is a ranking failure
    // even when every scraper reports healthy, so check the head composition.
    let profile = Profile {
        major: DEFAULT_MAJOR.to_string(),
        ..Default::default()
    };
    let ranked = personalize(items, &profile);
    let head: Vec<&str> = ranked
        .iter()
        .take(20)
        .map(|i| i.source_platform.as_str())
        .collect();
    let mut spread: BTreeMap<&str, usize> = BTreeMap::new();
    for src in &head {
        *spread.entry(src).or_insert(0) += 1;
    }
    println!("top 20 spans {} sources: {:?}", spread.len(), spread);

    let events = by_type.get("Event").copied().unwrap_or(0);
    println!("{}", "-".repeat(70));
    println!("{failed} problematic sources, {events} events");

    if failed > 0 {
        std::process::exit(1);
    }
}
