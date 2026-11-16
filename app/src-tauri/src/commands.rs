use std::sync::atomic::Ordering;
use std::sync::{Arc, Mutex};

use chrono::{DateTime, TimeZone, Utc};
use futures::stream::{self, StreamExt};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager, State};

use crate::db;
use crate::location;
use crate::models::{self, Item, ItemType};
use crate::prefs::{self, ScrapePrefs};
use crate::rank::{self, personalize_with, DEFAULT_MAJOR};
use crate::resume::{self, terms};
use crate::scrapers::{self, details};
use crate::skills::{self, SkillCategory};
use crate::sync::LockExt;
use crate::AppState;
use futures::FutureExt;

/// The old `/api/feed` endpoint capped its response at 60, which threw away
/// most of what the sources returned — the UI pages through the feed now, so
/// the cap only exists to bound the size of one `invoke` payload.
const FEED_LIMIT: usize = 400;
/// Matches the Python pipeline's 10-minute in-memory cache TTL.
const REFRESH_TTL_SECONDS: i64 = 600;
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
/// Opportunity rows the cache will hold. Ranking is about 1.2 s per 18,000
/// items and the hub payload grows with the count, so this is a ceiling on the
/// *work a read does*, not on what exists. Past it, rows are cut by
/// [`rank::fit`] — what the scrape alone can say about them — rather than by
/// age or by whichever source happened to be scraped last.
const OPPORTUNITY_CEILING: usize = 40_000;
/// How many last calls `get_last_call` hands back. The band on Today shows a
/// handful; the rest is there for a "see all", and this keeps the payload small
/// whatever the cache holds.
const LAST_CALL_LIMIT: usize = 24;
/// The ranked cache is also rebuilt when the hour turns. Time-dependent terms —
/// recency and above all how close a deadline is — are computed against a
/// clock, and a cache keyed only on data would show Tuesday's "closes in two
/// days" on Thursday until something else invalidated it.
const RANK_BUCKET_SECONDS: i64 = 3600;

/// Set once the reader has been shown the setup form and either answered it or
/// waved it away. Distinct from `db::ONBOARDED_KEY`, which says scraping may
/// begin: an upgrade is backfilled with that one (it was scraped before there
/// was a form) and has still never seen this.
const KEY_ONBOARDING_SEEN: &str = "onboarding_seen_at";
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
    /// "news" | "opportunities" | "events", so Settings can group the list the
    /// way the setup form asked the question.
    pub category: &'static str,
    /// Whether the reader's preferences let this source run at all.
    pub enabled: bool,
    pub last_ok_at: Option<String>,
    /// Why the last run failed — or, when `failing` is false, what a source
    /// that ran fine had to say about itself ("repository dormant").
    pub last_error: Option<String>,
    /// Whether the last run failed, as opposed to merely having a note.
    pub failing: bool,
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
    /// A really good posting that closes soon. `merit` itself stays out of the
    /// row: the verdict is all the list needs, and the number is relative to a
    /// set the list does not carry.
    #[serde(skip_serializing_if = "std::ops::Not::not")]
    last_call: bool,
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
            last_call: item.last_call,
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
    /// The hour this was ranked in; see [`RANK_BUCKET_SECONDS`].
    bucket: i64,
    /// The quality bars measured over `items`. Kept because a single item can
    /// only be judged against the set it came from — see `item_detail`.
    bars: rank::Bars,
    /// The instant the scores were computed against: the start of `bucket`,
    /// not the moment of the read, so a row and its detail pane agree.
    now: DateTime<Utc>,
    items: Arc<Vec<Item>>,
    /// When the pass finished, for [`Freshness::WhileScanning`].
    built: std::time::Instant,
}

/// Which hour a moment falls in.
pub(crate) fn rank_bucket(at: DateTime<Utc>) -> i64 {
    at.timestamp().div_euclid(RANK_BUCKET_SECONDS)
}

fn bucket_start(bucket: i64) -> DateTime<Utc> {
    Utc.timestamp_opt(bucket * RANK_BUCKET_SECONDS, 0)
        .single()
        .unwrap_or_else(Utc::now)
}

/// What a ranked read returns: the list, and what it was ranked against.
struct RankedView {
    items: Arc<Vec<Item>>,
    bars: rank::Bars,
    now: DateTime<Utc>,
}

/// Reads the persisted major, falling back to the default the old backend used
/// as its query-param default.
fn current_major(state: &AppState) -> String {
    let conn = state.db.lock_recover();
    db::get_setting(&conn, KEY_MAJOR)
        .ok()
        .flatten()
        .unwrap_or_else(|| DEFAULT_MAJOR.to_string())
}

/// The user's chosen interest tags. Stored as a JSON array in `settings`; an
/// unset or corrupt value means "no interests", which simply zeroes that term.
fn current_interests(state: &AppState) -> Vec<String> {
    let conn = state.db.lock_recover();
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
    let conn = state.db.lock_recover();
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
        let conn = state.db.lock_recover();
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
    let conn = state.db.lock_recover();
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
    let conn = state.db.lock_recover();
    let states = db::load_states(&conn).map_err(err)?;
    let details = db::load_details(&conn).map_err(err)?;
    let mut items: Vec<Item> = db::load_items(&conn)
        .map_err(err)?
        .into_iter()
        .filter(|i| !states.dismissed.contains(&i.url))
        .collect();
    db::annotate(&mut items, &states);
    // After `annotate`, which is what says a row is saved and must stay.
    scrapers::hide_mirrored(&mut items);
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

/// The visible cache, scored and ordered, recomputed when the data or the
/// profile has moved under it — or the hour has turned. For the list commands.
fn ranked_items(state: &AppState, major: Option<String>) -> CmdResult<Arc<Vec<Item>>> {
    Ok(ranked_view(state, major, Freshness::for_list(state))?.items)
}

/// How out of date a ranked cache a reader will accept.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Freshness {
    /// Same data and same hour. The default for the list commands.
    Exact,
    /// A scrape is writing: every batch moves the generation, and an exact
    /// answer would be a full ranking pass per batch. A cache a few seconds
    /// old is accepted instead, so the list refreshes at the pace the
    /// frontend polls and not at the pace the sources finish.
    WhileScanning,
    /// Any cache for this profile. For the detail pane and the enrichment
    /// queue, which are about one row or about order, not about the list.
    Any,
}

/// How old a cache a list read accepts while a scrape is running.
const SCAN_STALENESS: std::time::Duration = std::time::Duration::from_secs(4);

impl Freshness {
    fn for_list(state: &AppState) -> Freshness {
        if state.refreshing.load(Ordering::SeqCst) {
            Freshness::WhileScanning
        } else {
            Freshness::Exact
        }
    }
}

/// [`ranked_items`] plus what it was ranked against.
///
/// `freshness` says how old a cache is good enough. The list commands want the
/// answer that matches the data and the clock; the detail pane and the
/// enrichment queue do not, and must not pay for it: the first click after the
/// hour turned — or after a scrape wrote a batch — would otherwise stall for
/// the whole ranking pass to show a posting that is already on screen.
fn ranked_view(
    state: &AppState,
    major: Option<String>,
    freshness: Freshness,
) -> CmdResult<RankedView> {
    // Read before taking the cache lock, so a write that lands mid-rank leaves
    // a generation the *next* read will not accept rather than one it will.
    let generation = state.generation.load(Ordering::SeqCst);
    let profile = current_profile(state, major);
    let bucket = rank_bucket(Utc::now());

    // Held across the ranking pass on purpose: `loadAll()` fires several
    // commands at once and they run on separate threads, so without this the
    // feed and the hub would each start their own identical 1.2 s pass.
    let mut slot = state.ranked.lock_recover();
    if let Some(cached) = slot.as_ref() {
        let acceptable = cached.profile == profile
            && match freshness {
                Freshness::Exact => cached.generation == generation && cached.bucket == bucket,
                Freshness::WhileScanning => {
                    (cached.generation == generation && cached.bucket == bucket)
                        || cached.built.elapsed() < SCAN_STALENESS
                }
                Freshness::Any => true,
            };
        if acceptable {
            return Ok(RankedView {
                items: Arc::clone(&cached.items),
                bars: cached.bars,
                now: cached.now,
            });
        }
    }

    let now = bucket_start(bucket);
    let (mut items, bars) = personalize_with(visible_items(state)?, &profile, now, None);
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
        bucket,
        bars,
        now,
        items: Arc::clone(&items),
        built: std::time::Instant::now(),
    });
    Ok(RankedView { items, bars, now })
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

/// The really good postings that are about to close, soonest first.
///
/// Its own command because `get_feed` is the top 400 by score and a last call
/// is not guaranteed to be in it — a strong posting closing tomorrow is exactly
/// the kind that sits at 450. The ranker has already decided which rows
/// qualify ([`Item::last_call`]); this orders them by what the reader needs to
/// act on, which is the date, and caps the answer in Rust so the payload does
/// not depend on how big the cache is.
#[tauri::command(async)]
pub fn get_last_call(state: State<'_, AppState>, major: Option<String>) -> CmdResult<Payload> {
    let ranked = ranked_items(&state, major)?;
    serde_json::value::to_raw_value(&as_list(last_calls(&ranked))).map_err(err)
}

/// The flagged rows in the order a reader should act on them, capped. Pure, so
/// the ordering is testable without a running app.
fn last_calls(items: &[Item]) -> Vec<&Item> {
    let mut hits: Vec<&Item> = items.iter().filter(|i| i.last_call).collect();
    hits.sort_by(|a, b| {
        a.closes_at
            .cmp(&b.closes_at)
            .then(b.merit.unwrap_or(0.0).total_cmp(&a.merit.unwrap_or(0.0)))
    });
    hits.truncate(LAST_CALL_LIMIT);
    hits
}

/// Everything the user starred, newest first. Served from the `item_state`
/// snapshots, so it works even for listings the cache has since pruned.
#[tauri::command(async)]
pub fn get_saved(state: State<'_, AppState>) -> CmdResult<Vec<Item>> {
    let mut items = {
        let conn = state.db.lock_recover();
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
        // Both were judged against the set at the moment of starring, which is
        // not the set the reader is looking at now.
        item.merit = None;
        item.last_call = false;
    }
    Ok(items)
}

