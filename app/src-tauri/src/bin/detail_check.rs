//! Coverage check for job-description enrichment, in the style of
//! `scraper_check`: how many cached postings a handler can actually serve, and
//! what comes back.
//!
//! Reads the app's real database rather than scraping, so it measures the
//! corpus the user actually has. Run it after a refresh:
//!
//! ```text
//! cargo run --bin detail_check -- [db_path] [sample_size]
//! ```

use std::collections::HashMap;

use anti_fomo_lib::models::DetailStatus;
use anti_fomo_lib::scrapers::{build_client, details};
use anti_fomo_lib::{db, skills};
use futures::stream::{self, StreamExt};

const DEFAULT_SAMPLE: usize = 60;
const CONCURRENCY: usize = 8;

fn default_db() -> std::path::PathBuf {
    dirs_next()
        .join("dev.qwzynx.antifomo")
        .join("antifomo.db")
}

fn dirs_next() -> std::path::PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var("HOME").expect("HOME"))
                .join(".local")
                .join("share")
        })
}

#[tokio::main]
async fn main() -> anyhow::Result<()> {
    let mut args = std::env::args().skip(1);
    let path = args
        .next()
        .map(std::path::PathBuf::from)
        .unwrap_or_else(default_db);
    let sample: usize = args
        .next()
        .and_then(|s| s.parse().ok())
        .unwrap_or(DEFAULT_SAMPLE);

    let conn = db::open(&path)?;
    let items = db::load_items(&conn)?;
    let opportunities: Vec<_> = items
        .iter()
        .filter(|i| i.item_type.is_opportunity())
        .collect();

    println!(
        "{} opportunities of {} cached items\n",
        opportunities.len(),
        items.len()
    );

    // --- routing coverage, no network ---
    let mut routable = 0usize;
    let mut by_lead: HashMap<&str, usize> = HashMap::new();
    let mut chain_len: HashMap<usize, usize> = HashMap::new();
    let mut unrouted_hosts: HashMap<String, usize> = HashMap::new();

    for item in &opportunities {
        let chain = details::route(&item.url, item.simplify_id.as_deref());
        *chain_len.entry(chain.len()).or_default() += 1;
        match chain.first() {
            Some(handler) => {
                routable += 1;
                *by_lead.entry(handler.name()).or_default() += 1;
            }
            None => *unrouted_hosts.entry(url_host(&item.url)).or_default() += 1,
        }
    }

    println!("=== routing (no network) ===");
    println!("  by the handler tried first:");
    let mut leads: Vec<_> = by_lead.into_iter().collect();
    leads.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (name, n) in &leads {
        println!("  {n:>5} ({:>4.1}%)  {name}", pct(*n, opportunities.len()));
    }
    println!(
        "  {routable:>5} ({:>4.1}%)  ROUTABLE TOTAL",
        pct(routable, opportunities.len())
    );

    let mut lengths: Vec<_> = chain_len.into_iter().collect();
    lengths.sort();
    let fallbacks = lengths
        .iter()
        .map(|(len, n)| format!("{len} handler(s): {n}"))
        .collect::<Vec<_>>()
        .join(", ");
    println!("  chain depth — {fallbacks}");

    let mut tail: Vec<_> = unrouted_hosts.into_iter().collect();
    tail.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    if !tail.is_empty() {
        println!("\n  not routable, top hosts:");
        for (host, n) in tail.iter().take(8) {
            println!("  {n:>5} ({:>4.1}%)  {host}", pct(*n, opportunities.len()));
        }
    }

    // --- live fetch of a sample ---
    // Every nth posting rather than the first n: the cache is ordered by
    // source, so the head of it is all one handler and would measure nothing.
    let step = (opportunities.len() / sample.max(1)).max(1);
    let targets: Vec<_> = opportunities
        .iter()
        .step_by(step)
        .filter_map(|i| {
            let chain = details::route(&i.url, i.simplify_id.as_deref());
            let lead = chain.first()?.name();
            Some((i.title.clone(), lead, chain))
        })
        .take(sample)
        .collect();

    if targets.is_empty() {
        println!("\nnothing routable to sample");
        return Ok(());
    }

    println!("\n=== live fetch of {} ===", targets.len());
    let client = build_client()?;
    let started = std::time::Instant::now();

    let results: Vec<(String, &str, anti_fomo_lib::models::JobDetail)> = stream::iter(targets)
        .map(|(title, lead, chain)| {
            let client = client.clone();
            async move { (title, lead, details::fetch(&client, &chain).await) }
        })
        .buffer_unordered(CONCURRENCY)
        .collect()
        .await;

    let elapsed = started.elapsed();
    let mut status_counts: HashMap<&str, usize> = HashMap::new();
    let (mut with_req, mut with_resp, mut with_perks, mut with_tags) = (0, 0, 0, 0);

    let mut by_handler: HashMap<&str, (usize, usize)> = HashMap::new();

    for (_, lead, d) in &results {
        *status_counts.entry(d.status.as_str()).or_default() += 1;
        let entry = by_handler.entry(lead).or_default();
        entry.0 += 1;
        entry.1 += usize::from(d.status == DetailStatus::Ok);
        if d.requirements.is_some() {
            with_req += 1;
        }
        if d.responsibilities.is_some() {
            with_resp += 1;
        }
        if d.perks.is_some() {
            with_perks += 1;
        }
        if !d.tagged_skills.is_empty() {
            with_tags += 1;
        }
    }

    println!(
        "  took {elapsed:?} at concurrency {CONCURRENCY} ({:.0}ms/posting)",
        elapsed.as_millis() as f64 / results.len() as f64
    );
    let mut statuses: Vec<_> = status_counts.into_iter().collect();
    statuses.sort_by_key(|(_, n)| std::cmp::Reverse(*n));
    for (status, n) in statuses {
        println!("  {n:>5}  status={status}");
    }
    let n = results.len();
    println!("  {with_req:>5} ({:>4.1}%)  have requirements", pct(with_req, n));
    println!("  {with_resp:>5} ({:>4.1}%)  have responsibilities", pct(with_resp, n));
    println!("  {with_perks:>5} ({:>4.1}%)  have perks", pct(with_perks, n));
    println!("  {with_tags:>5} ({:>4.1}%)  have tagged skills", pct(with_tags, n));

    println!("\n  by the handler tried first:");
    let mut handlers: Vec<_> = by_handler.into_iter().collect();
    handlers.sort_by_key(|(_, (tried, _))| std::cmp::Reverse(*tried));
    for (name, (tried, ok)) in handlers {
        println!("  {ok:>5} / {tried:<5} ({:>4.1}%)  {name}", pct(ok, tried));
    }

    // --- what enrichment does to skill extraction ---
    println!("\n=== skill extraction, before vs after ===");
    let mut before_total = 0usize;
    let mut after_total = 0usize;
    let mut before_none = 0usize;
    let mut after_none = 0usize;

    for (title, _, detail) in results.iter().take(n) {
        let Some(item) = opportunities.iter().find(|i| &i.title == title) else {
            continue;
        };
        let mut bare = (*item).clone();
        bare.requirements = None;
        bare.responsibilities = None;
        bare.description = None;
        bare.tagged_skills = Vec::new();
        bare.url = format!("{}#bare", bare.url);
        let before = skills::extract(&bare);

        let mut rich = (*item).clone();
        rich.requirements = detail.requirements.clone();
        rich.responsibilities = detail.responsibilities.clone();
        rich.description = detail.description.clone();
        rich.tagged_skills = detail.tagged_skills.clone();
        rich.url = format!("{}#rich", rich.url);
        let after = skills::extract(&rich);

        before_total += before.len();
        after_total += after.len();
        if before.is_empty() {
            before_none += 1;
        }
        if after.is_empty() {
            after_none += 1;
        }
    }

    println!(
        "  mean skills/posting: {:.1} -> {:.1}",
        before_total as f64 / n as f64,
        after_total as f64 / n as f64
    );
    println!(
        "  postings with none:  {} ({:.0}%) -> {} ({:.0}%)",
        before_none,
        pct(before_none, n),
        after_none,
        pct(after_none, n)
    );

    println!("\n=== three samples ===");
    for (title, lead, d) in results.iter().filter(|(_, _, d)| d.status == DetailStatus::Ok).take(3) {
        println!("\n  {} [{lead}]", title.chars().take(70).collect::<String>());
        for (label, field) in [
            ("about", &d.description),
            ("requirements", &d.requirements),
            ("responsibilities", &d.responsibilities),
            ("perks", &d.perks),
        ] {
            if let Some(text) = field {
                let first = text.lines().next().unwrap_or_default();
                println!("    {label}: {}", first.chars().take(90).collect::<String>());
            }
        }
        if !d.tagged_skills.is_empty() {
            println!("    tagged: {:?}", d.tagged_skills);
        }
    }

    Ok(())
}

fn pct(n: usize, total: usize) -> f64 {
    if total == 0 {
        0.0
    } else {
        100.0 * n as f64 / total as f64
    }
}

fn url_host(url: &str) -> String {
    url.split("://")
        .nth(1)
        .and_then(|rest| rest.split('/').next())
        .unwrap_or("?")
        .to_string()
}
