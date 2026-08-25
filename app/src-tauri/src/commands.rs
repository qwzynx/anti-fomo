use std::sync::atomic::Ordering;
use std::sync::Arc;

use chrono::{DateTime, Utc};
use futures::stream::{self, StreamExt};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db;
use crate::models::{self, Item, ItemType};
use crate::rank::{self, personalize, DEFAULT_MAJOR};
use crate::resume;
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
    item_detail(state.inner(), &url)
}

/// One posting in full, resolved and re-scored.
///
/// Split out of the command because the résumé commands need exactly this and
/// for exactly the same reasons — a posting the reader is tailoring against may
/// well be one they saved months ago, which the ranked cache has long since
/// pruned. Two lookups that disagreed about where a posting can be found would
/// mean tailoring silently failing on saved jobs.
fn item_detail(state: &AppState, url: &str) -> CmdResult<Option<Item>> {
    let url = url.to_string();
    // The ranked cache already holds everything but the posting's own text and
    // the breakdown, both of which are stripped when it is built.
    let ranked = ranked_items(state, None)?;
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
    let profile = current_profile(state, None);
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

// --- résumés -----------------------------------------------------------
//
// Every one of these is `#[tauri::command(async)]` for the reason the rest of
// this file is: a plain `#[tauri::command]` on a synchronous function runs on
// the main thread, which is the webview's thread, so a save would freeze the
// window it is trying to give feedback in.
//
// None of them calls `invalidate()`. That rule is about writes which change
// what a *ranked read* returns, and nothing here does — a résumé does not enter
// scoring. Bumping the generation would throw away a ranking pass over 18,000
// items every time the user typed a bullet.

/// One row of the résumé picker.
#[derive(Serialize)]
pub struct ResumeSummary {
    id: String,
    name: String,
    is_default: bool,
    updated_at: String,
}

/// A résumé as the builder edits it.
#[derive(Serialize)]
pub struct StoredResume {
    id: String,
    name: String,
    doc: resume::Resume,
    theme: resume::Theme,
    is_default: bool,
}

/// A laid-out résumé, plus — when it was tailored to a posting — what the
/// tailoring decided and why.
#[derive(Serialize)]
pub struct ResumeView {
    pages: Vec<resume::layout::Page>,
    /// How full the last page is, 0.0–1.0.
    fill: f32,
    /// Bullet ids on the page.
    bullets: Vec<String>,
    /// Entry ids on the page.
    entries: Vec<String>,
    /// Bullet id → the posting skills it covers.
    why: std::collections::HashMap<String, Vec<String>>,
    /// Bullets the page budget trimmed.
    dropped: Vec<resume::tailor::Dropped>,
    /// Posting skills the surviving bullets speak to, and everything it asked
    /// for. The pane renders these as "your résumé covers 4 of 9".
    covered: Vec<String>,
    required: Vec<String>,
    /// The theme actually used, once the variant's override is applied — the
    /// picker has to show what is on screen, not what is stored.
    theme: resume::Theme,
    /// What a save dialog should suggest.
    filename: String,
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

/// Reads one stored résumé, or the default when no id is given.
fn stored_resume(state: &AppState, id: Option<&str>) -> CmdResult<Option<db::StoredResume>> {
    let conn = state.db.lock().unwrap();
    match id {
        Some(id) => db::load_resume(&conn, id).map_err(err),
        None => db::default_resume(&conn).map_err(err),
    }
}

/// Parses a stored row into the model, degrading a corrupt row to a usable
/// starting point rather than failing the whole screen. Losing the styling of a
/// résumé is recoverable; being unable to open it at all is not.
fn parse_stored(row: db::StoredResume) -> (String, String, resume::Resume, resume::Theme, bool) {
    let doc: resume::Resume = serde_json::from_str(&row.doc).unwrap_or_else(|e| {
        log::warn!(
            "résumé {} did not parse ({e}); opening an empty one",
            row.id
        );
        resume::Resume::starter()
    });
    let theme: resume::Theme = serde_json::from_str(&row.theme).unwrap_or_default();
    (row.id, row.name, doc, theme.sanitized(), row.is_default)
}

#[tauri::command(async)]
pub fn list_resumes(state: State<'_, AppState>) -> CmdResult<Vec<ResumeSummary>> {
    let conn = state.db.lock().unwrap();
    Ok(db::list_resumes(&conn)
        .map_err(err)?
        .into_iter()
        .map(|r| ResumeSummary {
            id: r.id,
            name: r.name,
            is_default: r.is_default,
            updated_at: r.updated_at,
        })
        .collect())
}

/// One résumé. `id` absent means the default one, which is how the builder
/// opens without the UI having to know an id first.
#[tauri::command(async)]
pub fn get_resume(
    state: State<'_, AppState>,
    id: Option<String>,
) -> CmdResult<Option<StoredResume>> {
    let Some(row) = stored_resume(state.inner(), id.as_deref())? else {
        return Ok(None);
    };
    let (id, name, doc, theme, is_default) = parse_stored(row);
    Ok(Some(StoredResume {
        id,
        name,
        doc,
        theme,
        is_default,
    }))
}

/// Creates or updates a résumé. Returns the id, which is how the UI learns the
/// id of one it just created.
#[tauri::command(async)]
pub fn save_resume(
    state: State<'_, AppState>,
    id: Option<String>,
    name: String,
    doc: resume::Resume,
    theme: resume::Theme,
) -> CmdResult<String> {
    let mut doc = doc;
    // Mints any missing id and re-reads every bullet's skills from its text.
    // Doing it on save rather than on read means tailoring is a set
    // intersection later, not a scan over every bullet on every keystroke.
    doc.normalize();

    let id = id
        .filter(|s| !s.trim().is_empty())
        .unwrap_or_else(resume::model::new_id);
    let name = match name.trim() {
        "" => "Untitled résumé".to_string(),
        name => name.to_string(),
    };
    let doc_json = serde_json::to_string(&doc).map_err(err)?;
    let theme_json = serde_json::to_string(&theme.sanitized()).map_err(err)?;

    let conn = state.db.lock().unwrap();
    db::save_resume(&conn, &id, &name, &doc_json, &theme_json, &now_rfc3339()).map_err(err)?;
    Ok(id)
}

#[tauri::command(async)]
pub fn delete_resume(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let mut conn = state.db.lock().unwrap();
    db::delete_resume(&mut conn, &id).map_err(err)
}

#[tauri::command(async)]
pub fn set_default_resume(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let mut conn = state.db.lock().unwrap();
    db::set_default_resume(&mut conn, &id).map_err(err)
}

/// The overrides saved for one posting, if any.
#[tauri::command(async)]
pub fn get_resume_variant(
    state: State<'_, AppState>,
    url: String,
    resume_id: String,
) -> CmdResult<Option<resume::Variant>> {
    let conn = state.db.lock().unwrap();
    Ok(db::load_variant(&conn, &url, &resume_id)
        .map_err(err)?
        .and_then(|json| serde_json::from_str(&json).ok()))
}

#[tauri::command(async)]
pub fn save_resume_variant(
    state: State<'_, AppState>,
    url: String,
    resume_id: String,
    variant: resume::Variant,
) -> CmdResult<()> {
    let json = serde_json::to_string(&variant).map_err(err)?;
    let conn = state.db.lock().unwrap();
    db::save_variant(&conn, &url, &resume_id, &json, &now_rfc3339()).map_err(err)
}

/// Throws away a posting's overrides, putting it back to whatever the auto pass
/// decides.
#[tauri::command(async)]
pub fn clear_resume_variant(
    state: State<'_, AppState>,
    url: String,
    resume_id: String,
) -> CmdResult<()> {
    let conn = state.db.lock().unwrap();
    db::delete_variant(&conn, &url, &resume_id).map_err(err)
}

/// Everything needed to draw a résumé: the pages, and — with a `url` — the
/// tailoring against that posting.
///
/// One command rather than a layout call and a separate tailoring call, because
/// the two are not independent: the fit loop *is* the tailoring, and asking for
/// them apart would either lay the page out twice or let the panel disagree
/// with the page beside it about which bullets survived.
#[tauri::command(async)]
pub fn layout_resume(
    state: State<'_, AppState>,
    id: Option<String>,
    url: Option<String>,
    theme: Option<resume::Theme>,
) -> CmdResult<Option<ResumeView>> {
    let Some(row) = stored_resume(state.inner(), id.as_deref())? else {
        return Ok(None);
    };
    let (resume_id, _, doc, stored_theme, _) = parse_stored(row);
    Ok(Some(build_view(
        state.inner(),
        &resume_id,
        &doc,
        stored_theme,
        url.as_deref(),
        theme,
    )?))
}

/// The shared body of `layout_resume` and `render_resume_pdf`, so the file the
/// user saves is laid out by the same call that drew the preview they approved.
fn build_view(
    state: &AppState,
    resume_id: &str,
    doc: &resume::Resume,
    stored_theme: resume::Theme,
    url: Option<&str>,
    override_theme: Option<resume::Theme>,
) -> CmdResult<ResumeView> {
    let Some(url) = url else {
        // No posting: the builder's own preview, everything included.
        let theme = override_theme.unwrap_or(stored_theme).sanitized();
        let laid_out = resume::preview(doc, &theme);
        let selection = resume::tailor::everything(doc);
        return Ok(view_of(laid_out, selection, theme, doc, None));
    };

    let variant: resume::Variant = {
        let conn = state.db.lock().unwrap();
        db::load_variant(&conn, url, resume_id)
            .map_err(err)?
            .and_then(|json| serde_json::from_str(&json).ok())
            .unwrap_or_default()
    };

    // An explicit argument beats the variant, which beats the résumé's own
    // theme: the picker has to be able to preview a theme before it is saved.
    let theme = override_theme
        .or(variant.theme)
        .unwrap_or(stored_theme)
        .sanitized();

    let item = item_detail(state, url)?;
    let required = item
        .as_ref()
        .map(|i| i.required_skills.clone())
        .unwrap_or_default();
    let have = current_skills(state);
    let (laid_out, selection) = resume::tailored(doc, &required, &have, &variant, &theme);
    let company = item.as_ref().and_then(|i| i.company.clone());
    Ok(view_of(laid_out, selection, theme, doc, company.as_deref()))
}

fn view_of(
    laid_out: resume::layout::LaidOut,
    selection: resume::Selection,
    theme: resume::Theme,
    doc: &resume::Resume,
    company: Option<&str>,
) -> ResumeView {
    ResumeView {
        pages: laid_out.pages,
        fill: laid_out.fill,
        bullets: selection.bullets.iter().cloned().collect(),
        entries: selection.entries.iter().cloned().collect(),
        why: selection.why,
        dropped: selection.dropped,
        covered: selection.covered,
        required: selection.required,
        theme,
        filename: resume::suggested_filename(&doc.contact.name, company),
    }
}

/// The PDF itself.
///
/// Returns raw bytes through `tauri::ipc::Response` rather than a `Vec<u8>`:
/// the default serializer would turn a 60 KB file into a JSON array of 60,000
/// numbers, which is roughly a quarter of a megabyte of text to parse for a
/// file the webview is about to hand straight to a save dialog.
#[tauri::command(async)]
pub fn render_resume_pdf(
    state: State<'_, AppState>,
    id: Option<String>,
    url: Option<String>,
    theme: Option<resume::Theme>,
) -> CmdResult<tauri::ipc::Response> {
    let Some(row) = stored_resume(state.inner(), id.as_deref())? else {
        return Err("there is no résumé to render yet".into());
    };
    let (resume_id, _, doc, stored_theme, _) = parse_stored(row);
    let view = build_view(
        state.inner(),
        &resume_id,
        &doc,
        stored_theme,
        url.as_deref(),
        theme,
    )?;

    let title = match doc.contact.name.trim() {
        "" => "Résumé".to_string(),
        name => format!("{name} — Résumé"),
    };
    let laid_out = resume::layout::LaidOut {
        pages: view.pages,
        fill: view.fill,
    };
    Ok(tauri::ipc::Response::new(resume::pdf::render(
        &laid_out, &title,
    )))
}

/// Imports a `resume.json` as a new résumé and returns its id.
#[tauri::command(async)]
pub fn import_json_resume(
    state: State<'_, AppState>,
    json: String,
    name: Option<String>,
) -> CmdResult<String> {
    let mut doc = resume::jsonresume::import(&json)?;
    doc.normalize();

    let name = name
        .map(|n| n.trim().to_string())
        .filter(|n| !n.is_empty())
        .or_else(|| Some(doc.contact.name.trim().to_string()).filter(|n| !n.is_empty()))
        .unwrap_or_else(|| "Imported résumé".to_string());

    let id = resume::model::new_id();
    let doc_json = serde_json::to_string(&doc).map_err(err)?;
    let theme_json = serde_json::to_string(&resume::Theme::default()).map_err(err)?;
    let conn = state.db.lock().unwrap();
    db::save_resume(&conn, &id, &name, &doc_json, &theme_json, &now_rfc3339()).map_err(err)?;
    Ok(id)
}

/// Exports a résumé as a `resume.json` string.
#[tauri::command(async)]
pub fn export_json_resume(state: State<'_, AppState>, id: Option<String>) -> CmdResult<String> {
    let Some(row) = stored_resume(state.inner(), id.as_deref())? else {
        return Err("there is no résumé to export yet".into());
    };
    let (_, _, doc, _, _) = parse_stored(row);
    resume::jsonresume::export(&doc)
}

/// The themes the picker offers, named and pre-coloured. Rust-owned for the
/// same reason the skill catalog is: a theme the layout has no arm for would
/// sit in the picker and render as the default, which reads as a broken button.
#[tauri::command(async)]
pub fn list_resume_themes() -> Vec<ResumeThemeOption> {
    resume::theme::ThemeId::ALL
        .iter()
        .map(|id| ResumeThemeOption {
            id: *id,
            label: id.label().to_string(),
            theme: resume::Theme::preset(*id, resume::theme::DEFAULT_ACCENT),
        })
        .collect()
}

#[derive(Serialize)]
pub struct ResumeThemeOption {
    id: resume::theme::ThemeId,
    label: String,
    theme: resume::Theme,
}
