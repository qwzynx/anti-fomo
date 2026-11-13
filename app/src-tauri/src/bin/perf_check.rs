//! Where a feed read actually spends its time, measured against the real
//! database rather than a fixture.
//!
//! The UI's `loadAll()` is four `invoke`s, and the two expensive ones —
//! `get_feed` and `get_internships` — each walk the whole cache. This times
//! every stage of that walk so an optimisation can be shown to have worked
//! rather than assumed to have:
//!
//! ```text
//! cargo run --release --features dev-tools --bin perf_check -- [db_path]
//! ```
//!
//! Run it against a *copy* of the database. It opens the file directly rather
//! than through [`db::open`], which would run the migration and drop `items`.

use std::time::Instant;

use anti_fomo_lib::commands::{as_list, detail_candidates};
use anti_fomo_lib::models::{Item, ItemType};
use anti_fomo_lib::rank::{personalize, personalize_with, Profile};
use anti_fomo_lib::scrapers::details::{route, Handler};
use anti_fomo_lib::{db, skills};
use chrono::{Duration, Utc};

fn default_db() -> std::path::PathBuf {
    std::env::var_os("XDG_DATA_HOME")
        .map(std::path::PathBuf::from)
        .unwrap_or_else(|| {
            std::path::PathBuf::from(std::env::var("HOME").expect("HOME"))
                .join(".local")
                .join("share")
        })
        .join("dev.qwzynx.antifomo")
        .join("antifomo.db")
}

macro_rules! time {
    ($label:expr, $body:expr) => {{
        let start = Instant::now();
        let value = $body;
        println!(
            "{:>34}  {:>8.1} ms",
            $label,
            start.elapsed().as_secs_f64() * 1000.0
        );
        value
    }};
}

/// How the quality bars and the last-call rule behave on this corpus, for one
/// reader. The numbers that matter are not timings: they are whether a
/// populated profile gets a handful of last calls rather than none or hundreds
/// (tune `OPPORTUNITY_FLOOR` and the percentiles in `rank.rs` until it does),
/// and whether the postings a reader would want a deadline for have one.
fn calibrate(label: &str, items: &[Item], profile: &Profile) {
    let now = Utc::now();
    let (ranked, bars) = personalize_with(items.to_vec(), profile, now, None);
    println!("--- calibration: {label} ---");
    println!(
        "{:>34}  roles {:.1}..{:.1}   events {:.1}..{:.1}",
        "quality bars (lo..hi)",
        bars.opportunity.lo,
        bars.opportunity.hi,
        bars.event.lo,
        bars.event.hi
    );

    let is_role = |i: &&Item| i.item_type.is_opportunity();
    let mut merits: Vec<f64> = ranked
        .iter()
        .filter(is_role)
        .filter_map(|i| i.merit)
        .collect();
    merits.sort_by(f64::total_cmp);
    let at = |p: f64| merits[((merits.len() - 1) as f64 * p).round() as usize];
    if !merits.is_empty() {
        println!(
            "{:>34}  p50 {:.1}  p75 {:.1}  p90 {:.1}  max {:.1}  (n={})",
            "role merit",
            at(0.5),
            at(0.75),
            at(0.9),
            at(1.0),
            merits.len()
        );
    }

    let week = now + Duration::days(7);
    let soon: Vec<&Item> = ranked
        .iter()
        .filter(|i| i.item_type.is_opportunity() || i.item_type == ItemType::Event)
        .filter(|i| i.closes_at.is_some_and(|c| c >= now && c <= week))
        .collect();
    println!(
        "{:>34}  {} rows closing within 7 days, {} flagged last call",
        "last call",
        soon.len(),
        soon.iter().filter(|i| i.last_call).count()
    );

    let mut roles: Vec<&Item> = ranked.iter().filter(is_role).collect();
    roles.sort_by(|a, b| b.merit.unwrap_or(0.0).total_cmp(&a.merit.unwrap_or(0.0)));
    for n in [600, 2_000] {
        let known = roles
            .iter()
            .take(n)
            .filter(|i| i.closes_at.is_some())
            .count();
        println!(
            "{:>34}  {known} of the top {n} by merit have a known deadline",
            "deadline coverage"
        );
    }
    println!();
}

