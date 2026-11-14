//! Which employer boards the scrapers read: the curated set, a bundled list
//! of probed boards, and boards learned from postings already in the cache.
//!
//! `employers::EMPLOYERS` is the hand-checked core and carries prestige tiers
//! (`companies::TIERS`). It is 92 rows. A probe of the boards named by the
//! URLs inside Simplify's own lists found nearly a thousand more that answer
//! and hold real postings, and an employer that posts one internship through
//! Simplify has a board that posts forty others. Those are `boards.json`,
//! loaded once. They score zero on prestige until a tier names them, which is
//! the honest answer for an employer nobody ranked.
//!
//! Boards are also **discovered**: every posting URL in the cache names its
//! ATS and its board, so a refresh reads them back out ([`discover`]) and the
//! next one fetches boards nobody listed. And boards **age out**: one that has
//! been *definitively* gone (a 404 or 410, or an empty board) on three days
//! running is skipped for a week ([`BoardHealth`]), so a dead slug stops costing
//! a request per refresh. Only definitive answers count: a timeout, a 429 or a
//! 502 says nothing about the board, and an offline laptop must not retire the
//! whole list.
//!
//! The scrapers want `&'static Employer`, and a board learned at runtime has
//! no static to point at, so those are leaked — once each, interned by key.
//! A few thousand small strings for the life of the process.

use chrono::{DateTime, Duration, Utc};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::sync::{Mutex, OnceLock, RwLock};

use super::employers::{Board, Employer, EMPLOYERS};
use crate::sync::{LockExt, RwLockExt};

/// A board named in the bundled list or found in the cache.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct Found {
    pub ats: String,
    pub slug: String,
    pub name: String,
}

/// How a board has been doing. Persisted in `settings`, so a dead board stays
/// skipped across launches.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct BoardHealth {
    /// Strikes in a row: days on which the board was definitively gone or
    /// empty. Reset by a run that returns rows.
    pub fails: u32,
    pub last_try: Option<DateTime<Utc>>,
    /// When the last strike was counted. Strikes are spaced a day apart, as the
    /// sweep spaces its misses, so a burst of manual refreshes is one strike.
    #[serde(default)]
    pub last_strike: Option<DateTime<Utc>>,
    pub last_count: usize,
}

/// What a board said when asked.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// It listed this many postings.
    Rows(usize),
    /// It answered, with nothing on it.
    Empty,
    /// It does not exist (404 or 410).
    Gone,
    /// It did not answer in a way that says anything: a timeout, a 429, a 5xx,
    /// a body that would not parse. Never counts against the board.
    Unclear,
}

impl Answer {
    /// Reads the answer out of how a board fetch ended.
    pub fn of(result: &anyhow::Result<Vec<crate::models::Item>>) -> Answer {
        match result {
            Ok(items) if items.is_empty() => Answer::Empty,
            Ok(items) => Answer::Rows(items.len()),
            Err(e) => match e.downcast_ref::<reqwest::Error>().and_then(|e| e.status()) {
                Some(code) if code == 404 || code == 410 => Answer::Gone,
                _ => Answer::Unclear,
            },
        }
    }
}

/// One strike per this long.
const STRIKE_SPACING_HOURS: i64 = 24;

/// Failures in a row before a board is skipped.
pub const FAILS_BEFORE_SKIP: u32 = 3;
/// How long a board that has given up is left alone.
pub const SKIP_FOR_DAYS: i64 = 7;
/// Discovered boards kept. A cap on the table, not on what is fetched.
pub const MAX_DISCOVERED: usize = 4000;

pub const KEY_DISCOVERED: &str = "boards_discovered";
pub const KEY_HEALTH: &str = "boards_health";

