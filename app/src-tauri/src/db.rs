//! Local SQLite store. Unlike the old backend — which held scraped items in a
//! module-global dict that died with the process — items are persisted here, so
//! the app paints instantly on launch and keeps working offline.

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};
use std::collections::{HashMap, HashSet};

use crate::models::{DetailStatus, Item, ItemType, JobDetail};

const SCHEMA_VERSION: i32 = 7;

pub fn open(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    // WAL keeps a background refresh from blocking reads by the UI.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    // NORMAL is the documented companion to WAL: durability is still crash-safe,
    // only a power cut can lose the last commit, and this is a rebuildable
    // cache of scraped listings. FULL costs an fsync per statement, which a
    // refresh writing 18,000 upserts pays 18,000 times.
    conn.pragma_update(None, "synchronous", "NORMAL")?;
    // ~32 MB of page cache and no temp files on disk. The whole database is
    // 43 MB, so this is the difference between a read walking the file and a
    // read walking memory. `mmap_size` is advisory — SQLite silently ignores
    // it where mmap is unavailable, which is why it is not an error here.
    conn.pragma_update(None, "cache_size", -32_000)?;
    conn.pragma_update(None, "temp_store", "MEMORY")?;
    let _ = conn.pragma_update(None, "mmap_size", 268_435_456i64);
    migrate(&conn)?;
    Ok(conn)
}

/// Rows in the item cache, without materialising them.
///
/// `feed_status` used to answer this by loading all 18,240 items and calling
/// `.len()`, which is 38 ms and several megabytes of allocation to produce one
/// integer — on a command the UI calls after every save and every dismissal.
pub fn count_items(conn: &Connection) -> Result<usize> {
    Ok(conn.query_row("SELECT COUNT(*) FROM items", [], |r| r.get::<_, i64>(0))? as usize)
}

/// How many cached items each source is currently contributing.
pub fn source_counts(conn: &Connection) -> Result<HashMap<String, usize>> {
    let mut stmt =
        conn.prepare("SELECT source_platform, COUNT(*) FROM items GROUP BY source_platform")?;
    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, i64>(1)? as usize))
    })?;
    Ok(rows.flatten().collect())
}

/// Every distinct source in the cache, alphabetically.
pub fn distinct_sources(conn: &Connection) -> Result<Vec<String>> {
    let mut stmt =
        conn.prepare("SELECT DISTINCT source_platform FROM items ORDER BY source_platform")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.flatten().collect())
}

/// How many items the user has starred and how many they have dismissed.
pub fn state_counts(conn: &Connection) -> Result<(usize, usize)> {
    Ok(conn.query_row(
        "SELECT COUNT(saved_at), COUNT(dismissed_at) FROM item_state",
        [],
        |r| Ok((r.get::<_, i64>(0)? as usize, r.get::<_, i64>(1)? as usize)),
    )?)
}

/// One posting's fetched description, for the detail pane.
///
/// The list commands deliberately do not carry this: `load_details` is 28 MB
/// of text across the cache, and a list renders none of it. The pane asks for
/// the one posting it is showing instead.
pub fn load_detail(conn: &Connection, url: &str) -> Result<Option<JobDetail>> {
    let mut stmt = conn.prepare(
        "SELECT description, requirements, responsibilities, perks,
                tagged_skills, status, closes_at, salary_min, salary_max,
                salary_currency, salary_period
         FROM job_details WHERE url = ?1",
    )?;
    Ok(stmt
        .query_row(params![url], |row| {
            let tagged: String = row.get(4)?;
            let status: String = row.get(5)?;
            Ok(JobDetail {
                description: row.get(0)?,
                requirements: row.get(1)?,
                responsibilities: row.get(2)?,
                perks: row.get(3)?,
                tagged_skills: serde_json::from_str(&tagged).unwrap_or_default(),
                closes_at: row
                    .get::<_, Option<String>>(6)?
                    .and_then(|s| parse_rfc3339(&s)),
                salary_min: row.get(7)?,
                salary_max: row.get(8)?,
                salary_currency: row.get(9)?,
                salary_period: row.get(10)?,
                status: DetailStatus::from_label(&status),
            })
        })
        .optional()?)
}

