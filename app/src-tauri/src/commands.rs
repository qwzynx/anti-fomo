use std::sync::atomic::Ordering;

use chrono::{DateTime, Utc};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db;
use crate::models::Item;
use crate::rank::{personalize, DEFAULT_MAJOR};
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

/// Ranked feed, capped like the old `/api/feed`.
#[tauri::command]
pub fn get_feed(state: State<'_, AppState>, major: Option<String>) -> CmdResult<Vec<Item>> {
    let major = major.unwrap_or_else(|| current_major(&state));
    let items = {
        let conn = state.db.lock().unwrap();
        db::load_items(&conn).map_err(err)?
    };
    let mut ranked = personalize(items, &major);
    ranked.truncate(FEED_LIMIT);
    Ok(ranked)
}

/// Jobs and internships only, uncapped — the old `/api/internships`.
#[tauri::command]
pub fn get_internships(state: State<'_, AppState>, major: Option<String>) -> CmdResult<Vec<Item>> {
    let major = major.unwrap_or_else(|| current_major(&state));
    let items = {
        let conn = state.db.lock().unwrap();
        db::load_items(&conn).map_err(err)?
    };
    let opportunities = items
        .into_iter()
        .filter(|i| i.item_type.is_opportunity())
        .collect();
    Ok(personalize(opportunities, &major))
}

#[tauri::command]
pub fn feed_status(state: State<'_, AppState>) -> CmdResult<FeedStatus> {
    let item_count = {
        let conn = state.db.lock().unwrap();
        db::load_items(&conn).map_err(err)?.len()
    };
    Ok(FeedStatus {
        last_refresh: last_refresh(&state).map(|d| d.to_rfc3339()),
        item_count,
        refreshing: state.refreshing.load(Ordering::SeqCst),
        stale: is_stale(&state),
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
    db::set_setting(&conn, KEY_LAST_REFRESH, &Utc::now().to_rfc3339()).map_err(err)?;
    Ok(count)
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
