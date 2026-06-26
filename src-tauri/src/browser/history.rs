//! browser/history.rs
//!
//! Browser history stored in SQLite — same database as the search index.
//!
//! Table: browser_history
//!   id          INTEGER PRIMARY KEY
//!   url         TEXT NOT NULL
//!   title       TEXT
//!   visit_time  INTEGER NOT NULL  (UNIX seconds)
//!   visit_count INTEGER DEFAULT 1
//!
//! Features:
//!   - add_visit   (upsert: increment count if URL seen today)
//!   - recent      (last N visits)
//!   - search      (LIKE-based full-text across url + title)
//!   - clear_all
//!   - clear_before(timestamp)

use std::path::PathBuf;
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

#[derive(Debug, Clone, Serialize)]
pub struct HistoryEntry {
    pub id:          i64,
    pub url:         String,
    pub title:       String,
    pub visit_time:  i64,
    pub visit_count: i64,
    /// Formatted date string (e.g. "Today", "Yesterday", "2024-01-15")
    pub date_label:  String,
}

pub struct HistoryStore {
    db_path: PathBuf,
}

impl HistoryStore {
    pub fn open() -> rusqlite::Result<Self> {
        let db_path = crate::search::indexer::db_path();
        let store = Self { db_path };
        store.init_schema()?;
        Ok(store)
    }

    fn connect(&self) -> rusqlite::Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        Ok(conn)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        self.connect()?.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS browser_history (
                id          INTEGER PRIMARY KEY AUTOINCREMENT,
                url         TEXT    NOT NULL,
                title       TEXT    NOT NULL DEFAULT '',
                visit_time  INTEGER NOT NULL,
                visit_count INTEGER NOT NULL DEFAULT 1,
                UNIQUE(url)
            );
            CREATE INDEX IF NOT EXISTS idx_history_time ON browser_history(visit_time DESC);
        "#)?;
        Ok(())
    }

    /// Record a visit. If the URL was visited today, increment count.
    /// Otherwise insert a new row.
    pub fn add_visit(&self, url: &str, title: &str) -> rusqlite::Result<()> {
        let now  = now_secs();
        let conn = self.connect()?;

        // Check if URL exists
        let existing: Option<(i64, i64)> = conn
            .query_row(
                "SELECT id, visit_time FROM browser_history WHERE url = ?1",
                [url],
                |r| Ok((r.get(0)?, r.get(1)?)),
            )
            .optional()?;

        if let Some((id, last_visit)) = existing {
            // Update: increment count + refresh time
            conn.execute(
                "UPDATE browser_history
                 SET visit_count = visit_count + 1,
                     visit_time  = ?1,
                     title       = CASE WHEN ?2 != '' THEN ?2 ELSE title END
                 WHERE id = ?3",
                params![now, title, id],
            )?;
        } else {
            conn.execute(
                "INSERT INTO browser_history(url, title, visit_time, visit_count)
                 VALUES(?1, ?2, ?3, 1)",
                params![url, title, now],
            )?;
        }

        Ok(())
    }

    /// Return the most recent `limit` history entries.
    pub fn recent(&self, limit: usize) -> rusqlite::Result<Vec<HistoryEntry>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, url, title, visit_time, visit_count
             FROM browser_history
             ORDER BY visit_time DESC
             LIMIT ?1"
        )?;

        let now = now_secs();
        let rows = stmt.query_map([limit as i64], |r| {
            let vt: i64 = r.get(3)?;
            Ok(HistoryEntry {
                id:          r.get(0)?,
                url:         r.get(1)?,
                title:       r.get(2)?,
                visit_time:  vt,
                visit_count: r.get(4)?,
                date_label:  format_date_label(vt, now),
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Search history by URL or title (case-insensitive LIKE).
    pub fn search(&self, query: &str) -> rusqlite::Result<Vec<HistoryEntry>> {
        let conn  = self.connect()?;
        let pat   = format!("%{}%", query.replace('%', "\\%").replace('_', "\\_"));
        let mut stmt = conn.prepare(
            "SELECT id, url, title, visit_time, visit_count
             FROM browser_history
             WHERE url LIKE ?1 ESCAPE '\\' OR title LIKE ?1 ESCAPE '\\'
             ORDER BY visit_time DESC
             LIMIT 100"
        )?;

        let now = now_secs();
        let rows = stmt.query_map([&pat], |r| {
            let vt: i64 = r.get(3)?;
            Ok(HistoryEntry {
                id:          r.get(0)?,
                url:         r.get(1)?,
                title:       r.get(2)?,
                visit_time:  vt,
                visit_count: r.get(4)?,
                date_label:  format_date_label(vt, now),
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Delete all history.
    pub fn clear_all(&self) -> rusqlite::Result<usize> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM browser_history", [])
    }

    /// Delete history entries older than `before_secs` (UNIX timestamp).
    pub fn clear_before(&self, before_secs: i64) -> rusqlite::Result<usize> {
        let conn = self.connect()?;
        conn.execute(
            "DELETE FROM browser_history WHERE visit_time < ?1",
            [before_secs],
        )
    }

    /// Delete a single entry by id.
    pub fn delete(&self, id: i64) -> rusqlite::Result<()> {
        let conn = self.connect()?;
        conn.execute("DELETE FROM browser_history WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Return total number of stored URLs.
    pub fn count(&self) -> rusqlite::Result<i64> {
        let conn = self.connect()?;
        conn.query_row("SELECT COUNT(*) FROM browser_history", [], |r| r.get(0))
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Convert a UNIX timestamp to a human-readable date label.
fn format_date_label(visit_time: i64, now: i64) -> String {
    let age_secs = now - visit_time;
    if age_secs < 60 { return "Just now".into(); }
    if age_secs < 3600 {
        let m = age_secs / 60;
        return format!("{} min ago", m);
    }
    if age_secs < 86400 {
        let h = age_secs / 3600;
        return format!("{} hr ago", h);
    }
    if age_secs < 86400 * 2 { return "Yesterday".into(); }
    // Format as YYYY-MM-DD using simple arithmetic (no chrono for this)
    let days_ago = age_secs / 86400;
    if days_ago < 7 { return format!("{} days ago", days_ago); }
    // For older entries: compute YYYY-MM-DD purely from UNIX timestamp (no chrono)
    let z        = visit_time / 86400 + 719468;
    let era      = (if z >= 0 { z } else { z - 146096 }) / 146097;
    let doe      = z - era * 146097;
    let yoe      = (doe - doe/1460 + doe/36524 - doe/146096) / 365;
    let y        = yoe + era * 400;
    let doy      = doe - (365*yoe + yoe/4 - yoe/100);
    let mp       = (5*doy + 2) / 153;
    let d        = doy - (153*mp + 2)/5 + 1;
    let m        = if mp < 10 { mp + 3 } else { mp - 9 };
    let yr       = if m <= 2 { y + 1 } else { y };
    format!("{:04}-{:02}-{:02}", yr, m, d)
}
