//! The scraping pipeline. Every source implements [`Scraper`]; they run
//! concurrently and a failing source logs and yields nothing rather than
//! taking the whole refresh down — the same degradation contract the Python
//! pipeline had, where each scraper wrapped its body in try/except.

use anyhow::Result;
use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use futures::FutureExt;
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::panic::AssertUnwindSafe;
use std::time::Duration;

use crate::companies;
use crate::db::Scope;
use crate::location::normalize_location;
use crate::models::{company_from_title, Item};
use crate::rank::{classify_item, classify_seniority};

pub mod ats;
pub mod boards;
pub mod daily_dev;
pub mod details;
pub mod devpost;
pub mod employers;
pub mod github_internships;
pub mod hacker_news;
pub mod himalayas;
pub mod hn_hiring;
pub mod hn_top_links;
pub mod job_bank;
pub mod jobicy;
pub mod jsonld;
pub mod linkedin;
pub mod luma;
pub mod rss;
pub mod sections;
pub mod tldr;

/// Several sources (notably the Lassonde WordPress feed) 403 anything that
/// doesn't look like a browser.
pub const USER_AGENT: &str = "Mozilla/5.0 (X11; Linux x86_64) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/126.0.0.0 Safari/537.36";

/// Where a scraper hands over rows as it gets them.
pub type Sink<'a> = &'a mut (dyn FnMut(Vec<Item>) + Send);

#[async_trait]
pub trait Scraper: Send + Sync {
    fn source_name(&self) -> &'static str;
    async fn fetch(&self, client: &reqwest::Client) -> Result<Vec<Item>>;

    /// [`fetch`](Scraper::fetch), but delivering the rows in pieces as they
    /// arrive. A source that is one request has one piece and need not
    /// override this; one that is a hundred requests (an ATS family, a paged
    /// listing) should, so the reader can browse the first boards while the
    /// last are still being fetched.
    ///
    /// **A piece is a promise.** Whatever has been handed to `sink` is stored,
    /// so a source must not stream rows it would then fail to stand behind:
    /// an `Err` after some pieces still stores them, but the run is recorded
    /// as failed and none of the source's scopes are treated as seen.
    async fn fetch_batches(&self, client: &reqwest::Client, sink: Sink<'_>) -> Result<()> {
        sink(self.fetch(client).await?);
        Ok(())
    }
}

/// Everything a streaming scraper produces, gathered up: what `fetch` is for a
/// source whose real implementation is `fetch_batches`.
pub async fn collect_batches(scraper: &dyn Scraper, client: &reqwest::Client) -> Result<Vec<Item>> {
    let mut all = Vec::new();
    scraper
        .fetch_batches(client, &mut |batch| all.extend(batch))
        .await?;
    Ok(all)
}

pub fn build_client() -> Result<reqwest::Client> {
    Ok(reqwest::Client::builder()
        .user_agent(USER_AGENT)
        .timeout(Duration::from_secs(20))
        .redirect(reqwest::redirect::Policy::limited(10))
        .build()?)
}

/// What a source is for, as far as *scraping* is concerned.
///
/// Not the same thing as [`ItemType`](crate::models::ItemType). Every employer
/// board emits `Job`, and whether a posting is an internship is a property of
/// its title, found after the fetch — so what a reader can switch off *before*
/// a request is made is a kind of source, and "internships versus jobs" is
/// decided on the rows that come back (see `ScrapePrefs::admits`).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Category {
    News,
    Opportunities,
    Events,
}

impl Category {
    pub fn as_str(self) -> &'static str {
        match self {
            Category::News => "news",
            Category::Opportunities => "opportunities",
            Category::Events => "events",
        }
    }

    /// How long a successful run stays fresh. Each is about how fast the thing
    /// actually changes: a front page in minutes, a company's open roles in
    /// hours, and an event list — which only ever grows towards a date — in
    /// half a day. A manual refresh ignores all of them.
    pub fn ttl(self) -> chrono::Duration {
        match self {
            Category::News => chrono::Duration::minutes(30),
            Category::Opportunities => chrono::Duration::hours(6),
            Category::Events => chrono::Duration::hours(12),
        }
    }
}

