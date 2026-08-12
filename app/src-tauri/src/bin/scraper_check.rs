//! Per-source item counts, for checking parity against the old Python
//! `check_scraper_counts.py`. Run with `cargo run --bin scraper_check`.

use anti_fomo_lib::scrapers;

#[tokio::main]
async fn main() {
    let client = scrapers::build_client().expect("http client");

    println!("{:<18} {:>6}  STATUS", "SOURCE", "ITEMS");
    println!("{}", "-".repeat(60));

    let mut total = 0usize;
    let mut failed = 0usize;

    for scraper in scrapers::all_scrapers() {
        let name = scraper.source_name();
        let started = std::time::Instant::now();
        match scraper.fetch(&client).await {
            Ok(items) => {
                total += items.len();
                let status = if items.is_empty() {
                    failed += 1;
                    "EMPTY — check selectors".to_string()
                } else {
                    format!("ok ({}ms)", started.elapsed().as_millis())
                };
                println!("{name:<18} {:>6}  {status}", items.len());
                if let Some(first) = items.first() {
                    let title: String = first.title.chars().take(60).collect();
                    println!("{:<18} {:>6}  └ {title}", "", "");
                }
            }
            Err(e) => {
                failed += 1;
                println!("{name:<18} {:>6}  ERROR: {e}", 0);
            }
        }
    }

    println!("{}", "-".repeat(60));
    println!(
        "{total} items across {} sources, {failed} problematic",
        scrapers::all_scrapers().len()
    );

    if failed > 0 {
        std::process::exit(1);
    }
}