/// One cached item by URL, for the detail pane. `None` once the cache has
/// pruned it — the saved list keeps its own snapshot for exactly that case.
pub fn load_item(conn: &Connection, url: &str) -> Result<Option<Item>> {
    Ok(load_items_where(conn, "WHERE url = ?1", params![url])?
        .into_iter()
        .next())
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    // `items` is a rebuildable cache, so schema changes just drop and refetch
    // rather than carrying migration logic. `settings`, `item_state` and
    // `job_details` hold the only data worth keeping and are never dropped —
    // `job_details` in particular represents real network work that would be
    // expensive to redo, though a bump does clear the attempts that came back
    // with nothing. See the end of the batch.
    conn.execute_batch(
        r#"
        DROP TABLE IF EXISTS items;

        CREATE TABLE items (
            id              INTEGER PRIMARY KEY AUTOINCREMENT,
            title           TEXT NOT NULL,
            source_platform TEXT NOT NULL,
            item_type       TEXT NOT NULL,
            url             TEXT NOT NULL,
            content_text    TEXT NOT NULL DEFAULT '',
            timestamp       TEXT NOT NULL,
            discipline      TEXT,
            relevance_score REAL,
            location        TEXT,
            location_tags   TEXT NOT NULL DEFAULT '[]',
            -- The simplify.jobs posting id, when the source knows one. Scraped
            -- source data rather than something derived on read, and stored
            -- because the enrichment pass runs off the database, not off the
            -- Vec the scrape happened to return.
            simplify_id     TEXT,
            -- The employer, kept out of `title` so it can be filtered,
            -- grouped and sorted on. Nullable: news and events have none.
            company         TEXT,
            -- Scrape-time facts. The enrichment pass discovers these too, and
            -- stores its answers in `job_details`; `attach_details` prefers
            -- the fetched value and falls back to whichever of these the list
            -- endpoint happened to carry.
            closes_at       TEXT,
            salary_min      REAL,
            salary_max      REAL,
            salary_currency TEXT,
            salary_period   TEXT,
            seniority       TEXT,
            -- URL is the listing's real identity. The old backend declared
            -- uniqueness on (title, source_platform), but it never actually
            -- wrote to the database, so the flaw never surfaced: Pitt CSC
            -- titles every row "Internship at {company}", which would collapse
            -- every distinct role at one company into a single item.
            UNIQUE (url)
        );

        CREATE INDEX IF NOT EXISTS idx_items_type ON items (item_type);
        CREATE INDEX IF NOT EXISTS idx_items_timestamp ON items (timestamp DESC);
        CREATE INDEX IF NOT EXISTS idx_items_closes ON items (closes_at);
        CREATE INDEX IF NOT EXISTS idx_items_company ON items (company);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );

        -- What the user did with an item, keyed by the same URL identity as
        -- `items`. Deliberately *not* a foreign key: the `items` cache is
        -- pruned at 60 days and dropped wholesale on a schema bump, and a
        -- starred listing has to outlive both. `snapshot` carries a serialized
        -- Item so the saved list can still render one the cache no longer has.
        CREATE TABLE IF NOT EXISTS item_state (
            url          TEXT PRIMARY KEY,
            saved_at     TEXT,
            dismissed_at TEXT,
            seen_at      TEXT,
            snapshot     TEXT
        );

        CREATE INDEX IF NOT EXISTS idx_state_saved ON item_state (saved_at);

        -- One row per posting whose description we have tried to fetch. Kept
        -- out of `items` on purpose: `save_items` overwrites `content_text` on
        -- every refresh, so a description stored there would be destroyed by
        -- the next scrape whose enrichment happened to fail.
        --
        -- This is also the ledger that makes enrichment incremental. A row
        -- here means "already attempted" — including a failure, so a dead link
        -- costs one request rather than one per refresh forever.
        CREATE TABLE IF NOT EXISTS job_details (
            url              TEXT PRIMARY KEY,
            description      TEXT,
            requirements     TEXT,
            responsibilities TEXT,
            perks            TEXT,
            tagged_skills    TEXT NOT NULL DEFAULT '[]',
            status           TEXT NOT NULL,
            fetched_at       TEXT NOT NULL
        );

        -- A schema bump is also when the handler chain in `scrapers::details`
        -- has changed, and "already attempted" is only a useful answer while
        -- the thing that attempted it is the same. Rows that came back with
        -- text are real work and stay; the ones that did not are exactly the
        -- postings a new handler exists to serve, and keeping them would lock
        -- the improvement out of every cache that already exists.
        DELETE FROM job_details WHERE status <> 'ok';

        -- And the ok rows that a fixed parser would now read differently. A
        -- description stored as one line of more than 400 characters was never
        -- split: simplify.jobs mirrors the employer's HTML and this used to
        -- flatten the whole posting into a single 5 KB run of text, which the
        -- UI could only render as a wall and `scrapers::sections` never got to
        -- see the headings of. There is no fixing those in place — the markup
        -- is gone — so they go back on the queue.
        DELETE FROM job_details
         WHERE status = 'ok'
           AND description IS NOT NULL
           AND instr(description, char(10)) = 0
           AND length(description) > 400;
        "#,
    )?;

    // `job_details` predates these columns and must survive the bump with its
    // rows intact — it represents real network work. So they are added in
    // place rather than by rebuilding the table. An existing `ok` row simply
    // reads back with no deadline, which is the honest answer: the fetch that
    // produced it was not looking for one.
    for (column, decl) in [
        ("closes_at", "TEXT"),
        ("salary_min", "REAL"),
        ("salary_max", "REAL"),
        ("salary_currency", "TEXT"),
        ("salary_period", "TEXT"),
    ] {
        add_column_if_missing(conn, "job_details", column, decl)?;
    }

    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
    Ok(())
}