/// Every scraper with its category, in the order dedupe prefers them.
pub fn registry() -> Vec<(Category, Box<dyn Scraper>)> {
    use Category::{Events, News, Opportunities};
    vec![
        // News
        (News, Box::new(hacker_news::HackerNews)),
        (News, Box::new(hn_top_links::HnTopLinks)),
        (News, Box::new(rss::PHORONIX)),
        (News, Box::new(rss::LOBSTERS)),
        (News, Box::new(rss::ARS_TECHNICA)),
        (News, Box::new(rss::THE_VERGE)),
        (News, Box::new(rss::INFOQ)),
        (News, Box::new(rss::LASSONDE)),
        (News, Box::new(tldr::TldrTech)),
        (News, Box::new(daily_dev::DailyDev)),
        // Opportunities — aggregators first, then the employers' own boards.
        // The aggregators decide what to carry, which is why the direct
        // boards exist: a CIBC co-op posting is on cibc.wd3 and nowhere else.
        (Opportunities, Box::new(github_internships::PITT_CSC)),
        (Opportunities, Box::new(github_internships::SIMPLIFY)),
        (Opportunities, Box::new(github_internships::NEW_GRAD)),
        (Opportunities, Box::new(job_bank::JOB_BANK)),
        (Opportunities, Box::new(himalayas::Himalayas)),
        (Opportunities, Box::new(jobicy::Jobicy)),
        (Opportunities, Box::new(linkedin::LinkedIn)),
        (Opportunities, Box::new(hn_hiring::HnHiring)),
        (Opportunities, Box::new(ats::workday::WorkdayBoards)),
        (Opportunities, Box::new(ats::greenhouse::GreenhouseBoards)),
        (Opportunities, Box::new(ats::lever::LeverBoards)),
        (Opportunities, Box::new(ats::ashby::AshbyBoards)),
        (
            Opportunities,
            Box::new(ats::smartrecruiters::SmartRecruitersBoards),
        ),
        (Opportunities, Box::new(ats::workable::WorkableBoards)),
        (Opportunities, Box::new(ats::recruitee::RecruiteeBoards)),
        (Opportunities, Box::new(ats::breezy::BreezyBoards)),
        // Events
        (Events, Box::new(luma::Luma)),
        (Events, Box::new(devpost::Devpost)),
    ]
}

pub fn all_scrapers() -> Vec<Box<dyn Scraper>> {
    registry().into_iter().map(|(_, scraper)| scraper).collect()
}

/// Every source and what it is, once each, in registry order. The one list the
/// preferences, the schedule and the health display all work from, so a source
/// cannot be known to one and missing from another.
pub fn sources() -> Vec<(Category, &'static str)> {
    let mut seen = HashSet::new();
    registry()
        .into_iter()
        .map(|(category, scraper)| (category, scraper.source_name()))
        .filter(|(_, name)| seen.insert(*name))
        .collect()
}

/// A source that answered correctly and has nothing current to say — a
/// repository nobody has touched in months, say. Not a failure: the request
/// worked, and asking again in half an hour would find the same thing. It is
/// its own error type so the caller can tell it from a network error by
/// downcasting, and schedule the next run accordingly.
#[derive(Debug)]
pub struct Dormant(pub String);

impl std::fmt::Display for Dormant {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str(&self.0)
    }
}

impl std::error::Error for Dormant {}

/// How one source's run went.
#[derive(Clone, Debug, PartialEq)]
pub enum Health {
    Ok,
    /// Ran fine, found its list abandoned. The note says so.
    Dormant(String),
    Failed(String),
}

/// What one source did in a refresh, for the progress display and the ledger.
#[derive(Clone, Debug, PartialEq)]
pub struct SourceOutcome {
    pub source: &'static str,
    pub category: Category,
    /// Rows it returned, before dedupe.
    pub count: usize,
    pub health: Health,
}

/// How the cache's sweep may reason about the rows a source returns. See
/// [`crate::db::sweep`] for what each answer permits.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Liveness {
    /// An employer board per request. A posting missing from a board that
    /// *did* answer is gone; one missing because its board failed is not, and
    /// the source cannot tell the two apart, so the scope is the employer.
    Boards,
    /// One list that is the whole of what the source carries, so a posting
    /// missing from a list that answered is gone.
    Listing,
    /// Samples a larger set by keyword, window or page limit (Workday, Job
    /// Bank), or carries items that end on a date (Luma, news). A posting
    /// missing from one of these may simply not have been drawn this time, so
    /// absence proves nothing and only the backstop and the dates apply.
    Sampled,
}

/// Keyed on `source_platform`, which is the scraper's `source_name` for every
/// item it produces.
pub fn liveness(source: &str) -> Liveness {
    match source {
        "Greenhouse" | "Lever" | "Ashby" | "SmartRecruiters" | "Workable" | "Recruitee"
        | "Breezy" => Liveness::Boards,
        "Pitt CSC Repo" | "Simplify" | "New Grad Positions" | "Devpost" => Liveness::Listing,
        _ => Liveness::Sampled,
    }
}

