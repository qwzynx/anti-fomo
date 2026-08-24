use std::sync::atomic::Ordering;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db;
use crate::models::{self, Item, ItemType};
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
/// which is well under this. Raised from 200 when the direct employer boards
/// landed: at ~200 a refresh, a cache of several thousand opportunities takes
/// dozens of refreshes to cover, and the reader would be looking at
/// description-less rows for days. Measured at 56 ms a posting, 600 at 12-wide
/// is about three seconds.
const DETAIL_BUDGET: usize = 600;
/// Simultaneous detail requests. Measured against simplify.jobs: 10 concurrent
/// finish in 0.84s where serial takes 15.7s, with no rate limiting seen. Kept
/// modest because the chain now spreads across a dozen hosts, one of which
/// (Job Bank) is a government server the list scraper already treats politely.
const DETAIL_CONCURRENCY: usize = 12;
/// How many rows to consider per refresh before routing. Larger than the
/// budget because deciding a URL is unservable is free, and clearing those out
/// is what keeps a source we cannot fetch from blocking the queue.
const DETAIL_CANDIDATES: usize = DETAIL_BUDGET * 8;

const KEY_MAJOR: &str = "major";
const KEY_LAST_REFRESH: &str = "last_refresh";
const KEY_INTERESTS: &str = "interests";
const KEY_SKILLS: &str = "skills";
/// The ranking weights, as a JSON `rank::Weights`. Absent until the user has
/// moved a slider, and any missing field falls back to the shipped default —
/// `Weights` is `#[serde(default)]` so an older value survives a new term.
const KEY_WEIGHTS: &str = "weights";
/// Per-company tier overrides, as a JSON object of canonical name to tier.
const KEY_COMPANY_TIERS: &str = "company_tiers";
/// A `location_tags` value the reader wants to favour, e.g. "Toronto".
const KEY_HOME_REGION: &str = "home_region";
/// Which seniority bands the reader is looking for, as a JSON array.
const KEY_TARGET_SENIORITY: &str = "target_seniority";

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

/// One row as a list renders it, borrowed straight out of the ranked cache.
///
/// The list commands used to return the whole [`Item`], fetched description
/// included. Measured against the real cache that made `get_internships` a
/// **45.6 MB** JSON payload for 17,739 rows — of which the four description
/// fields are 28 MB and `score_breakdown` most of the rest, and a list renders
/// neither. The detail pane asks for one posting through
/// [`get_item_detail`], which is the only place any of that is on screen.
///
/// Borrowed rather than owned so building the payload is a serialization pass
/// and not also a deep clone of the cache. Empty and absent fields are skipped
/// entirely: a null `salary_period` still costs its key name 17,739 times.
#[derive(Serialize)]
pub struct ListItem<'a> {
    title: &'a str,
    source_platform: &'a str,
    item_type: ItemType,
    url: &'a str,
    content_text: &'a str,
    timestamp: &'a DateTime<Utc>,
    discipline: Option<&'a str>,
    relevance_score: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    location: Option<&'a str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    location_tags: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    company: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    closes_at: Option<&'a DateTime<Utc>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_min: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_max: Option<f64>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_currency: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    salary_period: Option<&'a str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    seniority: Option<&'a str>,
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    matched_interests: &'a [String],
    /// Kept in full rather than reduced to a count: the UI re-intersects these
    /// against the profile the instant a skill chip is tapped, which is what
    /// lets the match figure move before the re-rank lands. `matched_skills`
    /// is *not* sent for the same reason — it is that intersection, and the UI
    /// is the one holding the live answer.
    #[serde(skip_serializing_if = "<[String]>::is_empty")]
    required_skills: &'a [String],
    #[serde(skip_serializing_if = "Option::is_none")]
    company_tier: Option<u8>,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    saved: bool,
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    seen: bool,
}

impl<'a> From<&'a Item> for ListItem<'a> {
    fn from(item: &'a Item) -> Self {
        ListItem {
            title: &item.title,
            source_platform: &item.source_platform,
            item_type: item.item_type,
            url: &item.url,
            content_text: &item.content_text,
            timestamp: &item.timestamp,
            discipline: item.discipline.as_deref(),
            relevance_score: item.relevance_score,
            location: item.location.as_deref(),
            location_tags: &item.location_tags,
            company: item.company.as_deref(),
            closes_at: item.closes_at.as_ref(),
            salary_min: item.salary_min,
            salary_max: item.salary_max,
            salary_currency: item.salary_currency.as_deref(),
            salary_period: item.salary_period.as_deref(),
            seniority: item.seniority.as_deref(),
            matched_interests: &item.matched_interests,
            required_skills: &item.required_skills,
            company_tier: item.company_tier,
            saved: item.saved,
            seen: item.seen,
        }
    }
}

