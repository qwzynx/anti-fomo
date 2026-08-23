//! Employer job boards, read directly from the ATS that serves them.
//!
//! One [`Scraper`](crate::scrapers::Scraper) per ATS family rather than one
//! per employer: `all_scrapers()` gains five entries instead of a hundred, and
//! the Sources health list in Settings stays readable. Which employers each
//! one covers is `super::employers::EMPLOYERS`.
//!
//! `source_platform` is the ATS family; `company` is the employer. That split
//! matters to ranking — `rank::diversify` buckets on the employer, so one
//! Workday tenant with two thousand postings cannot take the whole first page,
//! while `feed_status` still reports five sources rather than a hundred.

use futures::{stream, StreamExt};
use std::future::Future;
use std::pin::Pin;

use super::employers::{Board, Employer, EMPLOYERS};
use crate::models::{Item, JobDetail};

pub mod ashby;
pub mod greenhouse;
pub mod lever;
pub mod smartrecruiters;
pub mod workday;

/// Employer boards fetched at once. Each of these is a different host, so the
/// limit is about not looking like a crawler rather than about politeness to
/// any one of them.
const CONCURRENCY: usize = 8;

/// One employer board fetch, boxed.
///
/// Boxed rather than generic because a generic future here forces the closure
/// into a higher-ranked bound the compiler cannot satisfy — the employer
/// reference is `'static` but inference will not commit to that. One
/// allocation per board is nothing against the request it wraps.
pub type BoardFuture = Pin<Box<dyn Future<Output = anyhow::Result<Vec<Item>>> + Send + 'static>>;

/// Runs `fetch` over every employer whose board `pick` claims, concurrently.
///
/// One employer's board failing yields nothing for that employer and leaves
/// the rest alone — the same contract `fetch_all` gives a whole source, one
/// level down. A bank changing its Workday site name must not empty the feed.
pub async fn fan_out(
    client: &reqwest::Client,
    pick: fn(&Board) -> bool,
    fetch: fn(&'static Employer, reqwest::Client) -> BoardFuture,
) -> Vec<Item> {
    // The per-board futures are built eagerly here rather than inside a
    // stream combinator. A combinator closure that receives the boxed future
    // forces a higher-ranked lifetime bound that `async_trait`'s own boxed
    // future cannot satisfy; building them in a plain `map` before the stream
    // exists sidesteps the question.
    let jobs: Vec<_> = EMPLOYERS
        .iter()
        .filter(|e| pick(&e.board))
        .map(|employer| {
            let future = fetch(employer, client.clone());
            let name = employer.name;
            async move {
                match future.await {
                    Ok(items) => items,
                    Err(e) => {
                        log::warn!("{name} board failed: {e}");
                        Vec::new()
                    }
                }
            }
        })
        .collect();

    stream::iter(jobs)
        .buffer_unordered(CONCURRENCY)
        .flat_map(stream::iter)
        .collect()
        .await
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
