//! Employer job boards, read directly from the ATS that serves them.
//!
//! One [`Scraper`](crate::scrapers::Scraper) per ATS family rather than one
//! per employer: `all_scrapers()` gains five entries instead of a hundred, and
//! the Sources health list in Settings stays readable. Which employers each
//! one covers is `super::boards::all()`: the curated `EMPLOYERS`, the bundled
//! list and whatever the cache has taught us.
//!
//! `source_platform` is the ATS family; `company` is the employer. That split
//! matters to ranking — `rank::diversify` buckets on the employer, so one
//! Workday tenant with two thousand postings cannot take the whole first page,
//! while `feed_status` still reports five sources rather than a hundred.

use futures::{stream, StreamExt};
use std::future::Future;
use std::pin::Pin;

use super::boards;
use super::employers::{Board, Employer};
use super::Sink;
use crate::models::{Item, JobDetail};
use crate::rank::{classify_item, DEFAULT_MAJOR};

pub mod ashby;
pub mod breezy;
pub mod greenhouse;
pub mod lever;
pub mod recruitee;
pub mod smartrecruiters;
pub mod workable;
pub mod workday;

/// Employer boards fetched at once, per ATS family. A family is one API host
/// (Greenhouse's `boards-api`, Lever's `api.lever.co`), and those are built for
/// exactly this: public, cacheable, read by thousands of careers pages. The
/// limit is about not looking like a crawler.
pub const CONCURRENCY: usize = 16;
/// Workday is a different host per tenant, each a real employer's careers
/// stack, and a request there is a search rather than a cached file.
pub const WORKDAY_CONCURRENCY: usize = 8;

/// Recruitee answers 429 well below the other families' rates (every board is a
/// subdomain of one host, so the limit is shared by all of them).
pub const RECRUITEE_CONCURRENCY: usize = 3;

/// How many boards of this family the scraper fetches at once.
pub fn concurrency_for(board: &Board) -> usize {
    match board {
        Board::Workday { .. } => WORKDAY_CONCURRENCY,
        Board::Recruitee(_) => RECRUITEE_CONCURRENCY,
        _ => CONCURRENCY,
    }
}

/// GETs a JSON document, waiting out a 429 rather than reporting a healthy
/// board as dead. Two retries, backing off; anything else is an error as
/// usual.
pub async fn get_json<T: serde::de::DeserializeOwned>(
    client: &reqwest::Client,
    url: &str,
) -> anyhow::Result<T> {
    let mut wait = std::time::Duration::from_millis(1500);
    for attempt in 0..3 {
        let response = client.get(url).send().await?;
        if response.status() == reqwest::StatusCode::TOO_MANY_REQUESTS && attempt < 2 {
            let hinted = response
                .headers()
                .get(reqwest::header::RETRY_AFTER)
                .and_then(|v| v.to_str().ok())
                .and_then(|v| v.parse::<u64>().ok())
                .map(|s| std::time::Duration::from_secs(s.min(10)));
            tokio::time::sleep(hinted.unwrap_or(wait)).await;
            wait *= 2;
            continue;
        }
        let body = response.error_for_status()?.bytes().await?;
        // Through serde directly, so a field that is not what we assumed is
        // named in the error instead of "error decoding response body".
        return serde_json::from_slice(&body)
            .map_err(|e| anyhow::anyhow!("{url}: unreadable response: {e}"));
    }
    unreachable!("the last attempt returns")
}

/// One employer board fetch, boxed.
///
/// Boxed rather than generic because a generic future here forces the closure
/// into a higher-ranked bound the compiler cannot satisfy — the employer
/// reference is `'static` but inference will not commit to that. One
/// allocation per board is nothing against the request it wraps.
pub type BoardFuture = Pin<Box<dyn Future<Output = anyhow::Result<Vec<Item>>> + Send + 'static>>;

/// One employer's board, fetched by whichever module reads its ATS. For the
/// tools that want to try a board on its own; the scrapers go through
/// [`fan_out`].
pub fn fetch_one(employer: &'static Employer, client: reqwest::Client) -> BoardFuture {
    match employer.board {
        Board::Greenhouse(_) => Box::pin(greenhouse::fetch_board(employer, client)),
        Board::Lever(_) => Box::pin(lever::fetch_board(employer, client)),
        Board::Ashby(_) => Box::pin(ashby::fetch_board(employer, client)),
        Board::SmartRecruiters(_) => Box::pin(smartrecruiters::fetch_board(employer, client)),
        Board::Workable(_) => Box::pin(workable::fetch_board(employer, client)),
        Board::Recruitee(_) => Box::pin(recruitee::fetch_board(employer, client)),
        Board::Breezy(_) => Box::pin(breezy::fetch_board(employer, client)),
        Board::Workday { .. } => Box::pin(workday::fetch_board(employer, client)),
    }
}

/// Boards finished between hand-overs. Small enough that the first rows reach
/// the reader within seconds of the scrape starting, large enough that a
/// thousand-board scrape is a few dozen writes and not a thousand.
const BATCH_BOARDS: usize = 40;

