use std::sync::atomic::Ordering;

use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db;
use crate::models::{self, Item};
use crate::rank::{self, personalize, DEFAULT_MAJOR};
use crate::scrapers::{self, details};
use crate::skills::{self, SkillCategory};
use crate::AppState;

/// The old `/api/feed` endpoint capped its response at 60, which threw away
/// most of what the sources returned — the UI pages through the feed now, so
/// the cap only exists to bound the size of one `invoke` payload.
const FEED_LIMIT: usize = 400;
/// Matches the Python pipeline's 10-minute in-memory cache TTL.
const REFRESH_TTL_SECONDS: i64 = 600;
/// Items older than this are dropped; job posts and news both go stale fast.
const RETENTION_DAYS: i64 = 60;
/// Descriptions fetched per refresh. The first handful of refreshes drain the
/// backlog of a fresh cache; steady state is the new postings of the day,
/// which is well under this.
const DETAIL_BUDGET: usize = 200;
/// Simultaneous detail requests. Measured against simplify.jobs: 10 concurrent
/// finish in 0.84s where serial takes 15.7s, with no rate limiting seen. Kept
/// modest because the chain now spreads across a dozen hosts, one of which
/// (Job Bank) is a government server the list scraper already treats politely.
const DETAIL_CONCURRENCY: usize = 8;
/// How many rows to consider per refresh before routing. Larger than the
/// budget because deciding a URL is unservable is free, and clearing those out
/// is what keeps a source we cannot fetch from blocking the queue.
const DETAIL_CANDIDATES: usize = DETAIL_BUDGET * 8;

const KEY_MAJOR: &str = "major";
const KEY_LAST_REFRESH: &str = "last_refresh";
const KEY_INTERESTS: &str = "interests";
const KEY_SKILLS: &str = "skills";

type CmdResult<T> = Result<T, String>;

fn err(e: impl std::fmt::Display) -> String {
    e.to_string()
}

#[derive(Serialize, Clone)]
pub struct FeedStatus {
    pub last_refresh: Option<String>,
    pub item_count: usize,
    pub refreshing: bool,
    pub stale: bool,
    pub saved_count: usize,
    pub dismissed_count: usize,
    /// Per-source counts from the last completed refresh, so Settings can show
    /// which sources are actually contributing and which have gone quiet.
    pub sources: Vec<SourceHealth>,
}

#[derive(Serialize, Clone)]
pub struct SourceHealth {
    pub name: String,
    pub count: usize,
}

/// Reads the persisted major, falling back to the default the old backend used
/// as its query-param default.
fn current_major(state: &AppState) -> String {
    let conn = state.db.lock().unwrap();
    db::get_setting(&conn, KEY_MAJOR)
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_MAJOR.to_string())
}