/// Sources that read whole employer boards — everything an employer has open,
/// most of it nothing this app is for. These are the rows
/// [`rank::worth_storing`](crate::rank::worth_storing) judges; a list that was
/// curated for interns, or a search for developers, has already chosen.
pub fn reads_whole_boards(source: &str) -> bool {
    matches!(
        source,
        "Greenhouse" | "Lever" | "Ashby" | "SmartRecruiters" | "Workable" | "Recruitee" | "Breezy"
    )
}

/// Sources whose `timestamp` is the day the event *starts*, so it can stand in
/// for an end date. Devpost's is the day it opened, which cannot.
pub const START_DATED: &[&str] = &["Luma"];

/// The scopes this scrape vouches for: those it returned at least one row
/// for, under the policy above. An employer whose board failed returned no
/// rows and so is absent, which is what keeps its old postings from being
/// charged a miss.
pub fn alive_scopes(items: &[Item]) -> HashSet<Scope> {
    let mut alive = HashSet::new();
    for item in items {
        match liveness(&item.source_platform) {
            Liveness::Boards => {
                if let Some(company) = &item.company {
                    alive.insert(Scope {
                        source: item.source_platform.clone(),
                        company: Some(company.clone()),
                    });
                }
            }
            Liveness::Listing => {
                alive.insert(Scope {
                    source: item.source_platform.clone(),
                    company: None,
                });
            }
            Liveness::Sampled => {}
        }
    }
    alive
}

/// Runs every scraper concurrently, then dedupes by URL, backfills the
/// discipline classification, and derives location tags.
pub async fn fetch_all() -> Vec<Item> {
    let names: Vec<&str> = sources().into_iter().map(|(_, name)| name).collect();
    fetch_selected(&names, |_, _, _| {}).await.0
}

/// What a running scrape sends to whoever is listening.
pub enum Msg {
    /// Rows from a source that is still running. Not yet deduped or classified:
    /// run them through [`finish`].
    Batch {
        source: &'static str,
        items: Vec<Item>,
    },
    /// The source has finished, one way or another. After this it sends nothing.
    Done(SourceOutcome),
}

/// A source whose code panicked. Not retried: a parser that tripped over a
/// page will trip over the same page three seconds later.
#[derive(Debug)]
pub struct Panicked(pub String);

impl std::fmt::Display for Panicked {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        write!(f, "panicked: {}", self.0)
    }
}

impl std::error::Error for Panicked {}

/// The text of a caught panic, for the log and the source's health line.
pub fn panic_message(panic: &(dyn std::any::Any + Send)) -> String {
    panic
        .downcast_ref::<&str>()
        .map(|s| s.to_string())
        .or_else(|| panic.downcast_ref::<String>().cloned())
        .unwrap_or_else(|| "unknown panic".to_string())
}

/// The registry order of every source, for the write-time ownership rule.
pub fn source_order() -> &'static [&'static str] {
    static ORDER: std::sync::OnceLock<Vec<&'static str>> = std::sync::OnceLock::new();
    ORDER.get_or_init(|| sources().into_iter().map(|(_, name)| name).collect())
}

/// Runs the named sources concurrently and sends each one's rows as they
/// arrive, then a [`Msg::Done`].
///
/// This is the producer half of a scrape. It owns every in-flight request, so
/// **dropping it is how a scrape is stopped**: the futures go, and reqwest
/// abandons the connections with them. It never writes anything itself, which
/// is what makes that safe — a stop cannot land in the middle of a write.
///
/// A source that fails sends a failed `Done` and nothing else; it never takes
/// the others down. The channel closes when this returns.
pub async fn fetch_stream(names: &[&str], tx: futures::channel::mpsc::UnboundedSender<Msg>) {
    let selected: Vec<(Category, Box<dyn Scraper>)> = registry()
        .into_iter()
        .filter(|(_, scraper)| names.contains(&scraper.source_name()))
        .collect();
    stream_sources(selected, tx).await;
}

