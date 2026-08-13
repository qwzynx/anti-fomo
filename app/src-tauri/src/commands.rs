use std::sync::atomic::Ordering;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db;
use crate::models::Item;
use crate::rank::{self, personalize, DEFAULT_MAJOR};
use crate::scrapers;
use crate::AppState;

/// The old `/api/feed` endpoint capped its response at 60 items.
const FEED_LIMIT: usize = 60;
/// Matches the Python pipeline's 10-minute in-memory cache TTL.
const REFRESH_TTL_SECONDS: i64 = 600;
/// Items older than this are dropped; job posts and news both go stale fast.
const RETENTION_DAYS: i64 = 60;

const KEY_MAJOR: &str = "major";
const KEY_LAST_REFRESH: &str = "last_refresh";
const KEY_INTERESTS: &str = "interests";

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
    let mut items: Vec<Item> = db::load_items(&conn)
        .map_err(err)?
        .into_iter()
        .filter(|i| !states.dismissed.contains(&i.url))
        .collect();
    db::annotate(&mut items, &states);
    Ok(items)
}

/// Ranked feed, capped like the old `/api/feed`.
#[tauri::command]
pub fn get_feed(state: State<'_, AppState>, major: Option<String>) -> CmdResult<Vec<Item>> {
    let major = major.unwrap_or_else(|| current_major(&state));
    let interests = current_interests(&state);
    let mut ranked = personalize(visible_items(&state)?, &major, &interests);
    ranked.truncate(FEED_LIMIT);
    Ok(ranked)
}

/// Jobs and internships only, uncapped — the old `/api/internships`.
#[tauri::command]
pub fn get_internships(state: State<'_, AppState>, major: Option<String>) -> CmdResult<Vec<Item>> {
    let major = major.unwrap_or_else(|| current_major(&state));
    let interests = current_interests(&state);
    let opportunities = visible_items(&state)?
        .into_iter()
        .filter(|i| i.item_type.is_opportunity())
        .collect();
    Ok(personalize(opportunities, &major, &interests))
}

/// Everything the user starred, newest first. Served from the `item_state`
/// snapshots, so it works even for listings the cache has since pruned.
#[tauri::command]
pub fn get_saved(state: State<'_, AppState>) -> CmdResult<Vec<Item>> {
    let conn = state.db.lock().unwrap();
    db::load_saved(&conn).map_err(err)
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

/// Split out so the connection lock is never held across an await point.
fn persist(app: &AppHandle, items: Vec<Item>) -> CmdResult<usize> {
    let state = app.state::<AppState>();
    let count = items.len();

    // A scrape that returned nothing is a network failure, not an empty world —
    // keep the cached items rather than blanking the feed.
    if count == 0 {
        return Err("all sources returned no items (offline?)".to_string());
    }

    let mut conn = state.db.lock().unwrap();
    db::save_items(&mut conn, &items).map_err(err)?;
    db::prune_older_than(&conn, RETENTION_DAYS).map_err(err)?;
    // Runs after the prune so seen/dismissed rows for listings that have aged
    // out go with them. Saved rows are kept regardless.
    db::prune_orphan_states(&conn).map_err(err)?;
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