/// The user's chosen interest tags. Stored as a JSON array in `settings`; an
/// unset or corrupt value means "no interests", which simply zeroes that term.
fn current_interests(state: &AppState) -> Vec<String> {
    let conn = state.db.lock().unwrap();
    db::get_setting(&conn, KEY_INTERESTS)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// The skills the user says they have. Stored the same way as the interests:
/// a JSON array whose absence or corruption simply means "no skills", which
/// zeroes the coverage term rather than failing a read.
fn current_skills(state: &AppState) -> Vec<String> {
    let conn = state.db.lock().unwrap();
    db::get_setting(&conn, KEY_SKILLS)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

fn last_refresh(state: &AppState) -> Option<DateTime<Utc>> {
    let conn = state.db.lock().unwrap();
    db::get_setting(&conn, KEY_LAST_REFRESH)
        .ok()
        .flatten()
        .and_then(|s| DateTime::parse_from_rfc3339(&s).ok())
        .map(|d| d.with_timezone(&Utc))
}

fn is_stale(state: &AppState) -> bool {
    match last_refresh(state) {
        Some(at) => (Utc::now() - at).num_seconds() >= REFRESH_TTL_SECONDS,
        None => true,
    }
}

/// Loads the cache, drops anything the user dismissed, and stamps the
/// saved/seen flags. Every feed-shaped read starts here.
fn visible_items(state: &AppState) -> CmdResult<Vec<Item>> {
    let conn = state.db.lock().unwrap();
    let states = db::load_states(&conn).map_err(err)?;
    let details = db::load_details(&conn).map_err(err)?;
    let mut items: Vec<Item> = db::load_items(&conn)
        .map_err(err)?
        .into_iter()
        .filter(|i| !states.dismissed.contains(&i.url))
        .collect();
    db::annotate(&mut items, &states);
    // Before ranking, so skill extraction reads the real requirements rather
    // than the list endpoint's line of location text.
    db::attach_details(&mut items, &details);
    Ok(items)
}

/// Ranked feed, capped like the old `/api/feed`.
#[tauri::command]
pub fn get_feed(state: State<'_, AppState>, major: Option<String>) -> CmdResult<Vec<Item>> {
    let major = major.unwrap_or_else(|| current_major(&state));
    let interests = current_interests(&state);
    let user_skills = current_skills(&state);
    let mut ranked = personalize(visible_items(&state)?, &major, &interests, &user_skills);
    ranked.truncate(FEED_LIMIT);
    Ok(ranked)
}

/// Jobs and internships only, uncapped — the old `/api/internships`.
#[tauri::command]
pub fn get_internships(state: State<'_, AppState>, major: Option<String>) -> CmdResult<Vec<Item>> {
    let major = major.unwrap_or_else(|| current_major(&state));
    let interests = current_interests(&state);
    let user_skills = current_skills(&state);
    let opportunities = visible_items(&state)?
        .into_iter()
        .filter(|i| i.item_type.is_opportunity())
        .collect();
    Ok(personalize(opportunities, &major, &interests, &user_skills))
}

/// Everything the user starred, newest first. Served from the `item_state`
/// snapshots, so it works even for listings the cache has since pruned.
#[tauri::command]
pub fn get_saved(state: State<'_, AppState>) -> CmdResult<Vec<Item>> {
    let user_skills = current_skills(&state);
    let mut items = {
        let conn = state.db.lock().unwrap();
        let mut items = db::load_saved(&conn).map_err(err)?;
        // The snapshot predates any enrichment, so the live table wins.
        let details = db::load_details(&conn).map_err(err)?;
        db::attach_details(&mut items, &details);
        items
    };

    // The snapshot froze `matched_skills` at the moment of starring, so it
    // goes stale the first time the profile changes. `required_skills` is in
    // the snapshot too, which makes this a re-intersect rather than a re-scan.
    for item in &mut items {
        item.matched_skills = item
            .required_skills
            .iter()
            .filter(|s| user_skills.iter().any(|u| u == *s))
            .cloned()
            .collect();
    }
    Ok(items)
}

#[tauri::command]
pub fn feed_status(state: State<'_, AppState>) -> CmdResult<FeedStatus> {
    // The lock is scoped: `last_refresh`/`is_stale` below take it themselves,
    // and the mutex is not reentrant.
    let (items, states) = {
        let conn = state.db.lock().unwrap();
        (
            db::load_items(&conn).map_err(err)?,
            db::load_states(&conn).map_err(err)?,
        )
    };

    let mut counts: std::collections::HashMap<&str, usize> = std::collections::HashMap::new();
    for item in &items {
        *counts.entry(item.source_platform.as_str()).or_insert(0) += 1;
    }
    // Driven by the scraper registry, not by what happens to be in the cache,
    // so a source that has stopped returning anything shows up as 0 rather
    // than vanishing from the list. Deduped by name because some sources are
    // registered as several passes over one site (Job Bank runs two queries)
    // and the user should see one row per source, not one per request.
    let mut seen = std::collections::HashSet::new();
    let sources = scrapers::all_scrapers()
        .iter()
        .filter(|s| seen.insert(s.source_name()))
        .map(|s| SourceHealth {
            name: s.source_name().to_string(),
            count: counts.get(s.source_name()).copied().unwrap_or(0),
        })
        .collect();

    Ok(FeedStatus {
        last_refresh: last_refresh(&state).map(|d| d.to_rfc3339()),
        item_count: items.len(),
        refreshing: state.refreshing.load(Ordering::SeqCst),
        stale: is_stale(&state),
        saved_count: states.saved.len(),
        dismissed_count: states.dismissed.len(),
        sources,
    })
}

/// Scrapes every source and persists the results. Returns the number of items
/// written, or `None` when the call was skipped as still-fresh.
#[tauri::command]
pub async fn refresh(app: AppHandle, force: bool) -> CmdResult<Option<usize>> {
    let state = app.state::<AppState>();

    if !force && !is_stale(&state) {
        return Ok(None);
    }
    // Two windows, or a manual tap landing on top of the launch refresh,
    // should not scrape twice.
    if state
        .refreshing
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(None);
    }
    let _ = app.emit("feed:refreshing", true);

    let items = scrapers::fetch_all().await;
    let result = persist(&app, items);

    // Only worth doing once there is a cache to enrich, and deliberately
    // inside the `refreshing` flag so a second refresh cannot start on top of
    // it. The feed is already usable at this point — the scrape is persisted —
    // so this phase emits its own update when it lands.
    if result.is_ok() {
        match enrich_details(&app).await {
            Ok(0) => {}
            Ok(n) => {
                log::info!("fetched {n} job descriptions");
                let _ = app.emit("feed:updated", n);
            }
            // Enrichment is an enhancement; a failure leaves the scrape intact.
            Err(e) => log::warn!("description enrichment failed: {e}"),
        }
    }

    state.refreshing.store(false, Ordering::SeqCst);
    let _ = app.emit("feed:refreshing", false);

    match result {
        Ok(count) => {
            let _ = app.emit("feed:updated", count);
            Ok(Some(count))
        }
        Err(e) => Err(e),
    }
}

/// Fetches descriptions for postings we have never tried, up to
/// [`DETAIL_BUDGET`]. Returns how many rows were written.
///
/// Incremental by construction: a `job_details` row means "already attempted",
/// so the first refreshes on a fresh cache drain the backlog and later ones do
/// almost nothing. Failures are recorded too — otherwise a dead link would
/// cost one request on every refresh, forever.
async fn enrich_details(app: &AppHandle) -> CmdResult<usize> {
    let state = app.state::<AppState>();

    let candidates = {
        let conn = state.db.lock().unwrap();
        db::urls_needing_details(&conn, DETAIL_CANDIDATES).map_err(err)?
    };
    if candidates.is_empty() {
        return Ok(0);
    }

    // Routing is a pure function of the URL, so deciding that we cannot serve
    // a posting costs nothing. Recording those separately matters: Levels.fyi
    // stamps every row `Utc::now()`, so its unroutable postings sort to the
    // front of the queue and would otherwise consume the whole budget for
    // several refreshes without a single request being made.
    let mut unsupported = Vec::new();
    let mut queue = Vec::new();
    for (url, simplify_id) in candidates {
        let chain = details::route(&url, simplify_id.as_deref());
        if chain.is_empty() {
            unsupported.push((url, models::JobDetail::with_status(models::DetailStatus::Unsupported)));
        } else if queue.len() < DETAIL_BUDGET {
            queue.push((url, chain));
        }
    }

    if !unsupported.is_empty() {
        let mut conn = state.db.lock().unwrap();
        db::save_details(&mut conn, &unsupported).map_err(err)?;
    }
    if queue.is_empty() {
        return Ok(unsupported.len());
    }

    let client = scrapers::build_client().map_err(err)?;
    // `buffer_unordered` rather than `join_all`: the queue is 200 URLs and
    // opening 200 connections at once is neither polite nor faster. Note this
    // avoids `tokio::sync::Semaphore`, whose feature is not enabled here.
    let fetched: Vec<(String, models::JobDetail)> = stream::iter(queue)
        .map(|(url, chain)| {
            let client = client.clone();
            async move {
                let detail = details::fetch(&client, &chain).await;
                (url, detail)
            }
        })
        .buffer_unordered(DETAIL_CONCURRENCY)
        .collect()
        .await;

    let written = {
        let mut conn = state.db.lock().unwrap();
        db::save_details(&mut conn, &fetched).map_err(err)? + unsupported.len()
    };

    // The skill memo is keyed on URL and would otherwise keep serving the
    // answer it computed before these descriptions existed.
    skills::clear_memo();
    Ok(written)
}

/// Split out so the connection lock is never held across an await point.
fn persist(app: &AppHandle, items: Vec<Item>) -> CmdResult<usize> {
    let state = app.state::<AppState>();
    let count = items.len();

    // A scrape that returned nothing is a network failure, not an empty world —
    // keep the cached items rather than blanking the feed.
    if count == 0 {
        return Err("all sources returned no items (offline?)".to_string());
    }

    // A re-scrape can change the text behind a URL we have already extracted
    // skills for, so the memo has to go with it.
    skills::clear_memo();

    let mut conn = state.db.lock().unwrap();
    db::save_items(&mut conn, &items).map_err(err)?;
    db::prune_older_than(&conn, RETENTION_DAYS).map_err(err)?;
    // Runs after the prune so seen/dismissed rows for listings that have aged
    // out go with them. Saved rows are kept regardless.
    db::prune_orphan_states(&conn).map_err(err)?;
    db::prune_orphan_details(&conn).map_err(err)?;
    db::set_setting(&conn, KEY_LAST_REFRESH, &Utc::now().to_rfc3339()).map_err(err)?;
    Ok(count)
}

// --- user actions on items ---

/// Stars or unstars an item. The caller hands back the item it is looking at so
/// the snapshot can be written without a second lookup — the UI always has it.
#[tauri::command]
pub fn set_saved(
    state: State<'_, AppState>,
    url: String,
    saved: bool,
    item: Option<Item>,
) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    db::set_saved(&conn, &url, saved, item.as_ref()).map_err(err)
}

