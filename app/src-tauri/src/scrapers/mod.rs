//! The scraping pipeline. Every source implements [`Scraper`]; they run
//! concurrently and a failing source logs and yields nothing rather than
//! taking the whole refresh down — the same degradation contract the Python
//! pipeline had, where each scraper wrapped its body in try/except.

use anyhow::Result;
use async_trait::async_trait;
use futures::stream::{FuturesUnordered, StreamExt};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::time::Duration;

use crate::companies;
use crate::db::Scope;
use crate::location::normalize_location;
use crate::models::{company_from_title, Item};
use crate::rank::{classify_item, classify_seniority};

pub mod ats;
pub mod daily_dev;
pub mod details;
pub mod devpost;
pub mod employers;
pub mod github_internships;
pub mod hacker_news;
pub mod hn_top_links;
pub mod job_bank;
pub mod jsonld;
pub mod luma;
pub mod rss;
pub mod sections;
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
        (Opportunities, Box::new(ats::workday::WorkdayBoards)),
        (Opportunities, Box::new(ats::greenhouse::GreenhouseBoards)),
        (Opportunities, Box::new(ats::lever::LeverBoards)),
        (Opportunities, Box::new(ats::ashby::AshbyBoards)),
        (
            Opportunities,
            Box::new(ats::smartrecruiters::SmartRecruitersBoards),
        ),
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
        "Greenhouse" | "Lever" | "Ashby" | "SmartRecruiters" => Liveness::Boards,
        "Pitt CSC Repo" | "Simplify" | "New Grad Positions" | "Devpost" => Liveness::Listing,
        _ => Liveness::Sampled,
    }
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

/// Runs the named sources concurrently and reports each as it finishes.
///
/// `on_done` is told the outcome and how many of the selected sources have now
/// finished, which is what lets a first scrape show a list filling in rather
/// than a spinner for the half minute the slowest board takes. The results are
/// put back into registry order before they are merged, because completion
/// order is arbitrary and dedupe keeps the *first* copy of a URL: it has to
/// keep the same one whichever source happened to answer first.
///
/// A source that fails yields nothing and is reported as failed; it never
/// takes the others down.
pub async fn fetch_selected(
    names: &[&str],
    mut on_done: impl FnMut(&SourceOutcome, usize, usize),
) -> (Vec<Item>, Vec<SourceOutcome>) {
    let selected: Vec<(usize, Category, Box<dyn Scraper>)> = registry()
        .into_iter()
        .enumerate()
        .filter(|(_, (_, scraper))| names.contains(&scraper.source_name()))
        .map(|(at, (category, scraper))| (at, category, scraper))
        .collect();
    let total = selected.len();

    let client = match build_client() {
        Ok(client) => client,
        Err(e) => {
            log::error!("could not build HTTP client: {e}");
            let outcomes = selected
                .iter()
                .map(|(_, category, scraper)| SourceOutcome {
                    source: scraper.source_name(),
                    category: *category,
                    count: 0,
                    health: Health::Failed(format!("could not build HTTP client: {e}")),
                })
                .collect();
            return (Vec::new(), outcomes);
        }
    };

    let mut pending: FuturesUnordered<_> = selected
        .into_iter()
        .map(|(at, category, scraper)| {
            let client = client.clone();
            async move {
                let source = scraper.source_name();
                (at, category, source, scraper.fetch(&client).await)
            }
        })
        .collect();

    let mut results: Vec<(usize, Vec<Item>)> = Vec::with_capacity(total);
    let mut outcomes: Vec<SourceOutcome> = Vec::with_capacity(total);
    while let Some((at, category, source, result)) = pending.next().await {
        let (items, health) = match result {
            Ok(items) => {
                log::info!("{source}: {} items", items.len());
                (items, Health::Ok)
            }
            Err(e) => match e.downcast_ref::<Dormant>() {
                Some(note) => {
                    log::warn!("{source}: {note}");
                    (Vec::new(), Health::Dormant(note.0.clone()))
                }
                None => {
                    log::error!("Error scraping {source}: {e}");
                    (Vec::new(), Health::Failed(e.to_string()))
                }
            },
        };
        let outcome = SourceOutcome {
            source,
            category,
            count: items.len(),
            health,
        };
        on_done(&outcome, outcomes.len() + 1, total);
        outcomes.push(outcome);
        results.push((at, items));
    }

    results.sort_by_key(|(at, _)| *at);
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
fn finish(items: impl IntoIterator<Item = Item>) -> Vec<Item> {
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