/// Runs `fetch` over every employer whose board `pick` claims, concurrently,
/// handing the rows to `sink` as boards finish.
///
/// One employer's board failing yields nothing for that employer and leaves
/// the rest alone — the same contract `fetch_all` gives a whole source, one
/// level down. A bank changing its Workday site name must not empty the feed.
///
/// A board's rows always travel together, in one piece: the liveness sweep
/// reasons per employer, and half of a board is a board that looks to have
/// lost postings it still has.
pub async fn fan_out(
    client: &reqwest::Client,
    pick: fn(&Board) -> bool,
    fetch: fn(&'static Employer, reqwest::Client) -> BoardFuture,
    concurrency: usize,
    sink: Sink<'_>,
) {
    // The per-board futures are built eagerly here rather than inside a
    // stream combinator. A combinator closure that receives the boxed future
    // forces a higher-ranked lifetime bound that `async_trait`'s own boxed
    // future cannot satisfy; building them in a plain `map` before the stream
    // exists sidesteps the question.
    let jobs: Vec<_> = boards::all()
        .into_iter()
        .filter(|e| pick(&e.board))
        .map(|employer| {
            let future = fetch(employer, client.clone());
            let name = employer.name;
            let board = employer.board;
            async move {
                let result = future.await;
                boards::report(&board, boards::Answer::of(&result));
                match result {
                    Ok(items) => items,
                    Err(e) => {
                        log::warn!("{name} board failed: {e}");
                        Vec::new()
                    }
                }
            }
        })
        .collect();

    let mut boards = stream::iter(jobs).buffer_unordered(concurrency);
    let mut pending: Vec<Item> = Vec::new();
    let mut finished = 0usize;
    while let Some(items) = boards.next().await {
        pending.extend(items);
        finished += 1;
        if finished.is_multiple_of(BATCH_BOARDS) && !pending.is_empty() {
            sink(std::mem::take(&mut pending));
        }
    }
    if !pending.is_empty() {
        sink(pending);
    }
}

/// Trims one employer's postings to `cap`, keeping the ones this app is about.
///
/// Most boards are under their cap and pass through untouched. The few that
/// are not — Anduril and SpaceX each publish over two thousand roles — used to
/// be cut to the newest few hundred, which for a company that is mostly
/// manufacturing and operations meant a handful of engineering roles among
/// hundreds of unrelated ones. The software roles go first, newest first
/// within each group, so the cap spends its slots where the reader looks.
pub fn keep_best(mut items: Vec<Item>, cap: usize) -> Vec<Item> {
    if items.len() <= cap {
        return items;
    }
    // Cached key: classifying reads the title and the text, and a comparator
    // would redo it for both sides of every comparison.
    items.sort_by_cached_key(|item| {
        (
            classify_item(item) != DEFAULT_MAJOR,
            std::cmp::Reverse(item.timestamp),
        )
    });
    items.truncate(cap);
    items
}

/// Copies a description a *list* endpoint already carried onto the item.
///
/// Lever and Ashby hand back the whole posting in the same response as the
/// listing, so re-fetching it one URL at a time in the enrichment pass would
/// be several thousand requests for text we are already holding. `persist`
/// harvests these into `job_details`, which is where a description has to live
/// — `save_items` rewrites `content_text` on every refresh and would erase one
/// stored on the row.
pub fn seed_detail(item: &mut Item, detail: JobDetail) {
    item.description = detail.description;
    item.requirements = detail.requirements;
    item.responsibilities = detail.responsibilities;
    item.perks = detail.perks;
    item.tagged_skills = detail.tagged_skills;
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::ItemType;
    use chrono::{Duration, Utc};

    fn job(title: &str, days_ago: i64) -> Item {
        Item::new(
            title,
            "Greenhouse",
            ItemType::Job,
            format!("https://x.test/{title}"),
        )
        .with_timestamp(Utc::now() - Duration::days(days_ago))
    }

    fn titles(items: Vec<Item>) -> Vec<String> {
        items.into_iter().map(|i| i.title).collect()
    }

    #[test]
    fn a_board_under_its_cap_comes_back_exactly_as_it_was() {
        let items = vec![job("Machinist", 1), job("Software Engineer", 9)];
        assert_eq!(
            titles(keep_best(items, 5)),
            ["Machinist", "Software Engineer"]
        );
    }

    /// A board over its cap is where the choice matters: newest-first would
    /// keep the machinist and the recruiter and drop the month-old engineer.
    #[test]
    fn over_the_cap_software_roles_go_first_and_then_the_newest() {
        let items = vec![
            job("Machinist", 1),
            job("Software Engineer, Backend", 30),
            job("Software Engineer, Platform", 5),
            job("Recruiter", 2),
        ];
        assert_eq!(
            titles(keep_best(items, 3)),
            [
                "Software Engineer, Platform",
                "Software Engineer, Backend",
                "Machinist"
            ]
        );
    }
}