/// A stable name for a board, for health records and de-duplication. `None`
/// for Workday, whose boards are curated and whose scraper has its own paging.
pub fn key(board: &Board) -> Option<String> {
    let (ats, slug) = match board {
        Board::Greenhouse(s) => ("greenhouse", *s),
        Board::Lever(s) => ("lever", *s),
        Board::Ashby(s) => ("ashby", *s),
        Board::SmartRecruiters(s) => ("smartrecruiters", *s),
        Board::Workable(s) => ("workable", *s),
        Board::Recruitee(s) => ("recruitee", *s),
        Board::Breezy(s) => ("breezy", *s),
        Board::Workday { .. } => return None,
    };
    Some(format!("{ats}:{}", slug.to_ascii_lowercase()))
}

fn leak(s: &str) -> &'static str {
    Box::leak(s.to_string().into_boxed_str())
}

/// Builds a board from a kind and a slug, or `None` for a kind we do not read.
fn board_of(ats: &str, slug: &'static str) -> Option<Board> {
    Some(match ats {
        "greenhouse" => Board::Greenhouse(slug),
        "lever" => Board::Lever(slug),
        "ashby" => Board::Ashby(slug),
        "smartrecruiters" => Board::SmartRecruiters(slug),
        "workable" => Board::Workable(slug),
        "recruitee" => Board::Recruitee(slug),
        "breezy" => Board::Breezy(slug),
        _ => return None,
    })
}

fn employer_of(found: &Found) -> Option<Employer> {
    let slug = leak(&found.slug);
    let board = board_of(&found.ats, slug)?;
    Some(Employer {
        name: leak(&found.name),
        board,
    })
}

/// The bundled list, parsed once. A malformed file yields nothing rather than
/// a panic: the curated boards still work.
fn bundled() -> &'static [Employer] {
    static BUNDLED: OnceLock<Vec<Employer>> = OnceLock::new();
    BUNDLED.get_or_init(|| {
        let found: Vec<Found> =
            serde_json::from_str(include_str!("boards.json")).unwrap_or_else(|e| {
                log::error!("boards.json is malformed: {e}");
                Vec::new()
            });
        found.iter().filter_map(employer_of).collect()
    })
}

/// Boards learned from the cache, interned by key so each is leaked once.
fn learned() -> &'static RwLock<Vec<&'static Employer>> {
    static LEARNED: OnceLock<RwLock<Vec<&'static Employer>>> = OnceLock::new();
    LEARNED.get_or_init(|| RwLock::new(Vec::new()))
}

fn interned() -> &'static Mutex<HashMap<String, &'static Employer>> {
    static INTERNED: OnceLock<Mutex<HashMap<String, &'static Employer>>> = OnceLock::new();
    INTERNED.get_or_init(|| Mutex::new(HashMap::new()))
}

fn skipped() -> &'static RwLock<HashSet<String>> {
    static SKIPPED: OnceLock<RwLock<HashSet<String>>> = OnceLock::new();
    SKIPPED.get_or_init(|| RwLock::new(HashSet::new()))
}

/// Serialises the tests that go through the report queue below.
///
/// The queue is process-wide and tests run in parallel, so one test's
/// `record_reports` could drain a `Gone` another had just pushed — which is
/// how `an_offline_laptop_retires_no_board` failed in CI while passing
/// locally. Every test that pushes or drains (the `run` helper here,
/// `commands::finalize` in the command tests) holds this first.
#[cfg(test)]
pub(crate) fn test_lock() -> std::sync::MutexGuard<'static, ()> {
    static LOCK: Mutex<()> = Mutex::new(());
    LOCK.lock_recover()
}

fn reports() -> &'static Mutex<Vec<(String, Answer)>> {
    static REPORTS: OnceLock<Mutex<Vec<(String, Answer)>>> = OnceLock::new();
    REPORTS.get_or_init(|| Mutex::new(Vec::new()))
}

/// Replaces the learned boards. Called with what `settings` holds, before a
/// scrape.
pub fn set_learned(found: &[Found]) {
    let mut interned = interned().lock_recover();
    let mut boards = Vec::with_capacity(found.len());
    for f in found {
        let id = format!("{}:{}", f.ats, f.slug.to_ascii_lowercase());
        if let Some(existing) = interned.get(&id) {
            boards.push(*existing);
        } else if let Some(employer) = employer_of(f) {
            let leaked: &'static Employer = Box::leak(Box::new(employer));
            interned.insert(id, leaked);
            boards.push(leaked);
        }
    }
    *learned().write_recover() = boards;
}