/// Hides an item from the feed and the hub for good.
#[tauri::command]
pub fn set_dismissed(state: State<'_, AppState>, url: String, dismissed: bool) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    db::set_dismissed(&conn, &url, dismissed).map_err(err)
}

/// Records that the user opened an item, which sinks it in later rankings.
#[tauri::command]
pub fn mark_seen(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    db::mark_seen(&conn, &url).map_err(err)
}

#[tauri::command]
pub fn clear_dismissed(state: State<'_, AppState>) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    db::clear_dismissed(&conn).map_err(err).map(|_| ())
}

// --- interests ---

/// The interest tags on offer, so the Settings picker never keeps its own copy.
#[tauri::command]
pub fn list_interests() -> Vec<String> {
    rank::list_interests()
}

#[tauri::command]
pub fn get_interests(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(current_interests(&state))
}

#[tauri::command]
pub fn set_interests(state: State<'_, AppState>, interests: Vec<String>) -> CmdResult<()> {
    let json = serde_json::to_string(&interests).map_err(err)?;
    let conn = state.db.lock().unwrap();
    db::set_setting(&conn, KEY_INTERESTS, &json).map_err(err)
}

// --- skills ---

/// The categorized skill catalog, so the picker never keeps its own copy of
/// the names — and so a skill can only ever be one the extractor knows about.
#[tauri::command]
pub fn list_skills() -> Vec<SkillCategory> {
    skills::list_skills()
}

#[tauri::command]
pub fn get_skills(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(current_skills(&state))
}

#[tauri::command]
pub fn set_skills(state: State<'_, AppState>, skills: Vec<String>) -> CmdResult<()> {
    let json = serde_json::to_string(&skills).map_err(err)?;
    let conn = state.db.lock().unwrap();
    db::set_setting(&conn, KEY_SKILLS, &json).map_err(err)
}

#[tauri::command]
pub fn get_setting(state: State<'_, AppState>, key: String) -> CmdResult<Option<String>> {
    let conn = state.db.lock().unwrap();
    db::get_setting(&conn, &key).map_err(err)
}

#[tauri::command]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    db::set_setting(&conn, &key, &value).map_err(err)
}

/// Every distinct source currently represented in the local store, for the
/// source filter chips.
#[tauri::command]
pub fn list_sources(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    let conn = state.db.lock().unwrap();
    let items = db::load_items(&conn).map_err(err)?;
    let mut sources: Vec<String> = items
        .into_iter()
        .map(|i| i.source_platform)
        .collect::<std::collections::BTreeSet<_>>()
        .into_iter()
        .collect();
    sources.sort();
    Ok(sources)
}