pub fn as_list<'a>(items: impl IntoIterator<Item = &'a Item>) -> Vec<ListItem<'a>> {
    items.into_iter().map(ListItem::from).collect()
}

/// The whole visible cache, scored and ordered, held until something changes
/// it.
///
/// Ranking is the expensive half of a read — 1.2 s over 18,240 items on a
/// warm laptop, more on a phone — and `loadAll()` in the UI used to pay for it
/// three times over: once for the feed, once for the hub, once more for the
/// enrichment queue, on every event the refresh emitted. It is the same answer
/// all three times, so it is computed once per (data, profile) pair.
pub(crate) struct Ranked {
    generation: u64,
    profile: rank::Profile,
    items: Arc<Vec<Item>>,
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
    json_setting(state, KEY_SKILLS)
}

/// Reads one JSON-valued setting, degrading to the type's default rather than
/// failing the whole read. A corrupt `weights` value zeroes nothing — it falls
/// back to the shipped defaults.
fn json_setting<T: serde::de::DeserializeOwned + Default>(state: &AppState, key: &str) -> T {
    let conn = state.db.lock().unwrap();
    db::get_setting(&conn, key)
        .ok()
        .flatten()
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default()
}

/// Everything the scorer needs about the reader, read in one place so no
/// caller can rank against half a profile.
fn current_profile(state: &AppState, major: Option<String>) -> rank::Profile {
    let home_region = {
        let conn = state.db.lock().unwrap();
        db::get_setting(&conn, KEY_HOME_REGION)
            .ok()
            .flatten()
            .filter(|s| !s.trim().is_empty())
    };
    rank::Profile {
        major: major.unwrap_or_else(|| current_major(state)),
        interests: current_interests(state),
        skills: current_skills(state),
        weights: json_setting(state, KEY_WEIGHTS),
        company_tiers: json_setting(state, KEY_COMPANY_TIERS),
        home_region,
        target_seniority: json_setting(state, KEY_TARGET_SENIORITY),
    }
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

/// Marks every ranked answer stale. Called by anything that changes what a
/// read would return — a scrape, an enrichment pass, a setting, a save.
///
/// A counter rather than clearing the cache: invalidation happens on the write
/// path, which must never wait on a ranking pass that is already running.
pub(crate) fn invalidate(state: &AppState) {
    state.generation.fetch_add(1, Ordering::SeqCst);
}

/// The visible cache, scored and ordered, recomputed only when the data or the
/// profile has moved under it.
fn ranked_items(state: &AppState, major: Option<String>) -> CmdResult<Arc<Vec<Item>>> {
    // Read before taking the cache lock, so a write that lands mid-rank leaves
    // a generation the *next* read will not accept rather than one it will.
    let generation = state.generation.load(Ordering::SeqCst);
    let profile = current_profile(state, major);

    // Held across the ranking pass on purpose: `loadAll()` fires four commands
    // at once and they now run on separate threads, so without this the feed
    // and the hub would each start their own identical 1.2 s pass.
    let mut slot = state.ranked.lock().unwrap();
    if let Some(cached) = slot.as_ref() {
        if cached.generation == generation && cached.profile == profile {
            return Ok(Arc::clone(&cached.items));
        }
    }

    let mut items = personalize(visible_items(state)?, &profile);
    // The fetched posting has done its job — it is what skill extraction read
    // — and holding 28 MB of description text for the lifetime of the process
    // to render none of it is not worth the resident memory. `get_item_detail`
    // reads the one row it needs back out of `job_details`.
    for item in &mut items {
        item.description = None;
        item.requirements = None;
        item.responsibilities = None;
        item.perks = None;
        item.tagged_skills = Vec::new();
        item.score_breakdown = Vec::new();
    }

    let items = Arc::new(items);
    *slot = Some(Ranked {
        generation,
        profile,
        items: Arc::clone(&items),
    });
    Ok(items)
}

/// A payload already in its final form.
///
/// [`ListItem`] borrows from the ranked cache, so it cannot be handed back as
/// a value — the `Arc` is dropped at the end of the command. Serializing it
/// here and passing the JSON through verbatim is what keeps the response a
/// single serialization pass rather than a deep clone of the cache followed by
/// one.
type Payload = Box<serde_json::value::RawValue>;

/// Ranked feed, capped like the old `/api/feed`.
#[tauri::command(async)]
pub fn get_feed(state: State<'_, AppState>, major: Option<String>) -> CmdResult<Payload> {
    let ranked = ranked_items(&state, major)?;
    serde_json::value::to_raw_value(&as_list(ranked.iter().take(FEED_LIMIT))).map_err(err)
}

/// Jobs and internships only, uncapped — the old `/api/internships`.
///
/// Filtered out of the same ranked list the feed reads rather than ranked
/// again: the hub re-sorts client-side on every one of its six sort options,
/// including "Best match", so a second diversification pass over the same
/// scores was work whose result was thrown away.
#[tauri::command(async)]
pub fn get_internships(state: State<'_, AppState>, major: Option<String>) -> CmdResult<Payload> {
    let ranked = ranked_items(&state, major)?;
    serde_json::value::to_raw_value(&as_list(
        ranked.iter().filter(|i| i.item_type.is_opportunity()),
    ))
    .map_err(err)
}

/// Everything the user starred, newest first. Served from the `item_state`
/// snapshots, so it works even for listings the cache has since pruned.
#[tauri::command(async)]
pub fn get_saved(state: State<'_, AppState>) -> CmdResult<Vec<Item>> {
    let mut items = {
        let conn = state.db.lock().unwrap();
        db::load_saved(&conn).map_err(err)?
    };

    // The snapshot froze `matched_skills` at the moment of starring, so it
    // goes stale the first time the profile changes. `required_skills` is in
    // the snapshot too, which makes this a re-intersect rather than a re-scan.
    let user_skills = current_skills(state.inner());
    for item in &mut items {
        item.matched_skills = item
            .required_skills
            .iter()
            .filter(|s| user_skills.iter().any(|u| u == *s))
            .cloned()
            .collect();
        // The snapshot can carry a stale description; the pane refetches.
        item.description = None;
        item.requirements = None;
        item.responsibilities = None;
        item.perks = None;
        item.score_breakdown = Vec::new();
    }
    Ok(items)
}

/// One posting in full, for the detail pane and nothing else.
///
/// The list payload deliberately stops at what a row renders. This is the
/// other half: the fetched description, the sections it splits into, and the
/// score breakdown behind the row's position. One row of `job_details` rather
/// than the 28 MB the whole table holds.
#[tauri::command(async)]
pub fn get_item_detail(state: State<'_, AppState>, url: String) -> CmdResult<Option<Item>> {
    // The ranked cache already holds everything but the posting's own text and
    // the breakdown, both of which are stripped when it is built.
    let ranked = ranked_items(&state, None)?;
    let cached = ranked.iter().find(|i| i.url == url).cloned();

    let mut items = {
        let conn = state.db.lock().unwrap();
        // Not in the feed means dismissed or pruned — the saved list still
        // shows those, and it renders them from the snapshot.
        let item = match cached {
            Some(item) => item,
            None => match db::load_saved(&conn)
                .map_err(err)?
                .into_iter()
                .find(|i| i.url == url)
            {
                Some(item) => item,
                None => return Ok(None),
            },
        };
        let mut items = vec![item];
        if let Some(detail) = db::load_detail(&conn, &url).map_err(err)? {
            let details = std::iter::once((url, detail)).collect();
            db::attach_details(&mut items, &details);
        }
        items
    };

    // Re-scored for its own sake: the ranked cache drops `score_breakdown`
    // rather than hold 18,240 of them to render at most one, and the pane is
    // the only place the terms behind a position are ever shown.
    let profile = current_profile(state.inner(), None);
    items = personalize(items, &profile);
    Ok(items.pop())
}

#[tauri::command(async)]
pub fn feed_status(state: State<'_, AppState>) -> CmdResult<FeedStatus> {
    // Counted in SQL rather than by materialising the cache: this command runs
    // after every save and every dismissal, and loading 18,240 rows to call
    // `.len()` on them cost 38 ms and several megabytes each time.
    let (item_count, counts, saved_count, dismissed_count) = {
        let conn = state.db.lock().unwrap();
        let (saved, dismissed) = db::state_counts(&conn).map_err(err)?;
        (
            db::count_items(&conn).map_err(err)?,
            db::source_counts(&conn).map_err(err)?,
            saved,
            dismissed,
        )
    };

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
        item_count,
        refreshing: state.refreshing.load(Ordering::SeqCst),
        stale: is_stale(&state),
        saved_count,
        dismissed_count,
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
    // Writing 18,000 upserts is not something to do on the async runtime's
    // thread — it would stall every other task on it for the duration.
    let result = {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || persist(&app, items))
            .await
            .map_err(err)?
    };

    // Announced as soon as the scrape lands rather than after enrichment.
    // Enrichment is up to 600 HTTP requests and takes seconds; the feed is
    // already usable the moment `persist` returns, and holding the event back
    // meant a refresh looked like it had done nothing until the very end.
    if let Ok(count) = &result {
        let _ = app.emit("feed:updated", *count);
    }

    // Only worth doing once there is a cache to enrich, and deliberately
    // inside the `refreshing` flag so a second refresh cannot start on top of
    // it. This phase emits its own update when it lands.
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
    result.map(Some)
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

    // Rank order, not insertion order. With several thousand opportunities in
    // the cache the budget only ever covers a slice of the backlog, and it
    // should be the slice the reader is about to scroll through — a posting
    // ranked four hundredth can wait for the next refresh. `diversify` also
    // spreads the queue across employers, so no single Workday tenant's
    // backlog monopolises a refresh.
    //
    // Off the async runtime and through the shared ranked cache: this is the
    // same ranking pass `get_feed` needs a moment later, and running it a
    // third time on the thread driving the HTTP futures stalled every one of
    // them for its duration.
    let candidates: Vec<(String, Option<String>)> = {
        let app = app.clone();
        tauri::async_runtime::spawn_blocking(move || -> CmdResult<_> {
            let state = app.state::<AppState>();
            let attempted = {
                let conn = state.db.lock().unwrap();
                db::urls_with_details(&conn).map_err(err)?
            };
            Ok(ranked_items(&state, None)?
                .iter()
                .filter(|i| i.item_type.is_opportunity() && !attempted.contains(&i.url))
                .take(DETAIL_CANDIDATES)
                .map(|i| (i.url.clone(), i.simplify_id.clone()))
                .collect())
        })
        .await
        .map_err(err)??
    };

    if candidates.is_empty() {
        return Ok(0);
    }

    // Routing is a pure function of the URL, so deciding that we cannot serve
    // a posting costs nothing. Recording those separately matters: an
    // unroutable posting that ranks well sits at the front of the queue, where
    // it would otherwise consume the whole budget for several refreshes
    // without a single request being made.
    let mut unsupported = Vec::new();
    let mut queue = Vec::new();
    for (url, simplify_id) in candidates {
        let chain = details::route(&url, simplify_id.as_deref());
        if chain.is_empty() {
            unsupported.push((
                url,
                models::JobDetail::with_status(models::DetailStatus::Unsupported),
            ));
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
    invalidate(&state);
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

    // Lever and Ashby return the whole posting in the same response as the
    // listing. Those descriptions are harvested here rather than left on the
    // enrichment queue — several thousand requests for text already in hand —
    // and they go into `job_details`, not onto the row: `save_items`
    // overwrites `content_text` every refresh and would erase them.
    let seeded: Vec<(String, models::JobDetail)> = items
        .iter()
        .filter_map(|item| {
            let detail = models::JobDetail {
                description: item.description.clone(),
                requirements: item.requirements.clone(),
                responsibilities: item.responsibilities.clone(),
                perks: item.perks.clone(),
                tagged_skills: item.tagged_skills.clone(),
                closes_at: item.closes_at,
                salary_min: item.salary_min,
                salary_max: item.salary_max,
                salary_currency: item.salary_currency.clone(),
                salary_period: item.salary_period.clone(),
                status: models::DetailStatus::Ok,
            };
            detail.has_text().then(|| (item.url.clone(), detail))
        })
        .collect();

    let mut conn = state.db.lock().unwrap();
    db::save_items(&mut conn, &items).map_err(err)?;
    if !seeded.is_empty() {
        db::save_details(&mut conn, &seeded).map_err(err)?;
    }
    db::prune_older_than(&conn, RETENTION_DAYS).map_err(err)?;
    // Runs after the prune so seen/dismissed rows for listings that have aged
    // out go with them. Saved rows are kept regardless.
    db::prune_orphan_states(&conn).map_err(err)?;
    db::prune_orphan_details(&conn).map_err(err)?;
    db::set_setting(&conn, KEY_LAST_REFRESH, &Utc::now().to_rfc3339()).map_err(err)?;
    drop(conn);
    invalidate(&state);
    Ok(count)
}

// --- user actions on items ---
//
// Every one of these invalidates the ranked cache. They are cheap writes and
// the rebuild is lazy, so the cost lands on the next read that actually wants
// a ranked list — which for a save or a read mark is usually never, since the
// UI patches those into the list it is already holding.

/// Stars or unstars an item. The caller hands back the item it is looking at so
/// the snapshot can be written without a second lookup — the UI always has it.
#[tauri::command(async)]
pub fn set_saved(
    state: State<'_, AppState>,
    url: String,
    saved: bool,
    item: Option<Item>,
) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();
        db::set_saved(&conn, &url, saved, item.as_ref()).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

/// Hides an item from the feed and the hub for good.
#[tauri::command(async)]
pub fn set_dismissed(state: State<'_, AppState>, url: String, dismissed: bool) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();
        db::set_dismissed(&conn, &url, dismissed).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

/// Records that the user opened an item, which sinks it in later rankings.
#[tauri::command(async)]
pub fn mark_seen(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();
        db::mark_seen(&conn, &url).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

#[tauri::command(async)]
pub fn clear_dismissed(state: State<'_, AppState>) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();
        db::clear_dismissed(&conn).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

/// Empties the local store — cached items, fetched descriptions, saves,
/// dismissals and read marks — and keeps the profile in `settings`, so the
/// field, interests and skills the user set are still there afterwards.
#[tauri::command(async)]
pub fn clear_data(state: State<'_, AppState>) -> CmdResult<()> {
    // A scrape in flight would write its results in behind the delete and
    // leave the store half-full of the items the user just cleared.
    if state.refreshing.load(Ordering::SeqCst) {
        return Err("A refresh is running. Try again once it finishes.".into());
    }
    {
        let conn = state.db.lock().unwrap();
        db::clear_data(&conn).map_err(err)?;
    }
    skills::clear_memo();
    invalidate(&state);
    Ok(())
}

// --- interests ---

/// The interest tags on offer, so the Settings picker never keeps its own copy.
#[tauri::command]
pub fn list_interests() -> Vec<String> {
    rank::list_interests()
}

#[tauri::command(async)]
pub fn get_interests(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(current_interests(&state))
}

#[tauri::command(async)]
pub fn set_interests(state: State<'_, AppState>, interests: Vec<String>) -> CmdResult<()> {
    let json = serde_json::to_string(&interests).map_err(err)?;
    {
        let conn = state.db.lock().unwrap();
        db::set_setting(&conn, KEY_INTERESTS, &json).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

// --- skills ---

/// The categorized skill catalog, so the picker never keeps its own copy of
/// the names — and so a skill can only ever be one the extractor knows about.
#[tauri::command]
pub fn list_skills() -> Vec<SkillCategory> {
    skills::list_skills()
}

#[tauri::command(async)]
pub fn get_skills(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    Ok(current_skills(&state))
}

#[tauri::command(async)]
pub fn set_skills(state: State<'_, AppState>, skills: Vec<String>) -> CmdResult<()> {
    let json = serde_json::to_string(&skills).map_err(err)?;
    {
        let conn = state.db.lock().unwrap();
        db::set_setting(&conn, KEY_SKILLS, &json).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

#[tauri::command(async)]
pub fn get_setting(state: State<'_, AppState>, key: String) -> CmdResult<Option<String>> {
    let conn = state.db.lock().unwrap();
    db::get_setting(&conn, &key).map_err(err)
}

#[tauri::command(async)]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> CmdResult<()> {
    {
        let conn = state.db.lock().unwrap();
        db::set_setting(&conn, &key, &value).map_err(err)?;
    }
    // `major`, `weights`, `home_region` and the company tiers all arrive
    // through here, and every one of them is part of the ranking profile.
    invalidate(&state);
    Ok(())
}

/// Every distinct source currently represented in the local store, for the
/// source filter chips.
#[tauri::command(async)]
pub fn list_sources(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    let conn = state.db.lock().unwrap();
    db::distinct_sources(&conn).map_err(err)
}