/// Replaces the set of boards to leave alone this run.
pub fn set_skipped(health: &HashMap<String, BoardHealth>, now: DateTime<Utc>) {
    let skip = health
        .iter()
        .filter(|(_, h)| is_skipped(h, now))
        .map(|(key, _)| key.clone())
        .collect();
    *skipped().write_recover() = skip;
}

/// Whether a board has failed enough, recently enough, to be left alone.
pub fn is_skipped(health: &BoardHealth, now: DateTime<Utc>) -> bool {
    health.fails >= FAILS_BEFORE_SKIP
        && health
            .last_strike
            .or(health.last_try)
            .is_some_and(|at| now - at < Duration::days(SKIP_FOR_DAYS))
}

/// Every board to read, the curated first. Curated and bundled and learned
/// boards that name the same employer are read once, and a board that has
/// given up is left out.
pub fn all() -> Vec<&'static Employer> {
    let skip = skipped().read_recover();
    let learned = learned().read_recover();
    let mut seen: HashSet<String> = HashSet::new();
    let mut out = Vec::new();
    let candidates = EMPLOYERS
        .iter()
        .chain(bundled().iter())
        .chain(learned.iter().copied());
    for employer in candidates {
        // Workday has no key: curated only, so never duplicated.
        if let Some(key) = key(&employer.board) {
            if skip.contains(&key) || !seen.insert(key) {
                continue;
            }
        }
        out.push(employer);
    }
    out
}

/// A board finished.
pub fn report(board: &Board, answer: Answer) {
    if let Some(key) = key(board) {
        reports().lock_recover().push((key, answer));
    }
}

/// Forgets what boards have reported without filing it — for a run whose
/// answers cannot be trusted (every source failed: the network, not the boards).
pub fn discard_reports() {
    reports().lock_recover().clear();
}

/// Folds what boards reported since the last call into their health records.
/// Rows clear the board's strikes. A definitive no (gone, or empty) is a strike,
/// at most one a day. Anything unclear leaves the record alone but for the time
/// it was tried.
pub fn record_reports(health: &mut HashMap<String, BoardHealth>, now: DateTime<Utc>) -> usize {
    let drained: Vec<_> = std::mem::take(&mut *reports().lock_recover());
    let n = drained.len();
    for (key, answer) in drained {
        let entry = health.entry(key).or_default();
        entry.last_try = Some(now);
        match answer {
            Answer::Rows(count) => {
                entry.fails = 0;
                entry.last_strike = None;
                entry.last_count = count;
            }
            Answer::Empty | Answer::Gone => {
                entry.last_count = 0;
                let spaced = entry
                    .last_strike
                    .is_none_or(|at| now - at >= Duration::hours(STRIKE_SPACING_HOURS));
                if spaced {
                    entry.fails += 1;
                    entry.last_strike = Some(now);
                }
            }
            Answer::Unclear => {}
        }
    }
    n
}

/// The key of every board already read, built once.
fn known_keys() -> HashSet<String> {
    let learned = learned().read_recover();
    EMPLOYERS
        .iter()
        .chain(bundled().iter())
        .chain(learned.iter().copied())
        .filter_map(|e| key(&e.board))
        .collect()
}