/// Adds a column to a table that must not be dropped, skipping it when the
/// column is already there. SQLite has no `ADD COLUMN IF NOT EXISTS`, and
/// `PRAGMA table_info` is the documented way to ask.
fn add_column_if_missing(conn: &Connection, table: &str, column: &str, decl: &str) -> Result<()> {
    let mut stmt = conn.prepare(&format!("PRAGMA table_info({table})"))?;
    let existing: HashSet<String> = stmt
        .query_map([], |row| row.get::<_, String>(1))?
        .flatten()
        .collect();
    if existing.contains(column) {
        return Ok(());
    }
    // Table and column names here are literals from the caller above, never
    // user input.
    conn.execute_batch(&format!("ALTER TABLE {table} ADD COLUMN {column} {decl}"))?;
    Ok(())
}

/// Upserts on URL, so re-running a scrape refreshes existing listings in place
/// instead of duplicating them.
pub fn save_items(conn: &mut Connection, items: &[Item]) -> Result<usize> {
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            r#"
            INSERT INTO items
                (title, source_platform, item_type, url, content_text,
                 timestamp, discipline, relevance_score, location, location_tags,
                 simplify_id, company, closes_at, salary_min, salary_max,
                 salary_currency, salary_period, seniority)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13, ?14,
                    ?15, ?16, ?17, ?18)
            ON CONFLICT (url) DO UPDATE SET
                title           = excluded.title,
                source_platform = excluded.source_platform,
                item_type       = excluded.item_type,
                content_text    = excluded.content_text,
                timestamp       = excluded.timestamp,
                discipline      = excluded.discipline,
                relevance_score = excluded.relevance_score,
                location        = excluded.location,
                location_tags   = excluded.location_tags,
                -- Only ever gained, never cleared: one source knowing the id
                -- and another not should not lose it on the second write.
                simplify_id     = COALESCE(excluded.simplify_id, items.simplify_id),
                -- Same rule, same reason: two sources can carry the same
                -- posting and only one of them publish a deadline or a pay
                -- range. Whichever refresh writes second must not erase what
                -- the first one learned.
                company         = COALESCE(excluded.company, items.company),
                closes_at       = COALESCE(excluded.closes_at, items.closes_at),
                salary_min      = COALESCE(excluded.salary_min, items.salary_min),
                salary_max      = COALESCE(excluded.salary_max, items.salary_max),
                salary_currency = COALESCE(excluded.salary_currency, items.salary_currency),
                salary_period   = COALESCE(excluded.salary_period, items.salary_period),
                seniority       = COALESCE(excluded.seniority, items.seniority)
            "#,
        )?;

        for item in items {
            stmt.execute(params![
                item.title,
                item.source_platform,
                item.item_type.as_str(),
                item.url,
                item.content_text,
                item.timestamp.to_rfc3339(),
                item.discipline,
                item.relevance_score,
                item.location,
                serde_json::to_string(&item.location_tags)?,
                item.simplify_id,
                item.company,
                item.closes_at.map(|d| d.to_rfc3339()),
                item.salary_min,
                item.salary_max,
                item.salary_currency,
                item.salary_period,
                item.seniority,
            ])?;
        }
    }
    tx.commit()?;
    Ok(items.len())
}

pub fn load_items(conn: &Connection) -> Result<Vec<Item>> {
    load_items_where(conn, "ORDER BY timestamp DESC", [])
}

/// The shared row mapping, so a single-item read and a whole-cache read cannot
/// disagree about what an `items` row means.
fn load_items_where<P: rusqlite::Params>(
    conn: &Connection,
    tail: &str,
    params: P,
) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(&format!(
        "SELECT title, source_platform, item_type, url, content_text,
                timestamp, discipline, relevance_score, location, location_tags,
                simplify_id, company, closes_at, salary_min, salary_max,
                salary_currency, salary_period, seniority
         FROM items {tail}"
    ))?;

    let rows = stmt.query_map(params, |row| {
        let ts: String = row.get(5)?;
        let tags: String = row.get(9)?;
        Ok(Item {
            title: row.get(0)?,
            source_platform: row.get(1)?,
            item_type: ItemType::from_label(&row.get::<_, String>(2)?),
            url: row.get(3)?,
            content_text: row.get(4)?,
            timestamp: DateTime::parse_from_rfc3339(&ts)
                .map(|d| d.with_timezone(&Utc))
                .unwrap_or_else(|_| Utc.timestamp_opt(0, 0).unwrap()),
            discipline: row.get(6)?,
            relevance_score: row.get(7)?,
            location: row.get(8)?,
            location_tags: serde_json::from_str(&tags).unwrap_or_default(),
            simplify_id: row.get(10)?,
            company: row.get(11)?,
            closes_at: row
                .get::<_, Option<String>>(12)?
                .and_then(|s| parse_rfc3339(&s)),
            salary_min: row.get(13)?,
            salary_max: row.get(14)?,
            salary_currency: row.get(15)?,
            salary_period: row.get(16)?,
            seniority: row.get(17)?,
            // Derived on read: `annotate` fills the saved/seen flags from
            // `item_state`, `attach_details` fills the fetched posting from
            // `job_details`, and `rank::personalize` fills the interest and
            // skill lists.
            matched_interests: Vec::new(),
            required_skills: Vec::new(),
            matched_skills: Vec::new(),
            company_tier: None,
            score_breakdown: Vec::new(),
            description: None,
            requirements: None,
            responsibilities: None,
            perks: None,
            tagged_skills: Vec::new(),
            saved: false,
            seen: false,
        })
    })?;

    Ok(rows.filter_map(Result::ok).collect())
}

