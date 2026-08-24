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

use anti_fomo_lib::commands::as_list;
use anti_fomo_lib::rank::{personalize, Profile};
use anti_fomo_lib::{db, skills};

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