/// Reads the board out of a posting URL, when the URL names one.
///
/// Every ATS's job link carries its board: `boards.greenhouse.io/{slug}/jobs/…`,
/// `jobs.lever.co/{slug}/…`, `jobs.ashbyhq.com/{slug}/…`,
/// `jobs.smartrecruiters.com/{company}/…`, `apply.workable.com/{slug}/j/…`.
pub fn board_in_url(url: &str) -> Option<(&'static str, String)> {
    let rest = url
        .strip_prefix("https://")
        .or_else(|| url.strip_prefix("http://"))?;
    let (host, path) = rest.split_once('/')?;
    let first = path.split(['/', '?', '#']).next().unwrap_or("");
    if first.is_empty() {
        return None;
    }
    let host = host.to_ascii_lowercase();
    let ats = if host == "boards.greenhouse.io" || host == "job-boards.greenhouse.io" {
        // `/embed/job_app?for=slug` and `/jobs/123` carry no board in the path.
        if matches!(first, "embed" | "jobs" | "v1") {
            return None;
        }
        "greenhouse"
    } else if host == "jobs.lever.co" {
        "lever"
    } else if host == "jobs.ashbyhq.com" {
        "ashby"
    } else if host == "jobs.smartrecruiters.com" || host == "careers.smartrecruiters.com" {
        "smartrecruiters"
    } else if host == "apply.workable.com" {
        if matches!(first, "j" | "api") {
            return None;
        }
        "workable"
    } else {
        return None;
    };
    // A slug is a plain token; anything else is a path we misread.
    first
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '-' || c == '_' || c == '.')
        .then(|| (ats, first.to_string()))
}