/// Reads a stored RFC 3339 timestamp, discarding one we cannot parse rather
/// than dropping the whole row over it.
fn parse_rfc3339(s: &str) -> Option<DateTime<Utc>> {
    DateTime::parse_from_rfc3339(s)
        .ok()
        .map(|d| d.with_timezone(&Utc))
}

/// Drops entries older than `days` so the local store doesn't grow forever.
/// Job postings and news both go stale fast; nothing here is worth archiving.
pub fn prune_older_than(conn: &Connection, days: i64) -> Result<usize> {
    let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    Ok(conn.execute("DELETE FROM items WHERE timestamp < ?1", params![cutoff])?)
}

// --- item state: saved / dismissed / seen ---

/// The three durable per-item flags, as URL sets. Loaded once per read so
/// annotating a 400-item feed stays a hash lookup rather than 400 queries.
#[derive(Default)]
pub struct ItemStates {
    pub saved: HashSet<String>,
    pub dismissed: HashSet<String>,
    pub seen: HashSet<String>,
}

pub fn load_states(conn: &Connection) -> Result<ItemStates> {
    let mut stmt = conn.prepare("SELECT url, saved_at, dismissed_at, seen_at FROM item_state")?;
    let mut states = ItemStates::default();

    let rows = stmt.query_map([], |row| {
        Ok((
            row.get::<_, String>(0)?,
            row.get::<_, Option<String>>(1)?,
            row.get::<_, Option<String>>(2)?,
            row.get::<_, Option<String>>(3)?,
        ))
    })?;

    for (url, saved, dismissed, seen) in rows.flatten() {
        if saved.is_some() {
            states.saved.insert(url.clone());
        }
        if dismissed.is_some() {
            states.dismissed.insert(url.clone());
        }
        if seen.is_some() {
            states.seen.insert(url);
        }
    }
    Ok(states)
}

/// Stamps the saved/seen flags onto items that carry them. Dismissal is left to
/// the caller: the feed hides those rows, but the saved list must not.
pub fn annotate(items: &mut [Item], states: &ItemStates) {
    for item in items {
        item.saved = states.saved.contains(&item.url);
        item.seen = states.seen.contains(&item.url);
    }
}

// --- fetched job descriptions ---

/// Reads every stored description, keyed by URL, for [`attach_details`].
pub fn load_details(conn: &Connection) -> Result<HashMap<String, JobDetail>> {
    let mut stmt = conn.prepare(
        "SELECT url, description, requirements, responsibilities, perks,
                tagged_skills, status, closes_at, salary_min, salary_max,
                salary_currency, salary_period
         FROM job_details",
    )?;

    let rows = stmt.query_map([], |row| {
        let tagged: String = row.get(5)?;
        let status: String = row.get(6)?;
        Ok((
            row.get::<_, String>(0)?,
            JobDetail {
                description: row.get(1)?,
                requirements: row.get(2)?,
                responsibilities: row.get(3)?,
                perks: row.get(4)?,
                tagged_skills: serde_json::from_str(&tagged).unwrap_or_default(),
                closes_at: row
                    .get::<_, Option<String>>(7)?
                    .and_then(|s| parse_rfc3339(&s)),
                salary_min: row.get(8)?,
                salary_max: row.get(9)?,
                salary_currency: row.get(10)?,
                salary_period: row.get(11)?,
                status: DetailStatus::from_label(&status),
            },
        ))
    })?;

    Ok(rows.flatten().collect())
}

/// Stamps fetched descriptions onto the items that have one. The counterpart
/// to [`annotate`], and like it a no-op for items we know nothing about.
pub fn attach_details(items: &mut [Item], details: &HashMap<String, JobDetail>) {
    for item in items {
        let Some(detail) = details.get(&item.url) else {
            continue;
        };
        item.description = detail.description.clone();
        item.requirements = detail.requirements.clone();
        item.responsibilities = detail.responsibilities.clone();
        item.perks = detail.perks.clone();
        item.tagged_skills = detail.tagged_skills.clone();
        // The posting's own page outranks whatever the list endpoint said —
        // but only where it actually found something. A fetch that failed
        // must not blank a deadline the scrape already knew.
        if detail.closes_at.is_some() {
            item.closes_at = detail.closes_at;
        }
        if detail.salary_min.is_some() || detail.salary_max.is_some() {
            item.salary_min = detail.salary_min;
            item.salary_max = detail.salary_max;
            item.salary_currency = detail.salary_currency.clone();
            item.salary_period = detail.salary_period.clone();
        }
    }
}

