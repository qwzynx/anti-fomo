//! Local SQLite store. Unlike the old backend — which held scraped items in a
//! module-global dict that died with the process — items are persisted here, so
//! the app paints instantly on launch and keeps working offline.

use anyhow::Result;
use chrono::{DateTime, TimeZone, Utc};
use rusqlite::{params, Connection, OptionalExtension};

use crate::models::{Item, ItemType};

const SCHEMA_VERSION: i32 = 2;

pub fn open(path: &std::path::Path) -> Result<Connection> {
    if let Some(parent) = path.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let conn = Connection::open(path)?;
    // WAL keeps a background refresh from blocking reads by the UI.
    conn.pragma_update(None, "journal_mode", "WAL")?;
    conn.pragma_update(None, "foreign_keys", "ON")?;
    migrate(&conn)?;
    Ok(conn)
}

fn migrate(conn: &Connection) -> Result<()> {
    let version: i32 = conn.pragma_query_value(None, "user_version", |r| r.get(0))?;
    if version >= SCHEMA_VERSION {
        return Ok(());
    }

    // `items` is a rebuildable cache, so schema changes just drop and refetch
    // rather than carrying migration logic. `settings` holds the only data
    // worth keeping and is never dropped.
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
            -- URL is the listing's real identity. The old backend declared
            -- uniqueness on (title, source_platform), but it never actually
            -- wrote to the database, so the flaw never surfaced: Pitt CSC
            -- titles every row "Internship at {company}", which would collapse
            -- every distinct role at one company into a single item.
            UNIQUE (url)
        );

        CREATE INDEX IF NOT EXISTS idx_items_type ON items (item_type);
        CREATE INDEX IF NOT EXISTS idx_items_timestamp ON items (timestamp DESC);

        CREATE TABLE IF NOT EXISTS settings (
            key   TEXT PRIMARY KEY,
            value TEXT NOT NULL
        );
        "#,
    )?;
    conn.pragma_update(None, "user_version", SCHEMA_VERSION)?;
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
                 timestamp, discipline, relevance_score, location, location_tags)
            VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10)
            ON CONFLICT (url) DO UPDATE SET
                title           = excluded.title,
                source_platform = excluded.source_platform,
                item_type       = excluded.item_type,
                content_text    = excluded.content_text,
                timestamp       = excluded.timestamp,
                discipline      = excluded.discipline,
                relevance_score = excluded.relevance_score,
                location        = excluded.location,
                location_tags   = excluded.location_tags
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
            ])?;
        }
    }
    tx.commit()?;
    Ok(items.len())
}

pub fn load_items(conn: &Connection) -> Result<Vec<Item>> {
    let mut stmt = conn.prepare(
        "SELECT title, source_platform, item_type, url, content_text,
                timestamp, discipline, relevance_score, location, location_tags
         FROM items ORDER BY timestamp DESC",
    )?;

    let rows = stmt.query_map([], |row| {
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
        })
    })?;

    Ok(rows.filter_map(Result::ok).collect())
}

/// Drops entries older than `days` so the local store doesn't grow forever.
/// Job postings and news both go stale fast; nothing here is worth archiving.
pub fn prune_older_than(conn: &Connection, days: i64) -> Result<usize> {
    let cutoff = (Utc::now() - chrono::Duration::days(days)).to_rfc3339();
    Ok(conn.execute("DELETE FROM items WHERE timestamp < ?1", params![cutoff])?)
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
            "Levels.fyi",
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
            "Levels.fyi",
            ItemType::Internship,
            "https://x.test/1",
        );
        let second = Item::new(
            "Intern",
            "Levels.fyi",
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
}
