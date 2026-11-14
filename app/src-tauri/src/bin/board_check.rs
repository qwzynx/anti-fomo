//! Probes every employer board the scrapers know — the curated table, the
//! bundled list and any boards learned from a cache — and reports the dead,
//! the empty and the totals per ATS. The gate on adding a board.
//! Run with `cargo run --release --features dev-tools --bin board_check`.
//!
//! `ATS=greenhouse,lever` limits it to some families; `SHOW=all` lists healthy
//! boards too.

use std::collections::BTreeMap;

use anti_fomo_lib::scrapers::{self, ats, boards, employers::Board};
use futures::{stream, StreamExt};

fn kind(board: &Board) -> &'static str {
    match board {
        Board::Greenhouse(_) => "greenhouse",
        Board::Lever(_) => "lever",
        Board::Ashby(_) => "ashby",
        Board::SmartRecruiters(_) => "smartrecruiters",
        Board::Workable(_) => "workable",
        Board::Recruitee(_) => "recruitee",
        Board::Breezy(_) => "breezy",
        Board::Workday { .. } => "workday",
    }
}

#[tokio::main]
async fn main() {
    let client = scrapers::build_client().expect("http client");
    let only: Option<Vec<String>> = std::env::var("ATS").ok().map(|v| {
        v.split(',')
            .map(|s| s.trim().to_ascii_lowercase())
            .collect()
    });
    let show_all = std::env::var("SHOW").is_ok_and(|v| v == "all");

    let all: Vec<_> = boards::all()
        .into_iter()
        .filter(|e| {
            only.as_ref()
                .is_none_or(|o| o.iter().any(|k| k == kind(&e.board)))
        })
        .collect();
    println!("probing {} boards", all.len());

    // Each family at the pace its scraper runs at, and the families side by
    // side, as in a real refresh.
    let started = std::time::Instant::now();
    let mut groups: BTreeMap<&str, Vec<&'static scrapers::employers::Employer>> = BTreeMap::new();
    for employer in all {
        groups
            .entry(kind(&employer.board))
            .or_default()
            .push(employer);
    }
    let probes = groups.into_values().map(|group| {
        let client = client.clone();
        let limit = ats::concurrency_for(&group[0].board);
        stream::iter(group)
            .map(move |employer| {
                let future = ats::fetch_one(employer, client.clone());
                async move { (employer, future.await) }
            })
            .buffer_unordered(limit)
            .collect::<Vec<_>>()
    });
    let results: Vec<_> = futures::future::join_all(probes)
        .await
        .into_iter()
        .flatten()
        .collect();

    #[derive(Default)]
    struct Tally {
        boards: usize,
        live: usize,
        empty: usize,
        dead: usize,
        postings: usize,
    }
    let mut by_kind: BTreeMap<&str, Tally> = BTreeMap::new();
    for (employer, result) in &results {
        let tally = by_kind.entry(kind(&employer.board)).or_default();
        tally.boards += 1;
        match result {
            Ok(items) if items.is_empty() => {
                tally.empty += 1;
                println!("EMPTY {:<16} {}", kind(&employer.board), employer.name);
            }
            Ok(items) => {
                tally.live += 1;
                tally.postings += items.len();
                if show_all {
                    println!(
                        "ok    {:<16} {} ({})",
                        kind(&employer.board),
                        employer.name,
                        items.len()
                    );
                }
            }
            Err(e) => {
                tally.dead += 1;
                println!(
                    "DEAD  {:<16} {} — {e}",
                    kind(&employer.board),
                    employer.name
                );
            }
        }
    }

    println!("{}", "-".repeat(70));
    println!(
        "{:<16} {:>6} {:>6} {:>6} {:>6} {:>9}",
        "ATS", "boards", "live", "empty", "dead", "postings"
    );
    for (k, t) in &by_kind {
        println!(
            "{k:<16} {:>6} {:>6} {:>6} {:>6} {:>9}",
            t.boards, t.live, t.empty, t.dead, t.postings
        );
    }
    println!("{:.0}s", started.elapsed().as_secs_f64());
}