/// Records what one fetch found — including that it found nothing, which is
/// what stops a dead link being retried on every refresh.
pub fn save_details(conn: &mut Connection, rows: &[(String, JobDetail)]) -> Result<usize> {
    let now = Utc::now().to_rfc3339();
    let tx = conn.transaction()?;
    {
        let mut stmt = tx.prepare(
            r#"
            INSERT INTO job_details
                (url, description, requirements, responsibilities, perks,
                 tagged_skills, status, fetched_at, closes_at, salary_min,
                 salary_max, salary_currency, salary_period)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)
            ON CONFLICT (url) DO UPDATE SET
                description      = excluded.description,
                requirements     = excluded.requirements,
                responsibilities = excluded.responsibilities,
                perks            = excluded.perks,
                tagged_skills    = excluded.tagged_skills,
                status           = excluded.status,
                fetched_at       = excluded.fetched_at,
                closes_at        = excluded.closes_at,
                salary_min       = excluded.salary_min,
                salary_max       = excluded.salary_max,
                salary_currency  = excluded.salary_currency,
                salary_period    = excluded.salary_period
            "#,
        )?;
        for (url, detail) in rows {
            stmt.execute(params![
                url,
                detail.description,
                detail.requirements,
                detail.responsibilities,
                detail.perks,
                serde_json::to_string(&detail.tagged_skills)?,
                detail.status.as_str(),
                now,
                detail.closes_at.map(|d| d.to_rfc3339()),
                detail.salary_min,
                detail.salary_max,
                detail.salary_currency,
                detail.salary_period,
            ])?;
        }
    }
    tx.commit()?;
    Ok(rows.len())
}

/// Every posting we have already tried to fetch, successfully or not.
///
/// This is the "already attempted" ledger that makes enrichment incremental.
/// It is returned as a set rather than used as a `NOT IN` subquery because the
/// work queue is now ordered by relevance, which is computed in memory — SQL
/// cannot see a company's tier or the reader's skills.
pub fn urls_with_details(conn: &Connection) -> Result<HashSet<String>> {
    let mut stmt = conn.prepare("SELECT url FROM job_details")?;
    let rows = stmt.query_map([], |row| row.get::<_, String>(0))?;
    Ok(rows.flatten().collect())
}

/// Drops descriptions for postings that have aged out of the cache. Mirrors
/// [`prune_orphan_states`], including sparing anything the user has starred —
/// the saved list renders those from a snapshot and still wants their body.
pub fn prune_orphan_details(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "DELETE FROM job_details
         WHERE url NOT IN (SELECT url FROM items)
           AND url NOT IN (SELECT url FROM item_state WHERE saved_at IS NOT NULL)",
        [],
    )?)
}

/// Upserts one column of `item_state` to a timestamp or NULL, creating the row
/// if this is the first thing the user has done with the item.
fn set_flag(conn: &Connection, url: &str, column: &str, on: bool) -> Result<()> {
    let now = on.then(|| Utc::now().to_rfc3339());
    // `column` is never user input — every caller passes a literal below.
    conn.execute(
        &format!(
            "INSERT INTO item_state (url, {column}) VALUES (?1, ?2)
             ON CONFLICT (url) DO UPDATE SET {column} = excluded.{column}"
        ),
        params![url, now],
    )?;
    Ok(())
}

/// Stars an item. The snapshot is what lets it survive the `items` cache being
/// pruned or rebuilt, so it is written on save and left alone on unsave.
pub fn set_saved(conn: &Connection, url: &str, saved: bool, snapshot: Option<&Item>) -> Result<()> {
    set_flag(conn, url, "saved_at", saved)?;
    if saved {
        if let Some(item) = snapshot {
            conn.execute(
                "UPDATE item_state SET snapshot = ?2 WHERE url = ?1",
                params![url, serde_json::to_string(item)?],
            )?;
        }
    }
    Ok(())
}

pub fn set_dismissed(conn: &Connection, url: &str, dismissed: bool) -> Result<()> {
    set_flag(conn, url, "dismissed_at", dismissed)
}

pub fn mark_seen(conn: &Connection, url: &str) -> Result<()> {
    set_flag(conn, url, "seen_at", true)
}

/// Saved items, newest star first. Reads the snapshot rather than joining
/// `items`, so a role starred two months ago still renders after the cache has
/// pruned it.
pub fn load_saved(conn: &Connection) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(
        "SELECT snapshot, seen_at FROM item_state
         WHERE saved_at IS NOT NULL AND snapshot IS NOT NULL
         ORDER BY saved_at DESC",
    )?;

    let rows = stmt.query_map([], |row| {
        Ok((row.get::<_, String>(0)?, row.get::<_, Option<String>>(1)?))
    })?;

    Ok(rows
        .flatten()
        .filter_map(|(json, seen)| {
            let mut item: Item = serde_json::from_str(&json).ok()?;
            item.saved = true;
            item.seen = seen.is_some();
            Some(item)
        })
        .collect())
}

/// Un-hides everything the user has dismissed. Offered in Settings because a
/// dismissal is otherwise permanent and there is no per-item undo after the
/// card leaves the screen.
pub fn clear_dismissed(conn: &Connection) -> Result<usize> {
    Ok(conn.execute("UPDATE item_state SET dismissed_at = NULL", [])?)
}