/// For each search term, the postings whose fetched description has a word
/// starting with it. Roles ANDs these with what each row itself carries, so a
/// word that only appears in the body of a posting still finds it. See
/// `db::search_details` for why the answer is per term.
#[tauri::command(async)]
pub fn search_descriptions(
    state: State<'_, AppState>,
    terms: Vec<String>,
) -> CmdResult<std::collections::HashMap<String, Vec<String>>> {
    // A query is a handful of words; anything longer is not one.
    let terms: Vec<String> = terms.into_iter().take(8).collect();
    let conn = state.db.lock_recover();
    db::search_details(&conn, &terms).map_err(err)
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
    // Any age: the pane is showing a row the list already drew, and rebuilding
    // for the hour boundary here would put the whole ranking pass behind a click.
    let view = ranked_view(state, None, Freshness::Any)?;
    let cached = view.items.iter().find(|i| i.url == url).cloned();

    if let Some(id) = url.strip_prefix(PASTED_PREFIX) {
        let conn = state.db.lock_recover();
        let Some(posting) = db::load_pasted(&conn, id).map_err(err)? else {
            return Ok(None);
        };
        drop(conn);
        let profile = current_profile(state, None);
        let item = pasted_item(&posting, &url);
        return Ok(
            personalize_with(vec![item], &profile, view.now, Some(&view.bars))
                .0
                .into_iter()
                .next(),
        );
    }

    let mut items = {
        let conn = state.db.lock_recover();
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
    //
    // Against the set's own bars and clock, not a fresh measure of one item: a
    // percentile of one is meaningless, and the pane would disagree with the
    // row it was opened from about whether the posting is a last call.
    let profile = current_profile(state, None);
    items = personalize_with(items, &profile, view.now, Some(&view.bars)).0;
    Ok(items.pop())
}

#[tauri::command(async)]
pub fn feed_status(state: State<'_, AppState>) -> CmdResult<FeedStatus> {
    // Counted in SQL rather than by materialising the cache: this command runs
    // after every save and every dismissal, and loading 18,240 rows to call
    // `.len()` on them cost 38 ms and several megabytes each time.
    let (item_count, counts, saved_count, dismissed_count) = {
        let conn = state.db.lock_recover();
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
    // than vanishing from the list. `sources()` is already one entry per name:
    // some are registered as several passes over one site.
    let prefs = current_prefs(&state);
    let runs: std::collections::HashMap<String, db::SourceRun> = {
        let conn = state.db.lock_recover();
        db::load_source_runs(&conn)
            .map_err(err)?
            .into_iter()
            .map(|r| (r.source.clone(), r))
            .collect()
    };
    let sources = scrapers::sources()
        .into_iter()
        .map(|(category, name)| {
            let run = runs.get(name);
            SourceHealth {
                name: name.to_string(),
                count: counts.get(name).copied().unwrap_or(0),
                category: category.as_str(),
                enabled: prefs.source_enabled(category, name),
                last_ok_at: run.and_then(|r| r.last_ok_at).map(|d| d.to_rfc3339()),
                last_error: run.and_then(|r| r.last_error.clone()),
                failing: run.is_some_and(|r| !r.ok),
            }
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

/// Clears the single-flight flag and tells the UI, however a refresh ends. A
/// `?` between taking the flag and releasing it used to leave `refreshing`
/// true until the app was restarted, which made every later Refresh a no-op.
struct RefreshGuard(AppHandle);

impl Drop for RefreshGuard {
    fn drop(&mut self) {
        let state = self.0.state::<AppState>();
        state.refreshing.store(false, Ordering::SeqCst);
        let _ = self.0.emit("feed:refreshing", false);
        let _ = self.0.emit("feed:phase", "idle");
    }
}

/// One source finishing, as the setup form's progress list sees it.
#[derive(Serialize, Clone)]
struct Progress {
    source: &'static str,
    category: &'static str,
    /// "ok" | "dormant" | "failed"
    status: &'static str,
    count: usize,
    /// The failure, or the note a dormant source left.
    message: Option<String>,
    done: usize,
    total: usize,
}

impl Progress {
    fn of(outcome: &scrapers::SourceOutcome, done: usize, total: usize) -> Progress {
        let (status, message) = match &outcome.health {
            scrapers::Health::Ok => ("ok", None),
            scrapers::Health::Dormant(note) => ("dormant", Some(note.clone())),
            scrapers::Health::Failed(e) => ("failed", Some(e.clone())),
        };
        Progress {
            source: outcome.source,
            category: outcome.category.as_str(),
            status,
            count: outcome.count,
            message,
            done,
            total,
        }
    }
}

fn onboarded(state: &AppState) -> bool {
    let conn = state.db.lock_recover();
    db::get_setting(&conn, db::ONBOARDED_KEY)
        .ok()
        .flatten()
        .is_some()
}

/// What a refresh did, for the UI. `status` tells "nothing ran" (not asked yet,
/// or another refresh was already under way) from a run that finished and a
/// run the reader stopped, which the old `Option<usize>` could not.
#[derive(Serialize, Clone, Default)]
pub struct RefreshOutcome {
    /// "skipped" | "ran" | "stopped"
    pub status: &'static str,
    pub sources_done: usize,
    pub sources_total: usize,
    /// Rows written by this run's scrape, before the description phase.
    pub rows: usize,
}

/// Stopping a scrape, which is two things: a flag that says "do not start the
/// next phase", and the handle that abandons the one in flight. Both are
/// needed — a stop that lands between phases has nothing to abort and must
/// still be honoured.
#[derive(Default)]
pub(crate) struct ScanControl {
    stop: std::sync::atomic::AtomicBool,
    handle: Mutex<Option<futures::future::AbortHandle>>,
}

impl ScanControl {
    /// A new run: forget any earlier stop.
    fn begin(&self) {
        self.stop.store(false, Ordering::SeqCst);
        *self.handle.lock_recover() = None;
    }

    /// Registers the phase about to run. A stop that already arrived aborts it
    /// at once, so the request cannot be lost in the gap between phases.
    fn arm(&self, handle: futures::future::AbortHandle) {
        let mut slot = self.handle.lock_recover();
        if self.stop.load(Ordering::SeqCst) {
            handle.abort();
        }
        *slot = Some(handle);
    }

    fn disarm(&self) {
        *self.handle.lock_recover() = None;
    }

    fn request_stop(&self) {
        self.stop.store(true, Ordering::SeqCst);
        if let Some(handle) = self.handle.lock_recover().as_ref() {
            handle.abort();
        }
    }

    fn stop_requested(&self) -> bool {
        self.stop.load(Ordering::SeqCst)
    }
}

/// Stops the running refresh, whichever phase it is in. Everything already
/// written stays; sources that had not finished stay due, so the next Start
/// picks up exactly where this one left off. A no-op when nothing is running.
#[tauri::command]
pub fn stop_refresh(app: AppHandle) -> bool {
    let state = app.state::<AppState>();
    if !state.refreshing.load(Ordering::SeqCst) {
        return false;
    }
    state.scan.request_stop();
    let _ = app.emit("feed:phase", "stopping");
    true
}

/// What the scrape phase produced.
#[derive(Default)]
struct ScrapeRun {
    rows: usize,
    done: usize,
    outcomes: Vec<scrapers::SourceOutcome>,
    /// Scopes of the sources that finished well, for the sweep.
    alive: std::collections::HashSet<db::Scope>,
    stopped: bool,
}

/// Runs the producer (the sources) and the consumer (the writes) together.
///
/// Each source's rows are written as they arrive, so the feed fills while the
/// scrape is still going. The producer is wrapped in `Abortable`: stopping
/// drops it, which abandons every request in flight, and the channel closes.
/// The consumer is *not* abortable — it drains whatever was already sent, so a
/// stop never cancels a write halfway.
async fn scrape(app: &AppHandle, plan: Vec<&'static str>, run_start: DateTime<Utc>) -> ScrapeRun {
    use futures::future::{AbortHandle, Abortable};
    use std::collections::{HashMap, HashSet};

    let state = app.state::<AppState>();
    let total = plan.len();
    let (tx, mut rx) = futures::channel::mpsc::unbounded();
    let (handle, registration) = AbortHandle::new_pair();
    state.scan.arm(handle);

    let producer = Abortable::new(
        async {
            scrapers::fetch_stream(&plan, tx).await;
        },
        registration,
    );

    let consumer = async {
        let mut run = ScrapeRun::default();
        let mut alive: HashMap<&'static str, HashSet<db::Scope>> = HashMap::new();
        while let Some(msg) = rx.next().await {
            match msg {
                scrapers::Msg::Batch { source, items } => {
                    let result = {
                        let app = app.clone();
                        tauri::async_runtime::spawn_blocking(move || {
                            let state = app.state::<AppState>();
                            persist_batch(state.inner(), items, run_start)
                        })
                        .await
                    };
                    match result {
                        Ok(Ok(saved)) => {
                            run.rows += saved.rows;
                            let trimmer = app.clone();
                            let _ = tauri::async_runtime::spawn_blocking(move || {
                                let state = trimmer.state::<AppState>();
                                trim_when_over(
                                    state.inner(),
                                    (OPPORTUNITY_CEILING as f64 * SCAN_SLACK) as usize,
                                )
                            })
                            .await;
                            alive.entry(source).or_default().extend(saved.alive);
                            let _ = app.emit("feed:updated", run.rows);
                        }
                        Ok(Err(e)) => log::warn!("{source}: could not store rows: {e}"),
                        Err(e) => log::warn!("{source}: store task failed: {e}"),
                    }
                }
                scrapers::Msg::Done(outcome) => {
                    if matches!(outcome.health, scrapers::Health::Failed(_)) {
                        // Its rows stay, but it vouches for nothing: a source
                        // that errored part-way has not shown what is gone.
                        alive.remove(outcome.source);
                    }
                    {
                        let state = app.state::<AppState>();
                        let mut conn = state.db.lock_recover();
                        if let Err(e) = db::record_runs(&mut conn, &[run_of(&outcome, Utc::now())])
                        {
                            log::warn!("{}: could not record the run: {e}", outcome.source);
                        }
                    }
                    run.done += 1;
                    let _ = app.emit("feed:progress", Progress::of(&outcome, run.done, total));
                    run.outcomes.push(outcome);
                }
            }
        }
        (run, alive)
    };

    let (aborted, (mut run, alive)) = futures::join!(producer, consumer);
    state.scan.disarm();
    run.stopped = aborted.is_err();
    // A stopped run vouches for nothing. A posting listed by two sources may
    // have been seen only by the one that did not get to run, and a miss
    // charged for that would be a miss the posting did not earn.
    if !run.stopped {
        run.alive = alive.into_values().flatten().collect();
    }
    run
}

/// Scrapes what the reader asked for and what is due, writing each source's
/// rows as it finishes, and then reads the descriptions of the best of what is
/// stored. Everything is stoppable ([`stop_refresh`]).
///
/// `force` is the Refresh button: it overrides how recently a source ran, and
/// nothing else. A source the reader switched off stays off.
#[tauri::command]
pub async fn refresh(app: AppHandle, force: bool) -> CmdResult<RefreshOutcome> {
    let state = app.state::<AppState>();

    // The app asks before it fetches. The launch refresh and every button hit
    // this same gate, so there is no path to a request that skips the question.
    if !onboarded(&state) {
        return Ok(RefreshOutcome {
            status: "skipped",
            ..Default::default()
        });
    }
    // Two windows, or a manual tap landing on top of the launch refresh,
    // should not scrape twice.
    if state
        .refreshing
        .compare_exchange(false, true, Ordering::SeqCst, Ordering::SeqCst)
        .is_err()
    {
        return Ok(RefreshOutcome {
            status: "skipped",
            ..Default::default()
        });
    }
    let _release = RefreshGuard(app.clone());
    state.scan.begin();
    // Announced whether or not anything is due: an enrichment-only run is still
    // a run, and the reader should be able to see it and stop it.
    let _ = app.emit("feed:refreshing", true);
    let _ = app.emit("feed:phase", "scraping");

    // Boards the cache has taught us, and the ones that have given up, before
    // the plan: the scrapers read them when they start.
    load_boards(&state);

    let plan = {
        let prefs = current_prefs(&state);
        let runs = {
            let conn = state.db.lock_recover();
            db::load_source_runs(&conn).map_err(err)?
        };
        prefs::due(&scrapers::sources(), &runs, &prefs, Utc::now(), force)
    };

    let _ = app.emit("feed:plan", plan.len());
    let mut outcome = RefreshOutcome {
        status: "ran",
        sources_total: plan.len(),
        ..Default::default()
    };

    if !plan.is_empty() {
        let run_start = Utc::now();
        let run = scrape(&app, plan, run_start).await;
        outcome.sources_done = run.done;
        outcome.rows = run.rows;

        // Every source that ran failed: that is the network, not an empty
        // world, so keep the cached items rather than blanking the feed.
        if !run.stopped && all_failed(&run.outcomes, run.rows) {
            // The network, not the boards: nothing they said counts.
            scrapers::boards::discard_reports();
            return Err("every source failed (offline?)".to_string());
        }

        let stopped = run.stopped;
        let finalized = {
            let app = app.clone();
            tauri::async_runtime::spawn_blocking(move || {
                let state = app.state::<AppState>();
                finalize(state.inner(), run_start, &run.alive, !run.stopped)
            })
            .await
            .map_err(err)?
        };
        if let Err(e) = finalized {
            log::warn!("end-of-run housekeeping failed: {e}");
        }
        let _ = app.emit("feed:updated", outcome.rows);
        if stopped {
            outcome.status = "stopped";
        }
    }

    // Worth doing whenever there is a cache to enrich, *including* when nothing
    // was due: the backlog is thousands of postings and 600 are fetched a go,
    // so a launch inside the freshness window is exactly when to make progress
    // on it. Deliberately inside the `refreshing` flag so a second refresh
    // cannot start on top of it. This phase emits its own updates as it lands.
    if !state.scan.stop_requested() {
        let _ = app.emit("feed:phase", "details");
        match enrich_details(&app).await {
            Ok(0) => {}
            Ok(n) => log::info!("fetched {n} job descriptions"),
            // Enrichment is an enhancement; a failure leaves the scrape intact.
            Err(e) => log::warn!("description enrichment failed: {e}"),
        }
    }
    if state.scan.stop_requested() {
        outcome.status = "stopped";
    }
    Ok(outcome)
}

/// Which postings to fetch descriptions for, in the order to fetch them: the
/// not-yet-attempted opportunities, best first.
///
/// "Best" is `merit` — what a posting is *worth* — and not where the feed puts
/// it. Rank order includes recency, and postings are kept for as long as they
/// are open, so the old but excellent ones would be fetched last; a deadline is
/// only ever learned by fetching, which makes those the rows a "last call" most
/// needs read. Postings already closed are skipped: nobody applies to them, and
/// their description is a request spent on the past.
///
/// The result is interleaved by employer, as the feed is, so one Workday
/// tenant's backlog cannot take the whole budget. `diversify` orders its rounds
/// by `relevance_score`, so the copies it is handed carry merit there.
pub fn detail_candidates(
    items: &[Item],
    attempted: &std::collections::HashSet<String>,
    now: DateTime<Utc>,
) -> Vec<(String, Option<String>)> {
    let mut pending: Vec<&Item> = items
        .iter()
        .filter(|i| i.item_type.is_opportunity() && !attempted.contains(&i.url))
        .filter(|i| i.closes_at.is_none_or(|c| c >= now))
        .collect();
    pending.sort_by(|a, b| {
        let key = |i: &Item| (i.merit.unwrap_or(0.0), i.relevance_score.unwrap_or(0.0));
        let (a, b) = (key(a), key(b));
        b.0.total_cmp(&a.0).then(b.1.total_cmp(&a.1))
    });
    // Capped before the clone below, which is the expensive part.
    pending.truncate(DETAIL_CANDIDATES);
    let queue: Vec<Item> = pending
        .into_iter()
        .map(|i| {
            let mut copy = i.clone();
            copy.relevance_score = copy.merit;
            copy
        })
        .collect();
    rank::diversify(queue)
        .into_iter()
        .map(|i| (i.url, i.simplify_id))
        .collect()
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
                let conn = state.db.lock_recover();
                db::urls_with_details(&conn).map_err(err)?
            };
            let view = ranked_view(&state, None, Freshness::Any)?;
            Ok(detail_candidates(&view.items, &attempted, Utc::now()))
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
    // Postings already queued per paced handler (`Handler::per_pass`).
    let mut paced: std::collections::HashMap<&'static str, usize> =
        std::collections::HashMap::new();
    for (url, simplify_id) in candidates {
        let chain = details::route(&url, simplify_id.as_deref());
        if chain.is_empty() {
            unsupported.push((
                url,
                models::JobDetail::with_status(models::DetailStatus::Unsupported),
            ));
        } else if queue.len() < DETAIL_BUDGET {
            if let Some(cap) = chain[0].per_pass() {
                let taken = paced.entry(chain[0].name()).or_default();
                if *taken >= cap {
                    continue;
                }
                *taken += 1;
            }
            queue.push((url, chain));
        }
    }

    if !unsupported.is_empty() {
        let mut conn = state.db.lock_recover();
        db::save_details(&mut conn, &unsupported).map_err(err)?;
    }
    if queue.is_empty() {
        return Ok(unsupported.len());
    }

    let client = scrapers::build_client().map_err(err)?;
    // `buffer_unordered` rather than `join_all`: the queue is 200 URLs and
    // opening 200 connections at once is neither polite nor faster. Note this
    // avoids `tokio::sync::Semaphore`, whose feature is not enabled here.
    //
    // Fetched details are written in small groups as they arrive rather than
    // all at the end: a stop abandons the requests in flight, and everything
    // read so far is already stored.
    use futures::future::{AbortHandle, Abortable};
    let (tx, mut rx) = futures::channel::mpsc::unbounded::<(String, models::JobDetail)>();
    let (handle, registration) = AbortHandle::new_pair();
    state.scan.arm(handle);
    let producer = Abortable::new(
        async move {
            stream::iter(queue)
                .map(|(url, chain)| {
                    let client = client.clone();
                    async move {
                        // A handler that panics on one page fails that page,
                        // as a network error would — not the whole pass.
                        let detail = std::panic::AssertUnwindSafe(details::fetch(&client, &chain))
                            .catch_unwind()
                            .await
                            .unwrap_or_else(|panic| {
                                log::error!(
                                    "detail fetch for {url} panicked: {}",
                                    scrapers::panic_message(&panic)
                                );
                                models::JobDetail::with_status(models::DetailStatus::Failed)
                            });
                        (url, detail)
                    }
                })
                .buffer_unordered(DETAIL_CONCURRENCY)
                .for_each(|fetched| {
                    let _ = tx.unbounded_send(fetched);
                    futures::future::ready(())
                })
                .await;
        },
        registration,
    );
    let consumer = async {
        let mut written = 0usize;
        let mut group: Vec<(String, models::JobDetail)> = Vec::new();
        while let Some(fetched) = rx.next().await {
            group.push(fetched);
            if group.len() >= DETAIL_FLUSH {
                written += flush_details(app, std::mem::take(&mut group)).await?;
            }
        }
        if !group.is_empty() {
            written += flush_details(app, group).await?;
        }
        CmdResult::Ok(written)
    };
    let (_, written) = futures::join!(producer, consumer);
    state.scan.disarm();
    let written = written? + unsupported.len();

    // The skill memo is keyed on URL and would otherwise keep serving the
    // answer it computed before these descriptions existed.
    skills::clear_memo();
    invalidate(&state);
    Ok(written)
}

/// Details written per transaction while a description pass is running.
const DETAIL_FLUSH: usize = 50;

async fn flush_details(
    app: &AppHandle,
    group: Vec<(String, models::JobDetail)>,
) -> CmdResult<usize> {
    let handle = app.clone();
    let written = tauri::async_runtime::spawn_blocking(move || -> CmdResult<usize> {
        let state = handle.state::<AppState>();
        let mut conn = state.db.lock_recover();
        let n = db::save_details(&mut conn, &group).map_err(err)?;
        drop(conn);
        invalidate(state.inner());
        Ok(n)
    })
    .await
    .map_err(err)??;
    let _ = app.emit("feed:updated", written);
    Ok(written)
}

/// The ledger row for one source finishing.
fn run_of(o: &scrapers::SourceOutcome, now: DateTime<Utc>) -> db::SourceRun {
    let failed = matches!(o.health, scrapers::Health::Failed(_));
    db::SourceRun {
        source: o.source.to_string(),
        category: o.category.as_str().to_string(),
        last_run_at: now,
        ok: !failed,
        last_ok_at: (!failed).then_some(now),
        last_count: o.count as i64,
        last_error: match &o.health {
            scrapers::Health::Ok => None,
            scrapers::Health::Dormant(note) => Some(note.clone()),
            scrapers::Health::Failed(e) => Some(e.clone()),
        },
    }
}

/// Every source that ran failed, and nothing was stored. A source that ran and
/// had nothing to say is not a failure — a reader who wants only internships
/// can legitimately get no rows back from a run.
fn all_failed(outcomes: &[scrapers::SourceOutcome], rows: usize) -> bool {
    rows == 0
        && !outcomes.is_empty()
        && outcomes
            .iter()
            .all(|o| matches!(o.health, scrapers::Health::Failed(_)))
}

/// Rows written per lock hold, so a reader never waits on the whole of a
/// large batch.
const WRITE_CHUNK: usize = 500;

struct SavedBatch {
    rows: usize,
    /// The scopes this batch vouches for, once its source finishes well.
    alive: std::collections::HashSet<db::Scope>,
}

/// Stores one piece of a scrape: dedupe within it, drop what the reader does
/// not want, upsert, and keep any description the source sent along.
///
/// **Never sweeps.** Whether a posting is gone is a question about the whole
/// run — a URL two sources list is owned by one of them, and sweeping after
/// the first would charge it a miss the second was about to clear — so the
/// sweep waits for [`finalize`]. Split out so the connection lock is never held
/// across an await point.
fn persist_batch(
    state: &AppState,
    items: Vec<Item>,
    run_start: DateTime<Utc>,
) -> CmdResult<SavedBatch> {
    let prefs = current_prefs(state);
    let items: Vec<Item> = scrapers::finish(items)
        .into_iter()
        .filter(|i| prefs.admits(i))
        // An employer's board is everything it has open. Keep what this app is
        // for: early-career in any field, technical at any level.
        .filter(|i| !scrapers::reads_whole_boards(&i.source_platform) || rank::worth_storing(i))
        .collect();
    let alive = scrapers::alive_scopes(&items);

    // Lever and Ashby return the whole posting in the same response as the
    // listing. Those descriptions are harvested here rather than left on the
    // enrichment queue — several thousand requests for text already in hand —
    // and they go into `job_details`, not onto the row: `save_items`
    // overwrites `content_text` every refresh and would erase them.
    let seeded = |chunk: &[Item]| -> Vec<(String, models::JobDetail)> {
        chunk
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
            .collect()
    };

    for chunk in items.chunks(WRITE_CHUNK) {
        let details = seeded(chunk);
        let mut conn = state.db.lock_recover();
        db::save_items_ranked(&mut conn, chunk, run_start, scrapers::source_order())
            .map_err(err)?;
        if !details.is_empty() {
            db::save_details(&mut conn, &details).map_err(err)?;
        }
    }
    invalidate(state);
    Ok(SavedBatch {
        rows: items.len(),
        alive,
    })
}

/// The end of a run, once, whether it finished or was stopped: retention, the
/// ceiling, the housekeeping that returns space to the disk.
///
/// `alive` is empty for a stopped run, so only what the calendar decides —
/// a deadline long past, an event over — is removed and no posting is charged
/// a miss for a source that never got to answer.
fn finalize(
    state: &AppState,
    run_start: DateTime<Utc>,
    alive: &std::collections::HashSet<db::Scope>,
    complete: bool,
) -> CmdResult<()> {
    trim_to_ceiling(state)?;
    if let Err(e) = learn_boards(state) {
        log::warn!("could not update the board list: {e}");
    }
    // A re-scrape can change the text behind a URL we have already extracted
    // skills for, so the memo has to go with it.
    skills::clear_memo();

    let mut conn = state.db.lock_recover();
    // Postings leave when they stop being offered, not when they get old: see
    // `db::sweep`. Run even when this scrape returned no rows: what closes,
    // ends or ages out does so by the date, not by what was fetched.
    let swept = db::sweep(
        &mut conn,
        Utc::now(),
        run_start,
        alive,
        scrapers::START_DATED,
        &db::Retention::default(),
    )
    .map_err(err)?;
    log::info!("sweep: {swept:?}");
    // Runs after the sweep so seen/dismissed rows for listings that have gone
    // go with them. Saved rows are kept regardless.
    db::prune_orphan_states(&conn).map_err(err)?;
    db::prune_orphan_details(&conn).map_err(err)?;
    // The sweep and the two prunes above are the only bulk deletes the app does, and a
    // delete on its own just moves pages onto the freelist — the file stays as
    // large as it has ever been. This is where the space actually goes back,
    // and it is deliberately the last thing after them rather than a step of
    // its own: the pages have to be free before there is anything to return.
    db::incremental_vacuum(&conn).map_err(err)?;
    if complete {
        db::set_setting(&conn, KEY_LAST_REFRESH, &Utc::now().to_rfc3339()).map_err(err)?;
    }
    drop(conn);
    invalidate(state);
    Ok(())
}

/// Reads the learned boards and the health records out of `settings` and hands
/// them to the scrapers.
fn load_boards(state: &AppState) {
    let learned: Vec<scrapers::boards::Found> =
        json_setting(state, scrapers::boards::KEY_DISCOVERED);
    let health: std::collections::HashMap<String, scrapers::boards::BoardHealth> =
        json_setting(state, scrapers::boards::KEY_HEALTH);
    scrapers::boards::set_learned(&learned);
    scrapers::boards::set_skipped(&health, Utc::now());
    // Whatever a run that never filed its answers left behind is not evidence.
    scrapers::boards::discard_reports();
}

/// At the end of a run: file what each board reported, and read the boards
/// out of every posting URL now in the cache. The next refresh fetches them.
fn learn_boards(state: &AppState) -> CmdResult<()> {
    use scrapers::boards::{self, BoardHealth, Found};
    let now = Utc::now();
    // Read under the lock, worked out without it: the URL pass is tens of
    // thousands of strings and a reader must not wait behind it.
    let (stored_health, stored_learned, urls) = {
        let conn = state.db.lock_recover();
        (
            db::get_setting(&conn, boards::KEY_HEALTH).ok().flatten(),
            db::get_setting(&conn, boards::KEY_DISCOVERED)
                .ok()
                .flatten(),
            db::opportunity_urls(&conn).map_err(err)?,
        )
    };
    let mut health: std::collections::HashMap<String, BoardHealth> = stored_health
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();
    let mut learned: Vec<Found> = stored_learned
        .and_then(|s| serde_json::from_str(&s).ok())
        .unwrap_or_default();

    boards::record_reports(&mut health, now);
    let fresh = boards::discover(urls.iter().map(String::as_str), &learned);
    if !fresh.is_empty() {
        log::info!("learned {} new boards from the cache", fresh.len());
    }
    learned.extend(fresh);
    learned.truncate(boards::MAX_DISCOVERED);
    // A board nobody has tried in a month has no use for its record.
    health.retain(|_, h| {
        h.last_try
            .is_some_and(|at| now - at < chrono::Duration::days(30))
    });

    {
        let conn = state.db.lock_recover();
        db::set_setting(
            &conn,
            boards::KEY_HEALTH,
            &serde_json::to_string(&health).map_err(err)?,
        )
        .map_err(err)?;
        db::set_setting(
            &conn,
            boards::KEY_DISCOVERED,
            &serde_json::to_string(&learned).map_err(err)?,
        )
        .map_err(err)?;
    }
    boards::set_learned(&learned);
    boards::set_skipped(&health, now);
    Ok(())
}

/// Holds the opportunity count to [`OPPORTUNITY_CEILING`], dropping the rows
/// the scrape alone says least about ([`rank::fit`]) rather than the oldest or
/// whichever source happened to be written last. Starred postings are never
/// dropped.
fn trim_to_ceiling(state: &AppState) -> CmdResult<()> {
    trim_when_over(state, OPPORTUNITY_CEILING)
}

/// While a scrape is running the store can hold far more than the ceiling — the
/// ceiling is applied at the end, over the whole run — and measured against the
/// real app it peaked near fifty thousand roles, with a hub payload to match.
/// So a piece that takes the store past a tenth over the ceiling trims it back,
/// by the same measure, rather than waiting for the end.
const SCAN_SLACK: f64 = 1.1;

fn trim_when_over(state: &AppState, limit: usize) -> CmdResult<()> {
    let over = {
        let conn = state.db.lock_recover();
        db::opportunity_count(&conn).map_err(err)? > limit
    };
    if !over {
        return Ok(());
    }
    let profile = current_profile(state, None);
    // The read and the scoring are the long part; the lock is held for neither
    // the scoring nor the delete's preparation, so readers are not queued behind
    // a pass over fifty thousand rows.
    let all = {
        let conn = state.db.lock_recover();
        db::load_items(&conn).map_err(err)?
    };
    let every: Vec<String> = all.iter().map(|i| i.url.clone()).collect();
    let kept: std::collections::HashSet<String> =
        rank::cap_opportunities(all, &profile, OPPORTUNITY_CEILING)
            .into_iter()
            .map(|i| i.url)
            .collect();
    let dropped: Vec<String> = every
        .into_iter()
        .filter(|url| !kept.contains(url))
        .collect();
    let mut conn = state.db.lock_recover();
    let n = db::delete_unsaved(&mut conn, &dropped).map_err(err)?;
    log::info!("ceiling: dropped {n} lowest-fit postings");
    Ok(())
}

/// One scrape stored in a single call: the pieces and the end of the run
/// together. What the tests exercise, and what the batch path does in steps.
#[cfg(test)]
fn persist(
    state: &AppState,
    items: Vec<Item>,
    outcomes: Vec<scrapers::SourceOutcome>,
) -> CmdResult<usize> {
    let run_start = Utc::now();
    let runs: Vec<db::SourceRun> = outcomes.iter().map(|o| run_of(o, run_start)).collect();
    {
        let mut conn = state.db.lock_recover();
        db::record_runs(&mut conn, &runs).map_err(err)?;
    }
    if all_failed(&outcomes, items.len()) {
        return Err("every source failed (offline?)".to_string());
    }
    let saved = persist_batch(state, items, run_start)?;
    finalize(state, run_start, &saved.alive, true)?;
    Ok(saved.rows)
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
        let conn = state.db.lock_recover();
        db::set_saved(&conn, &url, saved, item.as_ref()).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

/// Opens the folder the log files are written to, for a bug report. From
/// Rust rather than the webview so the opener's path scope does not have to
/// grant the UI a directory outside anything it otherwise touches.
#[tauri::command(async)]
pub fn open_log_dir(app: AppHandle) -> CmdResult<()> {
    use tauri_plugin_opener::OpenerExt;
    let dir = app.path().app_log_dir().map_err(err)?;
    std::fs::create_dir_all(&dir).map_err(err)?;
    app.opener()
        .open_path(dir.to_string_lossy(), None::<&str>)
        .map_err(err)
}

/// Records where an application to a saved role stands, and a note on it.
/// Does not touch the ranking, so nothing is invalidated.
#[tauri::command(async)]
pub fn set_application(
    state: State<'_, AppState>,
    url: String,
    status: Option<String>,
    note: Option<String>,
) -> CmdResult<()> {
    let conn = state.db.lock_recover();
    db::set_application(&conn, &url, status.as_deref(), note.as_deref()).map_err(err)
}

/// Every saved role's application status and note.
#[tauri::command(async)]
pub fn get_applications(state: State<'_, AppState>) -> CmdResult<Vec<db::Application>> {
    let conn = state.db.lock_recover();
    db::load_applications(&conn).map_err(err)
}

/// Hides an item from the feed and the hub for good.
#[tauri::command(async)]
pub fn set_dismissed(state: State<'_, AppState>, url: String, dismissed: bool) -> CmdResult<()> {
    {
        let conn = state.db.lock_recover();
        db::set_dismissed(&conn, &url, dismissed).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

/// Records that the user opened an item, which sinks it in later rankings.
#[tauri::command(async)]
pub fn mark_seen(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    {
        let conn = state.db.lock_recover();
        db::mark_seen(&conn, &url).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

#[tauri::command(async)]
pub fn clear_dismissed(state: State<'_, AppState>) -> CmdResult<()> {
    {
        let conn = state.db.lock_recover();
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
        let conn = state.db.lock_recover();
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
        let conn = state.db.lock_recover();
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
        let conn = state.db.lock_recover();
        db::set_setting(&conn, KEY_SKILLS, &json).map_err(err)?;
    }
    invalidate(&state);
    Ok(())
}

#[tauri::command(async)]
pub fn get_setting(state: State<'_, AppState>, key: String) -> CmdResult<Option<String>> {
    let conn = state.db.lock_recover();
    db::get_setting(&conn, &key).map_err(err)
}

#[tauri::command(async)]
pub fn set_setting(state: State<'_, AppState>, key: String, value: String) -> CmdResult<()> {
    {
        let conn = state.db.lock_recover();
        db::set_setting(&conn, &key, &value).map_err(err)?;
    }
    // `major`, `weights`, `home_region` and the company tiers all arrive
    // through here, and every one of them is part of the ranking profile.
    // A `ui.` key is the interface remembering itself — the Roles filters, the
    // saved searches — and is written on every chip tap; re-ranking the whole
    // cache for it would cost a second per tap and change nothing.
    if !is_ui_setting(&key) {
        invalidate(&state);
    }
    Ok(())
}

/// Settings the ranking never reads. See `set_setting`.
fn is_ui_setting(key: &str) -> bool {
    key.starts_with("ui.")
}

/// Every distinct source currently represented in the local store, for the
/// source filter chips.
#[tauri::command(async)]
pub fn list_sources(state: State<'_, AppState>) -> CmdResult<Vec<String>> {
    let conn = state.db.lock_recover();
    db::distinct_sources(&conn).map_err(err)
}

// --- what to scrape ---------------------------------------------------------

fn current_prefs(state: &AppState) -> ScrapePrefs {
    json_setting(state, prefs::KEY)
}

/// The choices the setup form offers that Rust owns the spelling of. Seniority
/// bands and region tags are matched by equality against what the classifier
/// and the location tagger produce, so a copy typed into the UI would fail by
/// matching nothing; the interest list is served the same way by
/// `list_interests`.
#[derive(Serialize)]
pub struct SetupOptions {
    seniority: Vec<&'static str>,
    regions: Vec<&'static str>,
}

#[tauri::command]
pub fn list_setup_options() -> SetupOptions {
    SetupOptions {
        seniority: rank::seniority_levels(),
        regions: location::region_tags(),
    }
}

#[tauri::command(async)]
pub fn get_scrape_prefs(state: State<'_, AppState>) -> CmdResult<ScrapePrefs> {
    Ok(current_prefs(&state))
}

/// Stores new preferences and makes the cache and the schedule agree with them.
///
/// What is no longer wanted is deleted: the sweep never visits a source that is
/// not being scraped, so a category switched off would otherwise sit in the feed
/// forever, frozen at the moment it was disabled. What *newly* became wanted is
/// made due by forgetting its last run, so the next refresh fetches exactly
/// that and nothing else.
fn apply_prefs(state: &AppState, new: &ScrapePrefs) -> CmdResult<()> {
    let old = current_prefs(state);
    let sources = scrapers::sources();

    {
        let conn = state.db.lock_recover();
        db::set_setting(&conn, prefs::KEY, &serde_json::to_string(new).map_err(err)?)
            .map_err(err)?;
        db::purge_unwanted(&conn, new.wanted(), &new.disabled_sources).map_err(err)?;

        // A source with no ledger row has never run, and so is due. Off ones go
        // too, so that switching one back on starts from a clean slate.
        let mut forget: Vec<String> = sources
            .iter()
            .filter(|(category, name)| !new.source_enabled(*category, name))
            .map(|(_, name)| name.to_string())
            .collect();
        forget.extend(
            prefs::made_due(&old, new, &sources)
                .into_iter()
                .map(String::from),
        );
        db::forget_source_runs(&conn, &forget).map_err(err)?;

        db::prune_orphan_states(&conn).map_err(err)?;
        db::prune_orphan_details(&conn).map_err(err)?;
        db::incremental_vacuum(&conn).map_err(err)?;
    }
    invalidate(state);
    Ok(())
}

#[tauri::command(async)]
pub fn set_scrape_prefs(state: State<'_, AppState>, prefs: ScrapePrefs) -> CmdResult<()> {
    apply_prefs(&state, &prefs)
}

/// Where the reader is with the setup form: `"needed"` before anything has been
/// fetched (a first launch), `"offered"` for an existing install that has not
/// seen it yet — its feed is already there, so the form may be skipped — and
/// `"done"` after.
#[tauri::command(async)]
pub fn get_onboarding_state(state: State<'_, AppState>) -> CmdResult<&'static str> {
    onboarding_state(&state)
}

fn onboarding_state(state: &AppState) -> CmdResult<&'static str> {
    let conn = state.db.lock_recover();
    let has = |key: &str| {
        db::get_setting(&conn, key)
            .map(|v| v.is_some())
            .map_err(err)
    };
    Ok(match (has(db::ONBOARDED_KEY)?, has(KEY_ONBOARDING_SEEN)?) {
        (false, _) => "needed",
        (true, false) => "offered",
        (true, true) => "done",
    })
}

/// Saves the answers to the setup form and lets scraping begin.
///
/// Everything is written before `onboarded_at`, and `onboarded_at` is written
/// *now* rather than when the first scrape finishes: if the app is closed
/// halfway through, the next launch should carry on scraping what was asked for
/// instead of asking again. The run ledger is empty, so every wanted source is
/// due. The caller starts the scrape itself, so it can follow the progress
/// events from the first one.
///
/// Anything the app does not recognise is dropped rather than stored: an
/// interest, band or region it cannot match would be a preference that silently
/// does nothing.
#[tauri::command(async)]
pub fn complete_onboarding(
    state: State<'_, AppState>,
    prefs: ScrapePrefs,
    interests: Vec<String>,
    home_region: Option<String>,
    seniority: Vec<String>,
) -> CmdResult<()> {
    finish_onboarding(&state, &prefs, interests, home_region, seniority)
}

fn finish_onboarding(
    state: &AppState,
    prefs: &ScrapePrefs,
    interests: Vec<String>,
    home_region: Option<String>,
    seniority: Vec<String>,
) -> CmdResult<()> {
    let known = rank::list_interests();
    let interests: Vec<String> = interests
        .into_iter()
        .filter(|i| known.contains(i))
        .collect();
    let levels = rank::seniority_levels();
    let seniority: Vec<String> = seniority
        .into_iter()
        .filter(|l| levels.contains(&l.as_str()))
        .collect();
    let regions = location::region_tags();
    let home = home_region
        .filter(|r| regions.contains(&r.as_str()))
        .unwrap_or_default();

    apply_prefs(state, prefs)?;
    {
        let conn = state.db.lock_recover();
        let json = |v: &Vec<String>| serde_json::to_string(v).map_err(err);
        db::set_setting(&conn, KEY_INTERESTS, &json(&interests)?).map_err(err)?;
        db::set_setting(&conn, KEY_TARGET_SENIORITY, &json(&seniority)?).map_err(err)?;
        // Empty is how "no home region" is stored; `current_profile` reads it as none.
        db::set_setting(&conn, KEY_HOME_REGION, &home).map_err(err)?;

        let now = Utc::now().to_rfc3339();
        if db::get_setting(&conn, db::ONBOARDED_KEY)
            .map_err(err)?
            .is_none()
        {
            db::set_setting(&conn, db::ONBOARDED_KEY, &now).map_err(err)?;
        }
        db::set_setting(&conn, KEY_ONBOARDING_SEEN, &now).map_err(err)?;
    }
    invalidate(state);
    Ok(())
}

/// Dismisses the form for an existing install. Refused on a first launch,
/// where nothing has been asked and there is nothing to skip to.
#[tauri::command(async)]
pub fn skip_onboarding(state: State<'_, AppState>) -> CmdResult<()> {
    dismiss_onboarding(&state)
}

fn dismiss_onboarding(state: &AppState) -> CmdResult<()> {
    let conn = state.db.lock_recover();
    if db::get_setting(&conn, db::ONBOARDED_KEY)
        .map_err(err)?
        .is_none()
    {
        return Err("setup has not been completed yet".to_string());
    }
    db::set_setting(&conn, KEY_ONBOARDING_SEEN, &Utc::now().to_rfc3339()).map_err(err)
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
    /// Bullet id → the posting's own words it echoes. Weaker evidence than
    /// `why`, and kept apart from it so the UI cannot let the two read alike.
    echo: std::collections::HashMap<String, Vec<String>>,
    /// Bullets the page budget trimmed.
    dropped: Vec<resume::tailor::Dropped>,
    /// Posting skills the surviving bullets speak to, and everything it asked
    /// for. The pane renders these as "your résumé covers 4 of 9".
    covered: Vec<String>,
    required: Vec<String>,
    /// The subset of `required` the posting treats as non-negotiable.
    must: Vec<String>,
    /// What the posting keeps coming back to, in its own words.
    focus: Vec<String>,
    /// The theme actually used, once the variant's override is applied — the
    /// picker has to show what is on screen, not what is stored.
    theme: resume::Theme,
    /// What a save dialog should suggest.
    filename: String,
    /// Section id → its entries in the order they are drawn, whoever chose
    /// it — the reader's drag or the tailoring's own lead-with-the-relevant —
    /// so the outline lists them the way the page does.
    order: std::collections::BTreeMap<String, Vec<String>>,
    /// The skills block as `[label, skills]` lines, before the skills the
    /// reader left out — so the outline can offer them back.
    skills: Vec<(String, Vec<String>)>,
}

fn now_rfc3339() -> String {
    Utc::now().to_rfc3339()
}

/// Reads one stored résumé, or the default when no id is given.
fn stored_resume(state: &AppState, id: Option<&str>) -> CmdResult<Option<db::StoredResume>> {
    let conn = state.db.lock_recover();
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
    let conn = state.db.lock_recover();
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

    let conn = state.db.lock_recover();
    db::save_resume(&conn, &id, &name, &doc_json, &theme_json, &now_rfc3339()).map_err(err)?;
    Ok(id)
}

#[tauri::command(async)]
pub fn delete_resume(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let mut conn = state.db.lock_recover();
    db::delete_resume(&mut conn, &id).map_err(err)
}

#[tauri::command(async)]
pub fn set_default_resume(state: State<'_, AppState>, id: String) -> CmdResult<()> {
    let mut conn = state.db.lock_recover();
    db::set_default_resume(&mut conn, &id).map_err(err)
}

/// The overrides saved for one posting, if any.
#[tauri::command(async)]
pub fn get_resume_variant(
    state: State<'_, AppState>,
    url: String,
    resume_id: String,
) -> CmdResult<Option<resume::Variant>> {
    let conn = state.db.lock_recover();
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
    let conn = state.db.lock_recover();
    db::save_variant(&conn, &url, &resume_id, &json, &now_rfc3339()).map_err(err)
}

/// The URL scheme of a pasted job description. Tailoring keys everything on a
/// posting's URL, so a pasted one gets a URL of its own and every résumé
/// command works on it unchanged.
const PASTED_PREFIX: &str = "pasted:";

/// A pasted description, read the way a fetched one is: through
/// `sections::split`, so its Requirements / Responsibilities headings weigh
/// what they weigh on any other posting.
fn pasted_item(posting: &db::PastedPosting, url: &str) -> Item {
    let detail = details::detail_of(scrapers::sections::split(&posting.text), Vec::new());
    let mut item = Item {
        company: posting.company.clone(),
        ..Item::new(posting.title.clone(), "Pasted", ItemType::Job, url)
    }
    .with_content(posting.text.chars().take(280).collect::<String>());
    if let Ok(at) = DateTime::parse_from_rfc3339(&posting.created_at) {
        item = item.with_timestamp(at.with_timezone(&Utc));
    }
    let details = std::iter::once((url.to_string(), detail)).collect();
    db::attach_details(std::slice::from_mut(&mut item), &details);
    item
}

/// A pasted posting as the picker lists it.
#[derive(Serialize)]
pub struct PastedSummary {
    url: String,
    title: String,
    company: Option<String>,
    created_at: String,
    /// No job description at all: a version of the résumé chosen by hand.
    by_hand: bool,
}

/// Stores a job description pasted in from anywhere, and returns the URL to
/// tailor against.
///
/// The text may be empty. That is a version made without any job in mind —
/// a general one-pager, say — where the reader picks what goes on the page
/// and the page limit trims the rest. It is a posting with no text, which
/// tailors against nothing, so every résumé command already handles it.
#[tauri::command(async)]
pub fn save_pasted_posting(
    state: State<'_, AppState>,
    title: String,
    company: Option<String>,
    text: String,
) -> CmdResult<String> {
    let text = text.trim();
    let title = title.trim();
    let posting = db::PastedPosting {
        id: resume::model::new_id(),
        title: match (title.is_empty(), text.is_empty()) {
            (false, _) => title.into(),
            (true, true) => "General résumé".into(),
            (true, false) => "Pasted job".into(),
        },
        company: company
            .map(|c| c.trim().to_string())
            .filter(|c| !c.is_empty()),
        text: text.to_string(),
        created_at: now_rfc3339(),
    };
    let conn = state.db.lock_recover();
    db::save_pasted(&conn, &posting).map_err(err)?;
    Ok(format!("{PASTED_PREFIX}{}", posting.id))
}

/// Every pasted posting, newest first.
#[tauri::command(async)]
pub fn list_pasted_postings(state: State<'_, AppState>) -> CmdResult<Vec<PastedSummary>> {
    let conn = state.db.lock_recover();
    Ok(db::list_pasted(&conn)
        .map_err(err)?
        .into_iter()
        .map(|p| PastedSummary {
            url: format!("{PASTED_PREFIX}{}", p.id),
            title: p.title,
            company: p.company,
            created_at: p.created_at,
            by_hand: p.text.trim().is_empty(),
        })
        .collect())
}

/// Deletes a pasted posting and the tailoring made against it.
#[tauri::command(async)]
pub fn delete_pasted_posting(state: State<'_, AppState>, url: String) -> CmdResult<()> {
    let Some(id) = url.strip_prefix(PASTED_PREFIX) else {
        return Err("Not a pasted posting.".into());
    };
    let conn = state.db.lock_recover();
    db::delete_pasted(&conn, id, &url).map_err(err)
}

/// Throws away a posting's overrides, putting it back to whatever the auto pass
/// decides.
#[tauri::command(async)]
pub fn clear_resume_variant(
    state: State<'_, AppState>,
    url: String,
    resume_id: String,
) -> CmdResult<()> {
    let conn = state.db.lock_recover();
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
        let conn = state.db.lock_recover();
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
    let target = item
        .as_ref()
        .map(target_of)
        .unwrap_or_else(resume::Target::none);
    let have = current_skills(state);
    let (laid_out, selection) = resume::tailored(doc, &target, &have, &variant, &theme);
    let company = item.as_ref().and_then(|i| i.company.clone());
    Ok(view_of(laid_out, selection, theme, doc, company.as_deref()))
}

/// Everything the tailoring pass needs to know about the posting.
///
/// Two readings of the same page: what it asks for, and what it keeps coming
/// back to. A saved posting whose text the prune has since dropped still has
/// its `required_skills` snapshot, so it falls back to the skills alone rather
/// than to nothing.
fn target_of(item: &Item) -> resume::Target {
    if !skills::from_posting(item) {
        return resume::Target::for_skills(&item.required_skills);
    }
    let section = |text: &Option<String>| text.as_deref().unwrap_or("").to_string();
    let parts = [
        (section(&item.requirements), terms::REQUIREMENTS),
        (section(&item.responsibilities), terms::RESPONSIBILITIES),
        (section(&item.description), terms::DESCRIPTION),
    ];
    let parts: Vec<(&str, f32)> = parts
        .iter()
        .map(|(text, weight)| (text.as_str(), *weight))
        .collect();
    resume::Target::new(&skills::demand(item), terms::Focus::read(&parts))
}

fn view_of(
    laid_out: resume::layout::LaidOut,
    selection: resume::Selection,
    theme: resume::Theme,
    doc: &resume::Resume,
    company: Option<&str>,
) -> ResumeView {
    let order = doc
        .sections
        .iter()
        .map(|section| {
            let ids = selection
                .order_for(section)
                .iter()
                .map(|e| e.id.clone())
                .collect();
            (section.id.clone(), ids)
        })
        .collect();
    ResumeView {
        pages: laid_out.pages,
        fill: laid_out.fill,
        bullets: selection.bullets.iter().cloned().collect(),
        entries: selection.entries.iter().cloned().collect(),
        why: selection.why,
        echo: selection.echo,
        dropped: selection.dropped,
        covered: selection.covered,
        required: selection.required,
        must: selection.must,
        focus: selection.focus,
        theme,
        filename: resume::suggested_filename(&doc.contact.name, company),
        order,
        skills: selection.skill_lines,
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
    let conn = state.db.lock_recover();
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

#[cfg(test)]
mod tests {
    use super::*;
    use chrono::Duration;

    #[test]
    fn interface_settings_do_not_touch_the_ranking() {
        assert!(is_ui_setting("ui.roles_filters"));
        assert!(is_ui_setting("ui.saved_searches"));
        // Everything the ranking reads must still re-rank.
        for key in [
            "major",
            "weights",
            "home_region",
            "target_seniority",
            "company_tiers",
        ] {
            assert!(!is_ui_setting(key), "{key}");
        }
    }

    #[test]
    fn a_pasted_posting_is_tailored_against_like_a_fetched_one() {
        let t = test_state();
        let posting = db::PastedPosting {
            id: "p1".into(),
            title: "Data Engineer".into(),
            company: Some("Acme".into()),
            text: "We move money.\n\nRequirements:\n- 3+ years of Python\n- Strong SQL\n\nNice to have:\n- Kafka".into(),
            created_at: now_rfc3339(),
        };
        db::save_pasted(&t.state.db.lock_recover(), &posting).unwrap();
        let url = "pasted:p1";

        let item = item_detail(&t.state, url)
            .unwrap()
            .expect("the pasted posting resolves");
        assert_eq!(item.title, "Data Engineer");
        assert_eq!(item.company.as_deref(), Some("Acme"));
        assert!(
            skills::from_posting(&item),
            "its text counts as a posting's own"
        );
        let asks = skills::extract(&item);
        for skill in ["Python", "SQL", "Kafka"] {
            assert!(asks.iter().any(|s| s == skill), "{skill} in {asks:?}");
        }
        assert!(
            item.requirements.is_some(),
            "the Requirements heading is honoured"
        );

        // A version with no job text tailors against nothing: no skills, and
        // the reader's own choices plus the page limit decide the page.
        let blank = db::PastedPosting {
            id: "p2".into(),
            title: "General".into(),
            company: None,
            text: String::new(),
            created_at: now_rfc3339(),
        };
        db::save_pasted(&t.state.db.lock_recover(), &blank).unwrap();
        let general = item_detail(&t.state, "pasted:p2").unwrap().unwrap();
        assert!(!skills::from_posting(&general));
        assert!(skills::extract(&general).is_empty());

        // Deleting it takes its tailoring with it.
        let conn = t.state.db.lock_recover();
        db::save_variant(&conn, url, "r1", "{}", &now_rfc3339()).unwrap();
        db::delete_pasted(&conn, "p1", url).unwrap();
        assert!(db::load_pasted(&conn, "p1").unwrap().is_none());
        assert!(db::load_variant(&conn, url, "r1").unwrap().is_none());
    }

    #[test]
    fn the_ranked_cache_turns_over_on_the_hour() {
        let at = |h, m, sec| Utc.with_ymd_and_hms(2026, 11, 12, h, m, sec).unwrap();
        assert_eq!(rank_bucket(at(14, 0, 0)), rank_bucket(at(14, 59, 59)));
        assert_ne!(rank_bucket(at(14, 59, 59)), rank_bucket(at(15, 0, 0)));
        assert_eq!(bucket_start(rank_bucket(at(14, 37, 12))), at(14, 0, 0));
    }

    fn flagged(title: &str, in_days: i64, merit: f64) -> Item {
        let mut item = Item::new(title, "S", ItemType::Job, format!("https://x.test/{title}"));
        item.closes_at = Some(Utc::now() + Duration::days(in_days));
        item.merit = Some(merit);
        item.last_call = true;
        item
    }

    #[test]
    fn last_calls_are_soonest_first_then_best_and_skip_the_unflagged() {
        let mut unflagged = flagged("lc-unflagged", 1, 99.0);
        unflagged.last_call = false;
        let items = vec![
            flagged("lc-late", 6, 20.0),
            unflagged,
            flagged("lc-soon-weaker", 1, 14.0),
            flagged("lc-soon-better", 1, 18.0),
        ];
        // `1 day` is computed per call, so pin the two that must tie.
        let mut items = items;
        items[3].closes_at = items[2].closes_at;

        let titles: Vec<&str> = last_calls(&items)
            .iter()
            .map(|i| i.title.as_str())
            .collect();
        assert_eq!(titles, ["lc-soon-better", "lc-soon-weaker", "lc-late"]);
    }

    #[test]
    fn the_last_call_list_is_capped_in_rust() {
        let items: Vec<Item> = (0..LAST_CALL_LIMIT + 10)
            .map(|i| flagged(&format!("lc-{i}"), 2, 15.0))
            .collect();
        assert_eq!(last_calls(&items).len(), LAST_CALL_LIMIT);
    }

    // --- the persist and onboarding wiring, against a real store ---
    //
    // `persist` is where the sweep, the run ledger and the ceiling meet, and a
    // mistake there is silent: a sweep fed the wrong scopes deletes a board's
    // postings without an error. These run it over a real database file.

    use crate::scrapers::{Category, Health, SourceOutcome};
    use std::sync::atomic::{AtomicBool, AtomicU64};
    use std::sync::Mutex;

    struct TestState {
        state: AppState,
        path: std::path::PathBuf,
        /// Leave the file behind, for a test whose output is the database.
        keep: bool,
    }

    impl Drop for TestState {
        fn drop(&mut self) {
            if self.keep {
                return;
            }
            for suffix in ["", "-wal", "-shm"] {
                let _ = std::fs::remove_file(format!("{}{suffix}", self.path.display()));
            }
        }
    }

    fn test_state() -> TestState {
        let unique = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap()
            .as_nanos();
        test_state_at(
            std::env::temp_dir().join(format!("antifomo-cmd-{unique}.db")),
            false,
        )
    }

    fn test_state_at(path: std::path::PathBuf, keep: bool) -> TestState {
        let conn = db::open(&path).unwrap();
        TestState {
            state: AppState {
                db: Mutex::new(conn),
                refreshing: AtomicBool::new(false),
                generation: AtomicU64::new(0),
                ranked: Mutex::new(None),
                scan: ScanControl::default(),
            },
            path,
            keep,
        }
    }

    fn outcome(
        source: &'static str,
        category: Category,
        count: usize,
        health: Health,
    ) -> SourceOutcome {
        SourceOutcome {
            source,
            category,
            count,
            health,
        }
    }

    fn posting(title: &str, company: &str) -> Item {
        let mut item = Item::new(
            title,
            "Greenhouse",
            ItemType::Job,
            format!("https://x.test/{title}"),
        )
        .with_company(company);
        // Early-career, so the ingest filter has nothing to say about it: these
        // tests are about what happens to a row once it is stored.
        item.seniority = Some("Junior".into());
        item
    }

    fn titles(state: &AppState) -> Vec<String> {
        let conn = state.db.lock_recover();
        let mut all: Vec<String> = db::load_items(&conn)
            .unwrap()
            .into_iter()
            .map(|i| i.title)
            .collect();
        all.sort();
        all
    }

    #[test]
    fn a_scrape_records_what_became_of_every_source_it_ran() {
        let t = test_state();
        let outcomes = vec![
            outcome("Greenhouse", Category::Opportunities, 2, Health::Ok),
            outcome(
                "Pitt CSC Repo",
                Category::Opportunities,
                0,
                Health::Dormant("repository dormant".into()),
            ),
            outcome(
                "Lever",
                Category::Opportunities,
                0,
                Health::Failed("timed out".into()),
            ),
        ];
        persist(
            &t.state,
            vec![posting("a", "Acme"), posting("b", "Acme")],
            outcomes,
        )
        .unwrap();

        let conn = t.state.db.lock_recover();
        let runs: std::collections::HashMap<_, _> = db::load_source_runs(&conn)
            .unwrap()
            .into_iter()
            .map(|r| (r.source.clone(), r))
            .collect();
        assert!(runs["Greenhouse"].ok && runs["Greenhouse"].last_error.is_none());
        assert_eq!(runs["Greenhouse"].last_count, 2);
        // A dormant source worked: it is on the long schedule, with a note.
        assert!(runs["Pitt CSC Repo"].ok);
        assert_eq!(
            runs["Pitt CSC Repo"].last_error.as_deref(),
            Some("repository dormant")
        );
        // A failed one is not, and has never succeeded.
        assert!(!runs["Lever"].ok && runs["Lever"].last_ok_at.is_none());
    }

    #[test]
    fn every_source_failing_keeps_the_cache_and_still_records_why() {
        let t = test_state();
        persist(
            &t.state,
            vec![posting("keep", "Acme")],
            vec![outcome(
                "Greenhouse",
                Category::Opportunities,
                1,
                Health::Ok,
            )],
        )
        .unwrap();

        let result = persist(
            &t.state,
            Vec::new(),
            vec![outcome(
                "Greenhouse",
                Category::Opportunities,
                0,
                Health::Failed("offline".into()),
            )],
        );
        assert!(
            result.is_err(),
            "an all-failed run is the network, not an empty world"
        );
        assert_eq!(titles(&t.state), ["keep"]);

        let conn = t.state.db.lock_recover();
        let run = &db::load_source_runs(&conn).unwrap()[0];
        assert!(
            !run.ok,
            "the failure has to reach the ledger so it is retried soon"
        );
    }

    #[test]
    fn a_run_that_worked_but_returned_nothing_wanted_is_not_an_error() {
        let t = test_state();
        let result = persist(
            &t.state,
            Vec::new(),
            vec![outcome(
                "Greenhouse",
                Category::Opportunities,
                40,
                Health::Ok,
            )],
        );
        assert_eq!(result, Ok(0));
    }

    /// The whole point of scoping the sweep to employers: Acme's board failed,
    /// so nothing in this scrape speaks for Acme, and its postings must survive
    /// a scrape that Other's board answered.
    #[test]
    fn persist_charges_misses_only_to_employers_whose_board_answered() {
        let t = test_state();
        let board = [outcome(
            "Greenhouse",
            Category::Opportunities,
            3,
            Health::Ok,
        )];
        persist(
            &t.state,
            vec![
                posting("acme-a", "Acme"),
                posting("other-b", "Other"),
                posting("other-c", "Other"),
            ],
            board.to_vec(),
        )
        .unwrap();

        // Three days on, two misses already banked for everything.
        {
            let conn = t.state.db.lock_recover();
            conn.execute(
                "UPDATE items SET last_seen_at = ?1, missed = 2",
                rusqlite::params![(Utc::now() - chrono::Duration::days(3)).to_rfc3339()],
            )
            .unwrap();
        }

        // Acme's board failed this time; Other answered, without `other-c`.
        persist(&t.state, vec![posting("other-b", "Other")], board.to_vec()).unwrap();

        assert_eq!(
            titles(&t.state),
            ["acme-a", "other-b"],
            "Other's missing posting goes; Acme's silent board is not held against it"
        );
    }

    // --- a scrape that arrives in pieces ---

    fn listed(source: &'static str, title: &str, url: &str) -> Item {
        let mut item = Item::new(title, source, ItemType::Job, url).with_company("Acme");
        item.seniority = Some("Junior".into());
        item
    }

    fn age_everything(t: &TestState, days: i64, missed: i64) {
        let conn = t.state.db.lock_recover();
        conn.execute(
            "UPDATE items SET last_seen_at = ?1, missed = ?2",
            rusqlite::params![
                (Utc::now() - chrono::Duration::days(days)).to_rfc3339(),
                missed
            ],
        )
        .unwrap();
    }

    /// The case that rules out sweeping after each source: Simplify owns a URL
    /// the employer's board also lists. Simplify has dropped it and has two
    /// misses banked, so a sweep run when only Simplify has reported would
    /// delete it a moment before Greenhouse vouches for it.
    #[test]
    fn a_posting_two_sources_list_survives_whichever_finishes_first() {
        for greenhouse_first in [false, true] {
            let t = test_state();
            persist(
                &t.state,
                vec![listed("Simplify", "role", "https://acme.test/1")],
                vec![],
            )
            .unwrap();
            age_everything(&t, 3, 2);

            let run_start = Utc::now();
            // Simplify answered with a different posting; Greenhouse still lists
            // the old one.
            let simplify = vec![listed("Simplify", "other", "https://acme.test/2")];
            let board = vec![listed("Greenhouse", "role", "https://acme.test/1")];
            let mut alive = std::collections::HashSet::new();
            let batches = if greenhouse_first {
                [board, simplify]
            } else {
                [simplify, board]
            };
            for batch in batches {
                let saved = persist_batch(&t.state, batch, run_start).unwrap();
                alive.extend(saved.alive);
            }
            let _serial = scrapers::boards::test_lock();
            finalize(&t.state, run_start, &alive, true).unwrap();

            assert!(
                titles(&t.state).contains(&"role".to_string()),
                "greenhouse_first={greenhouse_first}: a posting a source still lists was swept"
            );
        }
    }

    #[test]
    fn a_stopped_run_charges_nobody_a_miss() {
        let t = test_state();
        persist(
            &t.state,
            vec![posting("a", "Acme"), posting("b", "Acme")],
            vec![outcome(
                "Greenhouse",
                Category::Opportunities,
                2,
                Health::Ok,
            )],
        )
        .unwrap();
        age_everything(&t, 3, 2);
        t.state
            .db
            .lock()
            .unwrap()
            .execute("DELETE FROM settings WHERE key = ?1", [KEY_LAST_REFRESH])
            .unwrap();

        // The stop landed after Acme's board answered without `b`. A complete
        // run would charge b its third miss; a stopped one carries no scopes.
        let run_start = Utc::now();
        persist_batch(&t.state, vec![posting("a", "Acme")], run_start).unwrap();
        let none = std::collections::HashSet::new();
        let _serial = scrapers::boards::test_lock();
        finalize(&t.state, run_start, &none, false).unwrap();

        assert_eq!(titles(&t.state), ["a", "b"]);
        let conn = t.state.db.lock_recover();
        assert!(
            db::get_setting(&conn, KEY_LAST_REFRESH).unwrap().is_none(),
            "a stopped run is not a completed refresh"
        );
    }

    #[test]
    fn a_source_switched_off_mid_run_lands_no_more_rows() {
        let t = test_state();
        let run_start = Utc::now();
        persist_batch(
            &t.state,
            vec![listed("Greenhouse", "before", "https://acme.test/before")],
            run_start,
        )
        .unwrap();
        {
            let conn = t.state.db.lock_recover();
            let off = ScrapePrefs {
                disabled_sources: vec!["Greenhouse".into()],
                ..ScrapePrefs::default()
            };
            db::set_setting(&conn, prefs::KEY, &serde_json::to_string(&off).unwrap()).unwrap();
        }
        persist_batch(
            &t.state,
            vec![listed("Greenhouse", "after", "https://acme.test/after")],
            run_start,
        )
        .unwrap();
        assert_eq!(titles(&t.state), ["before"]);
    }

    #[test]
    fn a_stop_request_aborts_the_phase_in_flight_and_survives_the_gap_between_phases() {
        use futures::future::{AbortHandle, Abortable};
        let control = ScanControl::default();
        control.begin();

        // A stop that arrives first is not lost: the next phase starts aborted.
        control.request_stop();
        let (handle, registration) = AbortHandle::new_pair();
        control.arm(handle);
        let runtime = tokio::runtime::Runtime::new().unwrap();
        let ended = runtime.block_on(Abortable::new(
            futures::future::pending::<()>(),
            registration,
        ));
        assert!(ended.is_err());
        assert!(control.stop_requested());

        // And a new run forgets it.
        control.begin();
        assert!(!control.stop_requested());
    }

    // --- onboarding ---

    #[test]
    fn a_first_launch_is_asked_before_anything_is_fetched() {
        let t = test_state();
        assert_eq!(onboarding_state(&t.state).unwrap(), "needed");
        assert!(!onboarded(&t.state), "refresh() must refuse to run");
        assert!(
            dismiss_onboarding(&t.state).is_err(),
            "there is nothing to skip to yet"
        );
    }

    #[test]
    fn answering_the_form_saves_the_answers_and_lets_scraping_begin() {
        let t = test_state();
        let prefs = ScrapePrefs {
            events: false,
            ..ScrapePrefs::default()
        };
        finish_onboarding(
            &t.state,
            &prefs,
            vec!["AI/ML".into(), "Not An Interest".into()],
            Some("Toronto".into()),
            vec!["Intern".into(), "Wizard".into()],
        )
        .unwrap();

        assert_eq!(onboarding_state(&t.state).unwrap(), "done");
        assert!(onboarded(&t.state));
        assert_eq!(current_prefs(&t.state), prefs);
        assert_eq!(
            current_interests(&t.state),
            ["AI/ML"],
            "unknown interests are dropped"
        );
        let profile = current_profile(&t.state, None);
        assert_eq!(profile.target_seniority, ["Intern"]);
        assert_eq!(profile.home_region.as_deref(), Some("Toronto"));
    }

    #[test]
    fn a_region_the_tagger_never_emits_is_not_stored() {
        let t = test_state();
        finish_onboarding(
            &t.state,
            &ScrapePrefs::default(),
            vec![],
            Some("Atlantis".into()),
            vec![],
        )
        .unwrap();
        assert_eq!(current_profile(&t.state, None).home_region, None);
    }

    /// An existing install was scraped before there was a form: it keeps its
    /// feed, is offered the form, and may dismiss it.
    #[test]
    fn an_upgrade_is_offered_the_form_and_may_skip_it() {
        let t = test_state();
        {
            let conn = t.state.db.lock_recover();
            db::set_setting(&conn, db::ONBOARDED_KEY, "2026-01-01T00:00:00+00:00").unwrap();
        }
        assert_eq!(onboarding_state(&t.state).unwrap(), "offered");
        assert!(onboarded(&t.state), "its launch refresh keeps running");
        dismiss_onboarding(&t.state).unwrap();
        assert_eq!(onboarding_state(&t.state).unwrap(), "done");
    }

    // --- changing your mind ---

    #[test]
    fn switching_a_kind_off_removes_its_rows_and_switching_it_on_makes_it_due() {
        let t = test_state();
        finish_onboarding(&t.state, &ScrapePrefs::default(), vec![], None, vec![]).unwrap();

        let mut intern = posting("intern", "Acme");
        intern.seniority = Some("Intern".into());
        let rows = vec![intern, posting("role", "Acme")];
        let ran = |source: &'static str| outcome(source, Category::Opportunities, 2, Health::Ok);
        persist(&t.state, rows, vec![ran("Greenhouse"), ran("Simplify")]).unwrap();
        assert_eq!(titles(&t.state), ["intern", "role"]);

        // Off: the job goes, the internship stays, and nothing is made due.
        let internships_only = ScrapePrefs {
            jobs: false,
            ..ScrapePrefs::default()
        };
        apply_prefs(&t.state, &internships_only).unwrap();
        assert_eq!(titles(&t.state), ["intern"]);
        let runs = |state: &AppState| {
            db::load_source_runs(&state.db.lock_recover())
                .unwrap()
                .len()
        };
        assert_eq!(
            runs(&t.state),
            2,
            "narrowing what is wanted must not trigger a refetch"
        );

        // On again: the opportunity sources were running all along, but the
        // jobs were dropped at ingest, so they have to be fetched again.
        apply_prefs(&t.state, &ScrapePrefs::default()).unwrap();
        assert_eq!(runs(&t.state), 0, "Greenhouse and Simplify are due again");
    }

    #[test]
    fn switching_a_source_off_removes_its_rows_and_its_ledger_entry() {
        let t = test_state();
        finish_onboarding(&t.state, &ScrapePrefs::default(), vec![], None, vec![]).unwrap();
        persist(
            &t.state,
            vec![posting("gh", "Acme")],
            vec![outcome(
                "Greenhouse",
                Category::Opportunities,
                1,
                Health::Ok,
            )],
        )
        .unwrap();

        let without = ScrapePrefs {
            disabled_sources: vec!["Greenhouse".into()],
            ..ScrapePrefs::default()
        };
        apply_prefs(&t.state, &without).unwrap();

        assert!(titles(&t.state).is_empty());
        assert!(db::load_source_runs(&t.state.db.lock_recover())
            .unwrap()
            .is_empty());
    }

    #[test]
    fn the_setup_options_are_the_strings_rust_matches_on() {
        let options = list_setup_options();
        assert_eq!(
            options.seniority,
            ["Intern", "New grad", "Junior", "Senior", "Lead"]
        );
        assert!(options.regions.contains(&"Toronto") && options.regions.contains(&"Canada"));
        assert!(!options.regions.contains(&"Global / Multi-region"));
    }

    /// The whole pipeline over the real sources, into a real store. Not run by
    /// default because it touches the network:
    ///
    /// ```text
    /// cargo test live_pipeline -- --ignored --nocapture
    /// ```
    ///
    /// `LIVE_SOURCES=all` runs every source rather than five, and `LIVE_DB=path`
    /// writes the store there and keeps it — which is how a database the size of
    /// a real cache is made for `perf_check`.
    ///
    /// It asserts what this change is *for* rather than counts, which drift with
    /// the live data: a hackathon that opened long ago is still there, an
    /// abandoned repository is reported rather than ingested, running the same
    /// scrape twice loses nothing, and what the reader switched off never
    /// arrives.
    #[test]
    #[ignore = "hits the network"]
    fn live_pipeline_keeps_what_is_open_and_reports_what_is_dormant() {
        let t = match std::env::var_os("LIVE_DB") {
            Some(path) => test_state_at(path.into(), true),
            None => test_state(),
        };
        let everything = std::env::var("LIVE_SOURCES").as_deref() == Ok("all");
        let wanted = ScrapePrefs {
            news: everything,
            ..ScrapePrefs::default()
        };
        finish_onboarding(
            &t.state,
            &wanted,
            vec!["AI/ML".into(), "Backend".into()],
            Some("Toronto".into()),
            vec!["Intern".into(), "New grad".into()],
        )
        .unwrap();

        let ran = ["Simplify", "Pitt CSC Repo", "Devpost", "Luma", "Greenhouse"];
        let plan: Vec<&'static str> =
            prefs::due(&scrapers::sources(), &[], &wanted, Utc::now(), false)
                .into_iter()
                .filter(|name| everything || ran.contains(name))
                .collect();
        assert!(
            everything || !plan.contains(&"Hacker News"),
            "news was switched off"
        );

        let runtime = tokio::runtime::Runtime::new().unwrap();
        let (items, outcomes) =
            runtime.block_on(scrapers::fetch_selected(&plan, |o, done, total| {
                println!(
                    "  {done}/{total} {:<20} {:>5} rows  {:?}",
                    o.source, o.count, o.health
                );
            }));
        let items: Vec<Item> = items.into_iter().filter(|i| wanted.admits(i)).collect();
        assert!(everything || items.iter().all(|i| i.item_type != ItemType::Article));

        persist(&t.state, items.clone(), outcomes).unwrap();
        let first = titles(&t.state).len();
        // The same scrape again: nothing was removed from the source, so
        // nothing may be removed from the store.
        persist(&t.state, items, Vec::new()).unwrap();
        assert_eq!(titles(&t.state).len(), first, "a repeat scrape lost rows");

        let runs = db::load_source_runs(&t.state.db.lock_recover()).unwrap();
        let pitt = runs
            .iter()
            .find(|r| r.source == "Pitt CSC Repo")
            .expect("ran");
        assert!(pitt.ok, "a dormant repository is not a failure");
        assert!(pitt
            .last_error
            .as_deref()
            .is_some_and(|n| n.contains("dormant")));

        let view = ranked_view(&t.state, None, Freshness::Exact).unwrap();
        let kept = |kind: ItemType| view.items.iter().filter(|i| i.item_type == kind).count();
        let devpost: Vec<&Item> = view
            .items
            .iter()
            .filter(|i| i.source_platform == "Devpost")
            .collect();
        println!(
            "  kept {first} rows: {} internships, {} jobs, {} events ({} Devpost)",
            kept(ItemType::Internship),
            kept(ItemType::Job),
            kept(ItemType::Event),
            devpost.len()
        );
        assert!(!devpost.is_empty(), "Devpost vanished");
        // The bug this replaces: Devpost stamps the day entries *opened*, which
        // the old date prune read as "ninety days old, delete".
        assert!(
            devpost
                .iter()
                .any(|d| d.timestamp < Utc::now() - chrono::Duration::days(30)),
            "expected an open hackathon whose entry period began long ago"
        );
        assert!(
            view.items
                .iter()
                .all(|i| i.source_platform != "Pitt CSC Repo"),
            "a dormant repository was ingested"
        );
        println!(
            "  bars: roles {:.1}..{:.1}  events {:.1}..{:.1}",
            view.bars.opportunity.lo,
            view.bars.opportunity.hi,
            view.bars.event.lo,
            view.bars.event.hi
        );
        let flagged: Vec<&Item> = view.items.iter().filter(|i| i.last_call).collect();
        let roles: Vec<&Item> = flagged
            .iter()
            .copied()
            .filter(|i| i.item_type.is_opportunity())
            .collect();
        println!(
            "  last calls flagged: {} ({} roles, {} events)",
            flagged.len(),
            roles.len(),
            flagged.len() - roles.len()
        );
        for item in last_calls(&view.items).into_iter().take(10) {
            println!(
                "  band: {:<9} {:<46} {:<16} merit {:>4.1} closes {}",
                format!("{:?}", item.item_type),
                item.title.chars().take(46).collect::<String>(),
                item.company.as_deref().unwrap_or("-"),
                item.merit.unwrap_or(0.0),
                item.closes_at
                    .map(|d| d.format("%b %d").to_string())
                    .unwrap_or_default()
            );
        }
    }
}