/// [`fetch_stream`] over scrapers already chosen — the seam the tests use to
/// put a source that misbehaves among ones that do not.
async fn stream_sources(
    selected: Vec<(Category, Box<dyn Scraper>)>,
    tx: futures::channel::mpsc::UnboundedSender<Msg>,
) {
    let client = match build_client() {
        Ok(client) => client,
        Err(e) => {
            log::error!("could not build HTTP client: {e}");
            for (category, scraper) in &selected {
                let _ = tx.unbounded_send(Msg::Done(SourceOutcome {
                    source: scraper.source_name(),
                    category: *category,
                    count: 0,
                    health: Health::Failed(format!("could not build HTTP client: {e}")),
                }));
            }
            return;
        }
    };

    let mut pending: FuturesUnordered<_> = selected
        .into_iter()
        .map(|(category, scraper)| {
            let client = client.clone();
            let tx = tx.clone();
            async move {
                let source = scraper.source_name();
                let mut count = 0usize;
                let mut result;
                let mut attempt = 0;
                loop {
                    result = {
                        let tx = tx.clone();
                        let mut sink = |items: Vec<Item>| {
                            count += items.len();
                            let _ = tx.unbounded_send(Msg::Batch { source, items });
                        };
                        // A panic in one source's parser — an index past the
                        // end of a page that changed shape — is that source
                        // failing, not the app. Caught here because every
                        // source runs in this one task; uncaught it would
                        // unwind through the whole scrape.
                        AssertUnwindSafe(scraper.fetch_batches(&client, &mut sink))
                            .catch_unwind()
                            .await
                            .unwrap_or_else(|panic| Err(Panicked(panic_message(&panic)).into()))
                    };
                    // A source that failed before it produced anything gets one
                    // more try after a pause: with dozens of requests in flight
                    // a connection can fail for reasons that are gone a moment
                    // later, and the next scheduled attempt is a quarter of an
                    // hour away. A source that already sent rows is not retried —
                    // those rows would arrive twice.
                    let retryable = matches!(&result, Err(e)
                        if e.downcast_ref::<Dormant>().is_none()
                            && e.downcast_ref::<Panicked>().is_none());
                    if retryable && count == 0 && attempt == 0 {
                        attempt += 1;
                        log::info!("{source}: failed, trying once more");
                        tokio::time::sleep(Duration::from_secs(3)).await;
                        continue;
                    }
                    break;
                }
                let health = match result {
                    Ok(()) => {
                        log::info!("{source}: {count} items");
                        Health::Ok
                    }
                    Err(e) => match e.downcast_ref::<Dormant>() {
                        Some(note) => {
                            log::warn!("{source}: {note}");
                            Health::Dormant(note.0.clone())
                        }
                        None => {
                            log::error!("Error scraping {source}: {e}");
                            Health::Failed(e.to_string())
                        }
                    },
                };
                let _ = tx.unbounded_send(Msg::Done(SourceOutcome {
                    source,
                    category,
                    count,
                    health,
                }));
            }
        })
        .collect();
    drop(tx);

    while pending.next().await.is_some() {}
}

/// Runs the named sources concurrently and reports each as it finishes.
///
/// The collecting form of [`fetch_stream`], for the tools and tests that want
/// one deduped list. `on_done` is told the outcome and how many of the
/// selected sources have now finished. The rows are put back into registry
/// order before they are merged, because completion order is arbitrary and
/// dedupe keeps the *first* copy of a URL: it has to keep the same one
/// whichever source happened to answer first.
pub async fn fetch_selected(
    names: &[&str],
    mut on_done: impl FnMut(&SourceOutcome, usize, usize),
) -> (Vec<Item>, Vec<SourceOutcome>) {
    let order = source_order();
    let at = |source: &str| {
        order
            .iter()
            .position(|n| *n == source)
            .unwrap_or(usize::MAX)
    };
    let total = sources()
        .iter()
        .filter(|(_, name)| names.contains(name))
        .count();

    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    let consume = async {
        let mut results: Vec<(usize, Vec<Item>)> = Vec::new();
        let mut outcomes: Vec<SourceOutcome> = Vec::new();
        while let Some(msg) = rx.next().await {
            match msg {
                Msg::Batch { source, items } => results.push((at(source), items)),
                Msg::Done(outcome) => {
                    on_done(&outcome, outcomes.len() + 1, total);
                    outcomes.push(outcome);
                }
            }
        }
        (results, outcomes)
    };
    let (_, (mut results, outcomes)) = futures::join!(fetch_stream(names, tx), consume);

    // A source that failed after streaming some rows has its rows dropped here,
    // as it always did: the collecting form promises a failed source yields
    // nothing.
    let failed: HashSet<usize> = outcomes
        .iter()
        .filter(|o| matches!(o.health, Health::Failed(_)))
        .map(|o| at(o.source))
        .collect();
    results.retain(|(source_at, _)| !failed.contains(source_at));
    results.sort_by_key(|(source_at, _)| *source_at);
    let merged = finish(results.into_iter().flat_map(|(_, items)| items));
    (merged, outcomes)
}