/// "acme-corp" → "Acme Corp". All the name a board found in a URL has.
pub fn pretty(slug: &str) -> String {
    let spaced: String = slug
        .chars()
        .map(|c| if c == '-' || c == '_' { ' ' } else { c })
        .collect();
    let spaced = spaced.trim();
    if spaced.chars().any(|c| c.is_ascii_uppercase()) {
        return spaced.to_string();
    }
    spaced
        .split(' ')
        .filter(|w| !w.is_empty())
        .map(|w| {
            let mut chars = w.chars();
            match chars.next() {
                Some(first) => first.to_uppercase().collect::<String>() + chars.as_str(),
                None => String::new(),
            }
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// The boards named by `urls` that we do not already read, deduped. `existing`
/// is what is already learned, so the list only grows by what is new.
pub fn discover<'a>(urls: impl IntoIterator<Item = &'a str>, existing: &[Found]) -> Vec<Found> {
    let mut have = known_keys();
    have.extend(
        existing
            .iter()
            .map(|f| format!("{}:{}", f.ats, f.slug.to_ascii_lowercase())),
    );
    let mut new = Vec::new();
    for url in urls {
        let Some((ats, slug)) = board_in_url(url) else {
            continue;
        };
        if !have.insert(format!("{ats}:{}", slug.to_ascii_lowercase())) {
            continue;
        }
        new.push(Found {
            ats: ats.to_string(),
            name: pretty(&slug),
            slug,
        });
    }
    new
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_bundled_list_loads_and_is_large() {
        let list = bundled();
        assert!(list.len() > 900, "only {} bundled boards", list.len());
        assert!(list
            .iter()
            .all(|e| !e.name.is_empty() && key(&e.board).is_some()));
    }

    #[test]
    fn curated_boards_win_over_the_same_board_bundled() {
        let all = all();
        let mut keys = HashSet::new();
        for e in &all {
            if let Some(k) = key(&e.board) {
                assert!(keys.insert(k.clone()), "{k} listed twice");
            }
        }
        assert!(all.len() > EMPLOYERS.len() + 800);
        // The first rows are the curated ones, in their own order.
        assert_eq!(all[0].name, EMPLOYERS[0].name);
    }

    #[test]
    fn a_board_is_read_out_of_every_kind_of_job_link() {
        let cases = [
            (
                "https://job-boards.greenhouse.io/acme/jobs/123",
                Some(("greenhouse", "acme")),
            ),
            (
                "https://boards.greenhouse.io/acme/jobs/123?gh_jid=1",
                Some(("greenhouse", "acme")),
            ),
            (
                "https://boards.greenhouse.io/embed/job_app?for=acme&token=1",
                None,
            ),
            (
                "https://jobs.lever.co/acme/0000-1111",
                Some(("lever", "acme")),
            ),
            (
                "https://jobs.ashbyhq.com/Acme-Corp/uuid/application",
                Some(("ashby", "Acme-Corp")),
            ),
            (
                "https://jobs.smartrecruiters.com/AcmeInc/743999",
                Some(("smartrecruiters", "AcmeInc")),
            ),
            (
                "https://apply.workable.com/acme/j/ABC123/",
                Some(("workable", "acme")),
            ),
            ("https://apply.workable.com/j/ABC123", None),
            ("https://acme.com/careers/42", None),
            ("https://jobs.lever.co/", None),
        ];
        for (url, want) in cases {
            let got = board_in_url(url);
            let want = want.map(|(a, s)| (a, s.to_string()));
            assert_eq!(got, want, "{url}");
        }
    }

    #[test]
    fn discovery_only_returns_boards_we_do_not_read_yet() {
        let curated = match EMPLOYERS.iter().find_map(|e| match e.board {
            Board::Greenhouse(s) => Some(s),
            _ => None,
        }) {
            Some(s) => s,
            None => return,
        };
        let known = format!("https://boards.greenhouse.io/{curated}/jobs/1");
        let urls = [
            known.as_str(),
            "https://jobs.lever.co/brand-new-board-zz/1",
            "https://jobs.lever.co/brand-new-board-zz/2",
        ];
        let found = discover(urls, &[]);
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].slug, "brand-new-board-zz");
        assert_eq!(found[0].name, "Brand New Board Zz");
        // Already learned: nothing to add.
        assert!(discover(urls, &found).is_empty());
    }

    fn run(
        health: &mut HashMap<String, BoardHealth>,
        key: &str,
        answer: Answer,
        at: DateTime<Utc>,
    ) {
        reports().lock_recover().push((key.to_string(), answer));
        record_reports(health, at);
    }

    #[test]
    fn a_board_gone_three_days_running_is_skipped_for_a_week_and_then_tried_again() {
        let _serial = test_lock();
        let day0 = Utc::now();
        let mut health = HashMap::new();
        let key = "greenhouse:dead";
        for day in 0..FAILS_BEFORE_SKIP as i64 {
            run(&mut health, key, Answer::Gone, day0 + Duration::days(day));
        }
        let last = day0 + Duration::days(2);
        assert!(is_skipped(&health[key], last));
        assert!(is_skipped(&health[key], last + Duration::days(6)));
        assert!(!is_skipped(&health[key], last + Duration::days(8)));

        // Rows again, and it is healthy.
        run(&mut health, key, Answer::Rows(12), last + Duration::days(9));
        assert_eq!(health[key].fails, 0);
    }

    #[test]
    fn a_burst_of_refreshes_in_one_day_is_one_strike() {
        let _serial = test_lock();
        let now = Utc::now();
        let mut health = HashMap::new();
        for minute in 0..10 {
            run(
                &mut health,
                "lever:gone",
                Answer::Gone,
                now + Duration::minutes(minute),
            );
        }
        assert_eq!(health["lever:gone"].fails, 1);
    }

    /// The failure this guards against: three refreshes on a laptop with no
    /// network, which would otherwise retire every board for a week.
    #[test]
    fn an_offline_laptop_retires_no_board() {
        let _serial = test_lock();
        let now = Utc::now();
        let mut health = HashMap::new();
        for day in 0..5 {
            for key in ["greenhouse:a", "lever:b", "ashby:c"] {
                run(&mut health, key, Answer::Unclear, now + Duration::days(day));
            }
        }
        let later = now + Duration::days(5);
        assert!(health.values().all(|h| h.fails == 0));
        assert!(health.keys().all(|k| !is_skipped(&health[k], later)));
    }

    #[test]
    fn only_a_404_or_an_empty_board_is_a_definitive_no() {
        let empty: anyhow::Result<Vec<crate::models::Item>> = Ok(Vec::new());
        assert_eq!(Answer::of(&empty), Answer::Empty);
        let timeout: anyhow::Result<Vec<crate::models::Item>> = Err(anyhow::anyhow!("timed out"));
        assert_eq!(Answer::of(&timeout), Answer::Unclear);
    }
}