/// Wipes everything the app collected — the item cache, the fetched postings,
/// and every save, dismissal and read mark — while leaving `settings` alone, so
/// the field, interests and skills the user typed in survive. `last_refresh`
/// is the one setting that goes: it describes the cache, not the user, and
/// leaving it behind would make the now-empty store look fresh and stop the
/// next non-forced refresh from running.
pub fn clear_data(conn: &Connection) -> Result<()> {
    conn.execute_batch(
        "DELETE FROM item_state;
         DELETE FROM job_details;
         DELETE FROM items;
         DELETE FROM settings WHERE key = 'last_refresh';",
    )?;
    // Deleting rows only frees pages inside the file. The point of the button
    // is that the data is gone, so hand the space back to the disk too.
    conn.execute_batch("VACUUM")?;
    Ok(())
}

/// Drops state rows that no longer matter: seen or dismissed, never saved, and
/// no longer backed by a cached item. Without this the table grows forever as
/// listings churn.
pub fn prune_orphan_states(conn: &Connection) -> Result<usize> {
    Ok(conn.execute(
        "DELETE FROM item_state
         WHERE saved_at IS NULL
           AND url NOT IN (SELECT url FROM items)",
        [],
    )?)
}

pub fn get_setting(conn: &Connection, key: &str) -> Result<Option<String>> {
    Ok(conn
        .query_row(
            "SELECT value FROM settings WHERE key = ?1",
            params![key],
            |r| r.get(0),
        )
        .optional()?)
}