/// Dedupes by URL and fills in what the scrapers leave to be derived.
///
/// The first copy of a URL wins, which is the registry's order — aggregators
/// first, then the employers' own boards. But the first copy is not always the
/// better informed one: the GitHub lists know nothing of a deadline, a pay
/// range or a description, and the employer's board often does. So the copy
/// that is dropped is not discarded, it is read for what the kept one lacks.
pub fn finish(items: impl IntoIterator<Item = Item>) -> Vec<Item> {
    let mut index: HashMap<String, usize> = HashMap::new();
    let mut deduped: Vec<Item> = Vec::new();

    for mut item in items {
        if item.url.is_empty() {
            continue;
        }
        if let Some(&kept) = index.get(&item.url) {
            absorb(&mut deduped[kept], item);
            continue;
        }
        if item.discipline.is_none() {
            item.discipline = Some(classify_item(&item));
        }
        // Backfilled here rather than in each scraper so a source that knows
        // its employer and one that only has "{role} at {company}" in a title
        // both end up with the same canonical name — otherwise the employer
        // filter shows "Shopify" and "Shopify Inc." as two companies.
        if item.company.is_none() {
            item.company = company_from_title(&item.title);
        }
        item.company = item.company.as_deref().map(companies::canonical);
        if item.seniority.is_none() {
            item.seniority = classify_seniority(&item.title);
        }
        item.location_tags = normalize_location(item.location.as_deref());
        index.insert(item.url.clone(), deduped.len());
        deduped.push(item);
    }

    deduped
}

/// Copies onto `kept` the facts it lacks from a later copy of the same posting.
/// Only ever fills a gap — a value the kept copy has is never overwritten, so
/// which source "wins" a field is still decided by registry order.
fn absorb(kept: &mut Item, dup: Item) {
    if kept.closes_at.is_none() {
        kept.closes_at = dup.closes_at;
    }
    if kept.simplify_id.is_none() {
        kept.simplify_id = dup.simplify_id;
    }
    if kept.company.is_none() {
        kept.company = dup.company.as_deref().map(companies::canonical);
    }
    if kept.salary_min.is_none() && kept.salary_max.is_none() {
        kept.salary_min = dup.salary_min;
        kept.salary_max = dup.salary_max;
        kept.salary_currency = dup.salary_currency;
        kept.salary_period = dup.salary_period;
    }
    // The posting text travels as a set: a description from one copy with the
    // requirements of another would be a posting nobody wrote. `persist`
    // turns text that arrives on a row into a `job_details` entry, which is a
    // request the enrichment pass no longer has to make.
    let kept_has_text = kept.description.is_some()
        || kept.requirements.is_some()
        || kept.responsibilities.is_some()
        || kept.perks.is_some();
    if !kept_has_text {
        kept.description = dup.description;
        kept.requirements = dup.requirements;
        kept.responsibilities = dup.responsibilities;
        kept.perks = dup.perks;
        kept.tagged_skills = dup.tagged_skills;
    }
}

/// Sources that re-list postings employers publish on boards of their own.
/// A LinkedIn copy of a Greenhouse posting has a different URL, so the
/// URL-keyed dedupe above cannot see that it is the same job.
pub const MIRRORS: &[&str] = &[linkedin::NAME, jobicy::NAME];

/// A title reduced to its words, for telling that two sources list the same
/// role: "Software Engineer, Intern (Summer)" and "Software Engineer - Intern
/// (Summer)" are one posting.
fn title_key(title: &str) -> String {
    let spaced: String = title
        .to_lowercase()
        .chars()
        .map(|c| if c.is_alphanumeric() { c } else { ' ' })
        .collect();
    collapse_ws(&spaced)
}