/// Which postings the next description fetches go to, and whether they can come
/// back with a deadline. Only three handlers read one (Workday's `endDate`,
/// Job Bank's "advertised until", and a posting page's schema.org
/// `validThrough`), so what matters is how many of a refresh's 600 fetches are
/// aimed at one of them — a "last call" can only ever show what has been read.
fn queue_check(items: &[Item], attempted: &std::collections::HashSet<String>, profile: &Profile) {
    const BUDGET: usize = 600;
    let now = Utc::now();
    let (ranked, _) = personalize_with(items.to_vec(), profile, now, None);

    // The queue this replaces: feed order, straight off the ranked list.
    let by_rank: Vec<(String, Option<String>)> = ranked
        .iter()
        .filter(|i| i.item_type.is_opportunity() && !attempted.contains(&i.url))
        .take(BUDGET * 8)
        .map(|i| (i.url.clone(), i.simplify_id.clone()))
        .collect();
    let by_merit = detail_candidates(&ranked, attempted, now);

    let capable = |chain: &[Handler]| {
        chain.iter().any(|h| {
            matches!(
                h,
                Handler::Workday { .. } | Handler::JobBank(_) | Handler::Page(_)
            )
        })
    };
    // The first `BUDGET` fetches an ordering would make: only routable postings
    // spend budget (an unroutable one is recorded as unsupported for free), and
    // each is tagged with whether its chain can come back with a deadline.
    let fetches = |queue: &[(String, Option<String>)]| -> Vec<(String, bool)> {
        let mut out = Vec::new();
        for (url, simplify_id) in queue {
            let chain = route(url, simplify_id.as_deref());
            if chain.is_empty() {
                continue;
            }
            out.push((url.clone(), capable(&chain)));
            if out.len() == BUDGET {
                break;
            }
        }
        out
    };

    // What the ordering is *for*: the postings a reader would most want a
    // deadline for. Taking the share of fetches that can return one (below)
    // says nothing about that — both orderings draw from the same pool — so
    // this asks the question directly: of the best `BUDGET` unread postings by
    // merit, how many does each ordering reach, and how many of those can
    // actually come back with a deadline?
    let mut best: Vec<&Item> = ranked
        .iter()
        .filter(|i| i.item_type.is_opportunity() && !attempted.contains(&i.url))
        .filter(|i| i.closes_at.is_none_or(|c| c >= now))
        .filter(|i| !route(&i.url, i.simplify_id.as_deref()).is_empty())
        .collect();
    best.sort_by(|a, b| b.merit.unwrap_or(0.0).total_cmp(&a.merit.unwrap_or(0.0)));
    let best: std::collections::HashSet<&str> =
        best.iter().take(BUDGET).map(|i| i.url.as_str()).collect();

    println!("--- the next {BUDGET} description fetches ---");
    for (label, queue) in [
        ("feed order (before)", &by_rank),
        ("merit order (now)", &by_merit),
    ] {
        let got = fetches(queue);
        let readable = got.iter().filter(|(_, c)| *c).count();
        let reached = got
            .iter()
            .filter(|(u, _)| best.contains(u.as_str()))
            .count();
        let reached_readable = got
            .iter()
            .filter(|(u, c)| *c && best.contains(u.as_str()))
            .count();
        println!(
            "{label:>34}  {readable}/{} can return a deadline · reaches {reached}/{} of the best-by-merit ({reached_readable} readable)",
            got.len(),
            best.len()
        );
    }
    println!();
}

fn main() -> anyhow::Result<()> {
    let path = std::env::args()
        .nth(1)
        .map(std::path::PathBuf::from)
        .unwrap_or_else(default_db);
    let conn = rusqlite::Connection::open(&path)?;
    println!("{}\n", path.display());

    // --- one `visible_items()` ---
    let states = time!("load_states", db::load_states(&conn)?);
    let details = time!("load_details", db::load_details(&conn)?);
    let mut items = time!("load_items", db::load_items(&conn)?);
    items.retain(|i| !states.dismissed.contains(&i.url));
    time!("annotate", db::annotate(&mut items, &states));
    time!("attach_details", db::attach_details(&mut items, &details));
    println!(
        "{:>34}  {:>8} items, {} details\n",
        "corpus",
        items.len(),
        details.len()
    );

    let profile = Profile {
        major: "Software Engineering".into(),
        skills: vec!["Python".into(), "React".into(), "AWS".into()],
        ..Profile::default()
    };

    // --- ranking, cold and warm ---
    // Cold is the first read after a scrape, when the skill memo is empty;
    // warm is every read after that.
    let ranked = time!(
        "personalize (cold memo)",
        personalize(items.clone(), &profile)
    );
    let ranked2 = time!(
        "personalize (warm memo)",
        personalize(items.clone(), &profile)
    );
    drop(ranked2);

    let opportunities: Vec<_> = ranked
        .iter()
        .filter(|i| i.item_type.is_opportunity())
        .cloned()
        .collect();

    // --- calibration: an empty reader, and one who has answered the form ---
    calibrate("nothing set", &items, &Profile::default());
    calibrate(
        "onboarded: AI/ML + Backend, intern/new grad, Toronto",
        &items,
        &Profile {
            major: "Software Engineering".into(),
            interests: vec!["AI/ML".into(), "Backend".into()],
            target_seniority: vec!["Intern".into(), "New grad".into()],
            home_region: Some("Toronto".into()),
            ..Profile::default()
        },
    );

    let attempted = db::urls_with_details(&conn)?;
    queue_check(
        &items,
        &attempted,
        &Profile {
            major: "Software Engineering".into(),
            interests: vec!["AI/ML".into(), "Backend".into()],
            target_seniority: vec!["Intern".into(), "New grad".into()],
            home_region: Some("Toronto".into()),
            ..Profile::default()
        },
    );

    // --- what crosses the IPC boundary ---
    let feed_json = time!(
        "serialize get_feed (400)",
        serde_json::to_string(&as_list(ranked.iter().take(400)))?
    );
    let hub_json = time!(
        "serialize get_internships (all)",
        serde_json::to_string(&as_list(opportunities.iter()))?
    );
    let whole_json = time!(
        "  (was: whole Item, all)",
        serde_json::to_string(&opportunities)?
    );
    println!(
        "{:>34}  {:>8.1} MB feed, {:.1} MB hub  (was {:.1} MB)\n",
        "payload",
        feed_json.len() as f64 / 1e6,
        hub_json.len() as f64 / 1e6,
        whole_json.len() as f64 / 1e6
    );

    // --- feed_status, which loads the whole cache to count it ---
    time!("feed_status counts (SQL)", {
        db::count_items(&conn)?;
        db::source_counts(&conn)?;
        db::state_counts(&conn)?
    });
    time!("  (was: load_items + len)", db::load_items(&conn)?.len());
    time!(
        "skills::extract over the cache",
        ranked
            .iter()
            .map(|i| skills::extract(i).len())
            .sum::<usize>()
    );

    Ok(())
}