pub fn set_setting(conn: &Connection, key: &str, value: &str) -> Result<()> {
    conn.execute(
        "INSERT INTO settings (key, value) VALUES (?1, ?2)
         ON CONFLICT (key) DO UPDATE SET value = excluded.value",
        params![key, value],
    )?;
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn mem() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        migrate(&conn).unwrap();
        conn
    }

    #[test]
    fn round_trips_items_with_tags() {
        let mut conn = mem();
        let mut it = Item::new(
            "Intern",
            "Simplify",
            ItemType::Internship,
            "https://x.test/1",
        );
        it.location_tags = vec!["Remote".into(), "Canada".into()];
        it.location = Some("Toronto, ON".into());

        save_items(&mut conn, &[it]).unwrap();
        let loaded = db_single(&conn);

        assert_eq!(loaded.title, "Intern");
        assert_eq!(loaded.item_type, ItemType::Internship);
        assert_eq!(loaded.location_tags, vec!["Remote", "Canada"]);
    }

    #[test]
    fn re_scraping_the_same_url_updates_in_place() {
        let mut conn = mem();
        let first = Item::new(
            "Intern",
            "Simplify",
            ItemType::Internship,
            "https://x.test/1",
        );
        let second = Item::new(
            "Intern",
            "Simplify",
            ItemType::Internship,
            "https://x.test/1",
        )
        .with_content("updated");

        save_items(&mut conn, &[first]).unwrap();
        save_items(&mut conn, &[second]).unwrap();

        let all = load_items(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].content_text, "updated");
    }

    #[test]
    fn distinct_roles_sharing_a_title_are_all_kept() {
        // Pitt CSC titles every row "Internship at {company}", so uniqueness on
        // (title, source_platform) would silently drop every role but one.
        let mut conn = mem();
        let rows: Vec<Item> = (0..3)
            .map(|i| {
                Item::new(
                    "Internship at Acme",
                    "Pitt CSC Repo",
                    ItemType::Internship,
                    format!("https://acme.test/apply/{i}"),
                )
            })
            .collect();

        save_items(&mut conn, &rows).unwrap();
        assert_eq!(load_items(&conn).unwrap().len(), 3);
    }

    #[test]
    fn one_item_and_its_detail_read_back_like_the_whole_cache() {
        // What the detail pane does. The list payload carries no description,
        // so this pair is the only path by which one ever reaches the screen —
        // and `load_detail` has to agree with `load_details`, which is what
        // ranking reads.
        let mut conn = mem();
        let item = Item::new("Intern", "S", ItemType::Internship, "https://x.test/1");
        let other = Item::new("Other", "S", ItemType::Internship, "https://x.test/2");
        save_items(&mut conn, &[item.clone(), other]).unwrap();

        let mut detail = JobDetail::with_status(DetailStatus::Ok);
        detail.description = Some("We build data pipelines.".into());
        detail.requirements = Some("Rust\nPostgres".into());
        detail.closes_at = parse_rfc3339("2026-09-01T00:00:00Z");
        save_details(&mut conn, &[(item.url.clone(), detail)]).unwrap();

        let mut one = vec![load_item(&conn, &item.url).unwrap().expect("cached")];
        assert_eq!(one[0].title, "Intern");

        let single = load_detail(&conn, &item.url).unwrap().expect("detail");
        let all = load_details(&conn).unwrap();
        assert_eq!(single.description, all[&item.url].description);
        assert_eq!(single.requirements, all[&item.url].requirements);
        assert_eq!(single.closes_at, all[&item.url].closes_at);

        attach_details(&mut one, &std::iter::once((item.url.clone(), single)).collect());
        assert_eq!(one[0].requirements.as_deref(), Some("Rust\nPostgres"));
        assert!(one[0].closes_at.is_some());

        // A posting with no fetched row, and one the cache never had.
        assert!(load_detail(&conn, "https://x.test/2").unwrap().is_none());
        assert!(load_item(&conn, "https://x.test/gone").unwrap().is_none());
    }

    #[test]
    fn counts_agree_with_the_rows_they_replace() {
        // `feed_status` counts in SQL now instead of materialising 18,240
        // items to call `.len()` on them. The two must not drift.
        let mut conn = mem();
        let rows: Vec<Item> = ["Simplify", "Simplify", "Luma"]
            .iter()
            .enumerate()
            .map(|(i, source)| {
                Item::new("x", *source, ItemType::Article, format!("https://x.test/{i}"))
            })
            .collect();
        save_items(&mut conn, &rows).unwrap();
        set_saved(&conn, "https://x.test/0", true, Some(&rows[0])).unwrap();
        set_dismissed(&conn, "https://x.test/1", true).unwrap();
        mark_seen(&conn, "https://x.test/2").unwrap();

        let items = load_items(&conn).unwrap();
        let states = load_states(&conn).unwrap();
        assert_eq!(count_items(&conn).unwrap(), items.len());
        assert_eq!(state_counts(&conn).unwrap(), (states.saved.len(), states.dismissed.len()));

        let counts = source_counts(&conn).unwrap();
        assert_eq!(counts["Simplify"], 2);
        assert_eq!(counts["Luma"], 1);
        assert_eq!(distinct_sources(&conn).unwrap(), ["Luma", "Simplify"]);
    }

    #[test]
    fn settings_read_write() {
        let conn = mem();
        assert_eq!(get_setting(&conn, "major").unwrap(), None);
        set_setting(&conn, "major", "Software Engineering").unwrap();
        set_setting(&conn, "major", "General").unwrap();
        assert_eq!(
            get_setting(&conn, "major").unwrap().as_deref(),
            Some("General")
        );
    }

    #[test]
    fn prune_drops_only_stale_rows() {
        let mut conn = mem();
        let fresh = Item::new("fresh", "S", ItemType::Article, "https://x.test/f");
        let stale = Item::new("stale", "S", ItemType::Article, "https://x.test/s")
            .with_timestamp(Utc::now() - chrono::Duration::days(90));

        save_items(&mut conn, &[fresh, stale]).unwrap();
        prune_older_than(&conn, 60).unwrap();

        let all = load_items(&conn).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].title, "fresh");
    }

    fn db_single(conn: &Connection) -> Item {
        load_items(conn).unwrap().into_iter().next().unwrap()
    }

    // --- item state ---

    #[test]
    fn saved_and_seen_flags_round_trip() {
        let conn = mem();
        set_saved(&conn, "https://x.test/1", true, None).unwrap();
        mark_seen(&conn, "https://x.test/1").unwrap();
        mark_seen(&conn, "https://x.test/2").unwrap();

        let states = load_states(&conn).unwrap();
        assert!(states.saved.contains("https://x.test/1"));
        assert!(states.seen.contains("https://x.test/1"));
        assert!(states.seen.contains("https://x.test/2"));
        assert!(!states.saved.contains("https://x.test/2"));
        assert!(states.dismissed.is_empty());
    }

    #[test]
    fn unsaving_clears_only_the_saved_flag() {
        let conn = mem();
        let it = Item::new("Intern", "S", ItemType::Internship, "https://x.test/1");
        set_saved(&conn, &it.url, true, Some(&it)).unwrap();
        mark_seen(&conn, &it.url).unwrap();
        set_saved(&conn, &it.url, false, None).unwrap();

        let states = load_states(&conn).unwrap();
        assert!(states.saved.is_empty());
        assert!(states.seen.contains("https://x.test/1"));
    }

    #[test]
    fn a_saved_item_outlives_the_pruned_cache() {
        // The whole reason `snapshot` exists: star something, let the items
        // cache drop it, and the saved list must still render it.
        let mut conn = mem();
        let it = Item::new(
            "Intern",
            "Simplify",
            ItemType::Internship,
            "https://x.test/1",
        )
        .with_content("Toronto, ON");
        save_items(&mut conn, std::slice::from_ref(&it)).unwrap();
        set_saved(&conn, &it.url, true, Some(&it)).unwrap();

        conn.execute("DELETE FROM items", []).unwrap();

        let saved = load_saved(&conn).unwrap();
        assert_eq!(saved.len(), 1);
        assert_eq!(saved[0].title, "Intern");
        assert_eq!(saved[0].content_text, "Toronto, ON");
        assert!(saved[0].saved);
    }

    #[test]
    fn saved_list_is_newest_star_first() {
        let conn = mem();
        for i in 0..3 {
            let it = Item::new(
                format!("item-{i}"),
                "S",
                ItemType::Article,
                format!("https://x.test/{i}"),
            );
            set_saved(&conn, &it.url, true, Some(&it)).unwrap();
            // saved_at is an RFC3339 string; without a gap the ORDER BY is a coin flip.
            std::thread::sleep(std::time::Duration::from_millis(2));
        }
        let saved = load_saved(&conn).unwrap();
        assert_eq!(saved[0].title, "item-2");
        assert_eq!(saved[2].title, "item-0");
    }

    #[test]
    fn annotate_stamps_flags_onto_matching_urls() {
        let conn = mem();
        set_saved(&conn, "https://x.test/1", true, None).unwrap();
        mark_seen(&conn, "https://x.test/2").unwrap();
        let states = load_states(&conn).unwrap();

        let mut items = vec![
            Item::new("a", "S", ItemType::Article, "https://x.test/1"),
            Item::new("b", "S", ItemType::Article, "https://x.test/2"),
            Item::new("c", "S", ItemType::Article, "https://x.test/3"),
        ];
        annotate(&mut items, &states);

        assert!(items[0].saved && !items[0].seen);
        assert!(!items[1].saved && items[1].seen);
        assert!(!items[2].saved && !items[2].seen);
    }

    #[test]
    fn clear_dismissed_unhides_everything() {
        let conn = mem();
        set_dismissed(&conn, "https://x.test/1", true).unwrap();
        set_dismissed(&conn, "https://x.test/2", true).unwrap();
        assert_eq!(load_states(&conn).unwrap().dismissed.len(), 2);

        clear_dismissed(&conn).unwrap();
        assert!(load_states(&conn).unwrap().dismissed.is_empty());
    }

    #[test]
    fn pruning_orphan_states_spares_saved_rows() {
        let mut conn = mem();
        let cached = Item::new("cached", "S", ItemType::Article, "https://x.test/live");
        save_items(&mut conn, std::slice::from_ref(&cached)).unwrap();

        mark_seen(&conn, &cached.url).unwrap(); // seen + still cached  → kept
        mark_seen(&conn, "https://x.test/gone").unwrap(); // seen + uncached  → dropped
        let starred = Item::new("starred", "S", ItemType::Article, "https://x.test/star");
        set_saved(&conn, &starred.url, true, Some(&starred)).unwrap(); // saved + uncached → kept

        prune_orphan_states(&conn).unwrap();

        let states = load_states(&conn).unwrap();
        assert!(states.seen.contains("https://x.test/live"));
        assert!(!states.seen.contains("https://x.test/gone"));
        assert!(states.saved.contains("https://x.test/star"));
    }

    #[test]
    fn a_schema_bump_keeps_fetched_descriptions_and_clears_the_misses() {
        let mut conn = mem();
        let rows = [
            ("https://x.test/served", DetailStatus::Ok),
            ("https://x.test/unsupported", DetailStatus::Unsupported),
            ("https://x.test/failed", DetailStatus::Failed),
            ("https://x.test/empty", DetailStatus::Empty),
        ]
        .map(|(url, status)| {
            let mut detail = JobDetail::with_status(status);
            detail.requirements = Some("Rust".into());
            (url.to_string(), detail)
        });
        save_details(&mut conn, &rows).unwrap();

        conn.pragma_update(None, "user_version", 0).unwrap();
        migrate(&conn).unwrap();

        // The three misses are what a newly added handler exists to serve, so
        // they go back on the queue; the one that worked is kept.
        let kept: Vec<_> = load_details(&conn).unwrap().into_keys().collect();
        assert_eq!(kept, ["https://x.test/served"]);
    }

    #[test]
    fn clear_data_wipes_the_store_but_not_the_profile() {
        let mut conn = mem();
        let item = Item::new("Intern", "S", ItemType::Internship, "https://x.test/1");
        save_items(&mut conn, std::slice::from_ref(&item)).unwrap();
        set_saved(&conn, &item.url, true, Some(&item)).unwrap();
        set_dismissed(&conn, "https://x.test/2", true).unwrap();
        save_details(
            &mut conn,
            &[(item.url.clone(), JobDetail::with_status(DetailStatus::Ok))],
        )
        .unwrap();
        set_setting(&conn, "skills", r#"["Rust"]"#).unwrap();
        set_setting(&conn, "last_refresh", "2026-01-01T00:00:00Z").unwrap();

        clear_data(&conn).unwrap();

        assert!(load_items(&conn).unwrap().is_empty());
        assert!(load_saved(&conn).unwrap().is_empty());
        assert!(load_details(&conn).unwrap().is_empty());
        let states = load_states(&conn).unwrap();
        assert!(states.saved.is_empty() && states.dismissed.is_empty());
        // The profile stays; the cache's own timestamp does not.
        assert_eq!(
            get_setting(&conn, "skills").unwrap().as_deref(),
            Some(r#"["Rust"]"#)
        );
        assert!(get_setting(&conn, "last_refresh").unwrap().is_none());
    }

    #[test]
    fn item_state_survives_a_schema_bump() {
        // `items` is dropped and rebuilt on migrate; user actions must not be.
        let conn = mem();
        set_saved(&conn, "https://x.test/1", true, None).unwrap();

        conn.pragma_update(None, "user_version", 0).unwrap();
        migrate(&conn).unwrap();

        assert!(load_states(&conn)
            .unwrap()
            .saved
            .contains("https://x.test/1"));
    }
}