/// Drops the rows a [`MIRRORS`] source re-lists from somewhere else: same
/// employer, same title, as a row from any other source. The employer's own
/// copy is the one kept — it is where the application goes, and the copy the
/// description handlers read best. Two mirrors listing the same title are both
/// kept: a role posted in Toronto and in Calgary is two cards on LinkedIn too.
///
/// Applied on read rather than at write time, because the copies arrive in
/// either order and either may be swept later; the mirror row stays in the
/// cache and comes back on its own when the original goes. A row the reader
/// saved is never hidden.
///
/// One pass over the cache, and cheap where most of it is: only rows at an
/// employer that some mirror row names build a title key, and the employer is
/// compared as stored, since every row's company went through
/// `companies::canonical` in [`finish`].
pub fn hide_mirrored(items: &mut Vec<Item>) {
    let is_mirror = |item: &Item| MIRRORS.contains(&item.source_platform.as_str());
    let mirror_companies: HashSet<&str> = items
        .iter()
        .filter(|i| is_mirror(i))
        .filter_map(|i| i.company.as_deref())
        .collect();
    if mirror_companies.is_empty() {
        return;
    }
    let originals: HashSet<(&str, String)> = items
        .iter()
        .filter(|i| !is_mirror(i) && i.item_type.is_opportunity())
        .filter_map(|i| {
            let company = i.company.as_deref()?;
            mirror_companies
                .contains(company)
                .then(|| (company, title_key(&i.title)))
        })
        .collect();
    if originals.is_empty() {
        return;
    }
    let hidden: Vec<bool> = items
        .iter()
        .map(|i| {
            is_mirror(i)
                && !i.saved
                && i.company
                    .as_deref()
                    .is_some_and(|c| originals.contains(&(c, title_key(&i.title))))
        })
        .collect();
    let mut at = 0;
    items.retain(|_| {
        let keep = !hidden[at];
        at += 1;
        keep
    });
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

/// Caps a string at `max` bytes without splitting a word or a UTF-8 sequence.
/// Job descriptions routinely run past 20 KB, most of it company boilerplate.
pub fn truncate_words(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    // `floor_char_boundary` is still unstable, so walk back to one by hand.
    let mut cut = max;
    while cut > 0 && !s.is_char_boundary(cut) {
        cut -= 1;
    }
    let head = &s[..cut];
    match head.rfind(char::is_whitespace) {
        Some(space) => head[..space].trim_end().to_string(),
        None => head.to_string(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    struct Fine;
    struct Panics;

    #[async_trait]
    impl Scraper for Fine {
        fn source_name(&self) -> &'static str {
            "Fine"
        }
        async fn fetch(&self, _: &reqwest::Client) -> Result<Vec<Item>> {
            Ok(vec![Item::new(
                "A role",
                "Fine",
                ItemType::Job,
                "https://fine.test/1",
            )])
        }
    }

    #[async_trait]
    impl Scraper for Panics {
        fn source_name(&self) -> &'static str {
            "Panics"
        }
        async fn fetch(&self, _: &reqwest::Client) -> Result<Vec<Item>> {
            let rows: Vec<&str> = Vec::new();
            // What a parser does when a page changes shape under it.
            let _ = rows[3];
            unreachable!()
        }
    }

    #[tokio::test]
    async fn a_source_that_panics_fails_alone() {
        let (tx, mut rx) = futures::channel::mpsc::unbounded();
        stream_sources(
            vec![
                (
                    Category::Opportunities,
                    Box::new(Panics) as Box<dyn Scraper>,
                ),
                (Category::Opportunities, Box::new(Fine) as Box<dyn Scraper>),
            ],
            tx,
        )
        .await;

        let mut rows = 0;
        let mut outcomes = Vec::new();
        while let Some(msg) = rx.next().await {
            match msg {
                Msg::Batch { items, .. } => rows += items.len(),
                Msg::Done(outcome) => outcomes.push(outcome),
            }
        }
        assert_eq!(rows, 1, "the healthy source's rows still arrive");
        assert_eq!(
            outcomes.len(),
            2,
            "both sources report, the panicking one included"
        );
        let panicked = outcomes.iter().find(|o| o.source == "Panics").unwrap();
        assert!(
            matches!(&panicked.health, Health::Failed(why) if why.contains("panicked")),
            "{:?}",
            panicked.health
        );
    }

    #[test]
    fn truncation_leaves_short_strings_alone() {
        assert_eq!(truncate_words("short", 100), "short");
    }

    #[test]
    fn truncation_cuts_on_a_word_boundary() {
        assert_eq!(truncate_words("alpha beta gamma", 12), "alpha beta");
    }

    use crate::models::ItemType;
    use chrono::{Duration as Days, Utc};

    fn posting(source: &str, company: &str, url: &str) -> Item {
        Item::new("Engineer", source, ItemType::Job, url).with_company(company)
    }

    /// The mechanism a Stop relies on: aborting the producer abandons whatever
    /// it was still waiting for, closes the channel behind it, and leaves every
    /// message it had already sent for the consumer to drain.
    #[test]
    fn stopping_the_producer_keeps_what_it_sent_and_ends_the_stream() {
        use futures::future::{AbortHandle, Abortable};
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (tx, mut rx) = futures::channel::mpsc::unbounded::<Msg>();
        let (handle, registration) = AbortHandle::new_pair();

        let producer = Abortable::new(
            async move {
                let _ = tx.unbounded_send(Msg::Batch {
                    source: "Greenhouse",
                    items: vec![posting("Greenhouse", "Acme", "https://acme.test/1")],
                });
                // A board still waiting on the network.
                futures::future::pending::<()>().await;
            },
            registration,
        );
        let consumer = async {
            let mut seen = 0;
            while let Some(msg) = rx.next().await {
                if let Msg::Batch { items, .. } = msg {
                    seen += items.len();
                    // The reader presses Stop once something has arrived.
                    handle.abort();
                }
            }
            seen
        };
        let (aborted, seen) = runtime.block_on(async { futures::join!(producer, consumer) });
        assert!(aborted.is_err(), "the producer was cut short");
        assert_eq!(seen, 1, "what arrived before the stop is kept");
    }

    #[test]
    fn a_dropped_duplicate_still_lends_its_deadline_and_pay() {
        let closes = Utc::now() + Days::days(5);
        let github = posting("Simplify", "Acme", "https://acme.test/1");
        let mut board =
            posting("Greenhouse", "Acme", "https://acme.test/1").with_closes_at(Some(closes));
        board.salary_min = Some(90_000.0);
        board.salary_max = Some(120_000.0);
        board.salary_period = Some("year".into());

        let merged = finish([github, board]);

        assert_eq!(merged.len(), 1);
        assert_eq!(
            merged[0].source_platform, "Simplify",
            "registry order still decides the source"
        );
        assert_eq!(merged[0].closes_at, Some(closes));
        assert_eq!(merged[0].salary_max, Some(120_000.0));
    }

    #[test]
    fn a_copy_never_overwrites_what_the_kept_one_has() {
        let first = Utc::now() + Days::days(2);
        let later = Utc::now() + Days::days(9);
        let a = posting("Simplify", "Acme", "https://acme.test/2").with_closes_at(Some(first));
        let b = posting("Greenhouse", "Acme", "https://acme.test/2").with_closes_at(Some(later));
        assert_eq!(finish([a, b])[0].closes_at, Some(first));
    }

    #[test]
    fn posting_text_is_taken_whole_or_not_at_all() {
        let bare = posting("Simplify", "Acme", "https://acme.test/3");
        let mut full = posting("Lever", "Acme", "https://acme.test/3");
        full.description = Some("About the role".into());
        full.requirements = Some("Rust".into());
        let merged = finish([bare, full]);
        assert_eq!(merged[0].description.as_deref(), Some("About the role"));
        assert_eq!(merged[0].requirements.as_deref(), Some("Rust"));

        // A kept copy that already has text keeps all of its own.
        let mut own = posting("Simplify", "Acme", "https://acme.test/4");
        own.requirements = Some("Go".into());
        let mut other = posting("Lever", "Acme", "https://acme.test/4");
        other.description = Some("Elsewhere".into());
        other.requirements = Some("Rust".into());
        let merged = finish([own, other]);
        assert_eq!(merged[0].description, None);
        assert_eq!(merged[0].requirements.as_deref(), Some("Go"));
    }

    #[test]
    fn rows_without_a_url_are_dropped_and_the_rest_are_derived() {
        let merged = finish([
            posting("Greenhouse", "Acme Inc.", ""),
            posting("Greenhouse", "Acme Inc.", "https://acme.test/5"),
        ]);
        assert_eq!(merged.len(), 1);
        assert!(merged[0].discipline.is_some());
    }

    #[test]
    fn a_mirror_copy_of_an_employers_posting_is_hidden() {
        let titled = |title: &str, source: &str, url: &str| {
            Item::new(title, source, ItemType::Job, url).with_company("Acme")
        };
        let mut items = vec![
            titled(
                "Software Engineer - Intern (Summer)",
                "LinkedIn",
                "https://www.linkedin.com/jobs/view/1",
            ),
            titled(
                "Software Engineer, Intern (Summer)",
                "Greenhouse",
                "https://acme.test/1",
            ),
            // Same title, other employer: a different job.
            Item::new(
                "Software Engineer, Intern (Summer)",
                "LinkedIn",
                ItemType::Job,
                "https://www.linkedin.com/jobs/view/2",
            )
            .with_company("Initech"),
            // Only on LinkedIn.
            titled(
                "Backend Developer",
                "LinkedIn",
                "https://www.linkedin.com/jobs/view/3",
            ),
        ];
        hide_mirrored(&mut items);
        let urls: Vec<&str> = items.iter().map(|i| i.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://acme.test/1",
                "https://www.linkedin.com/jobs/view/2",
                "https://www.linkedin.com/jobs/view/3",
            ]
        );
    }

    #[test]
    fn mirrors_do_not_hide_each_other_or_a_saved_row() {
        let mut a = Item::new(
            "Software Engineer",
            "LinkedIn",
            ItemType::Job,
            "https://www.linkedin.com/jobs/view/1",
        )
        .with_company("Acme");
        let b = Item::new(
            "Software Engineer",
            "LinkedIn",
            ItemType::Job,
            "https://www.linkedin.com/jobs/view/2",
        )
        .with_company("Acme");
        let mut items = vec![a.clone(), b.clone()];
        hide_mirrored(&mut items);
        assert_eq!(items.len(), 2, "a role posted in two cities is two cards");

        a.saved = true;
        let original = Item::new(
            "Software Engineer",
            "Lever",
            ItemType::Job,
            "https://acme.test/1",
        )
        .with_company("Acme");
        let mut items = vec![a, b, original];
        hide_mirrored(&mut items);
        let urls: Vec<&str> = items.iter().map(|i| i.url.as_str()).collect();
        assert_eq!(
            urls,
            [
                "https://www.linkedin.com/jobs/view/1",
                "https://acme.test/1"
            ],
            "the saved copy stays, the other mirror goes"
        );
    }

    #[test]
    fn only_employers_that_answered_are_alive() {
        let items = [
            posting("Greenhouse", "Acme", "https://acme.test/a"),
            posting("Greenhouse", "Acme", "https://acme.test/b"),
            posting("Simplify", "Anyone", "https://any.test/1"),
            posting("Workday", "Bank", "https://bank.test/1"),
            Item::new(
                "Story",
                "Hacker News",
                ItemType::Article,
                "https://news.test/1",
            ),
        ];
        let alive = alive_scopes(&items);

        let board = |s: &str, c: &str| Scope {
            source: s.into(),
            company: Some(c.into()),
        };
        assert!(alive.contains(&board("Greenhouse", "Acme")));
        assert!(
            !alive.contains(&board("Greenhouse", "Initech")),
            "a board that returned nothing must not be able to accuse its rows"
        );
        assert!(alive.contains(&Scope {
            source: "Simplify".into(),
            company: None
        }));
        assert_eq!(alive.len(), 2, "sampled sources and news vouch for nothing");
    }

    /// The sweep's behaviour hangs on these strings matching what the scrapers
    /// stamp onto `source_platform`, and a typo here fails silently.
    #[test]
    fn the_policy_table_names_real_sources() {
        let registered: HashSet<&str> = all_scrapers().iter().map(|s| s.source_name()).collect();
        for name in [
            "Greenhouse",
            "Lever",
            "Ashby",
            "SmartRecruiters",
            "Workable",
            "Recruitee",
            "Breezy",
            "Pitt CSC Repo",
            "Simplify",
            "New Grad Positions",
            "Devpost",
        ] {
            assert!(
                registered.contains(name),
                "{name} is not a registered source"
            );
            assert_ne!(liveness(name), Liveness::Sampled);
        }
        // Searches and samples: absence from one proves nothing.
        for name in [
            "Himalayas",
            "HN Who's Hiring",
            "Job Bank Canada",
            "Workday",
            "LinkedIn",
            "Jobicy",
        ] {
            assert!(
                registered.contains(name),
                "{name} is not a registered source"
            );
            assert_eq!(liveness(name), Liveness::Sampled, "{name}");
        }
        for name in START_DATED {
            assert!(
                registered.contains(name),
                "{name} is not a registered source"
            );
        }
    }

    #[test]
    fn every_source_appears_once_with_one_category() {
        let all = sources();
        let names: HashSet<&str> = all.iter().map(|(_, name)| *name).collect();
        assert_eq!(names.len(), all.len(), "a source is listed twice");

        let category = |name: &str| all.iter().find(|(_, n)| *n == name).map(|(c, _)| *c);
        assert_eq!(category("Hacker News"), Some(Category::News));
        assert_eq!(category("Greenhouse"), Some(Category::Opportunities));
        assert_eq!(category("Simplify"), Some(Category::Opportunities));
        assert_eq!(category("Luma"), Some(Category::Events));
        assert_eq!(category("Devpost"), Some(Category::Events));
        // And the wrapper the older callers use still sees every scraper.
        assert_eq!(all_scrapers().len(), registry().len());
    }

    /// The category names cross the IPC boundary and are stored in the run
    /// ledger, so renaming one is a data migration, not a refactor.
    #[test]
    fn category_names_are_stable() {
        assert_eq!(serde_json::to_string(&Category::News).unwrap(), "\"news\"");
        assert_eq!(
            serde_json::to_string(&Category::Opportunities).unwrap(),
            "\"opportunities\""
        );
        assert_eq!(
            serde_json::to_string(&Category::Events).unwrap(),
            "\"events\""
        );
        for category in [Category::News, Category::Opportunities, Category::Events] {
            let json = serde_json::to_string(&category).unwrap();
            assert_eq!(json, format!("\"{}\"", category.as_str()));
        }
    }

    #[test]
    fn a_dormant_note_is_told_apart_from_a_failure_by_its_type() {
        let dormant: anyhow::Error = Dormant("nothing updated for 74 days".into()).into();
        assert!(dormant.downcast_ref::<Dormant>().is_some());
        let failed = anyhow::anyhow!("connection reset");
        assert!(failed.downcast_ref::<Dormant>().is_none());
    }

    #[test]
    fn truncation_never_splits_a_utf8_sequence() {
        // Each "é" is two bytes, so a naive slice at 5 would panic.
        let s = "éééééééé";
        let out = truncate_words(s, 5);
        assert!(s.starts_with(&out) || out.is_empty());
    }
}
