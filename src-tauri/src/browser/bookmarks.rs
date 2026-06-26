//! browser/bookmarks.rs
//!
//! Browser bookmarks stored in the shared SQLite database.
//! Simple: url, title, folder (optional), created_at.
//! Folders are flat strings — no nested tree needed.

use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;
use std::path::PathBuf;

#[derive(Debug, Clone, Serialize)]
pub struct Bookmark {
    pub id:         i64,
    pub url:        String,
    pub title:      String,
    pub folder:     String,
    pub created_at: i64,
    pub favicon:    String,   // data-uri or empty
}

#[allow(dead_code)]
pub struct BookmarkStore {
    db_path: PathBuf,
}

impl BookmarkStore {
    pub fn open() -> rusqlite::Result<Self> {
        let db_path = crate::search::indexer::db_path();
        let store   = Self { db_path };
        store.init_schema()?;
        Ok(store)
    }

    fn connect(&self) -> rusqlite::Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        conn.execute_batch("PRAGMA journal_mode=WAL;")?;
        Ok(conn)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        self.connect()?.execute_batch(r#"
            CREATE TABLE IF NOT EXISTS bookmarks (
                id         INTEGER PRIMARY KEY AUTOINCREMENT,
                url        TEXT NOT NULL UNIQUE,
                title      TEXT NOT NULL DEFAULT '',
                folder     TEXT NOT NULL DEFAULT 'Unsorted',
                created_at INTEGER NOT NULL,
                favicon    TEXT NOT NULL DEFAULT ''
            );
            CREATE INDEX IF NOT EXISTS idx_bookmarks_folder ON bookmarks(folder);
        "#)?;
        Ok(())
    }

    /// Add or update a bookmark. Returns the bookmark.
    pub fn add(&self, url: &str, title: &str, folder: &str) -> rusqlite::Result<Bookmark> {
        let conn = self.connect()?;
        let now  = now_secs();
        let folder = if folder.is_empty() { "Unsorted" } else { folder };

        conn.execute(
            "INSERT INTO bookmarks(url, title, folder, created_at)
             VALUES(?1, ?2, ?3, ?4)
             ON CONFLICT(url) DO UPDATE SET title=excluded.title, folder=excluded.folder",
            params![url, title, folder, now],
        )?;

        let id: i64 = conn.query_row(
            "SELECT id FROM bookmarks WHERE url = ?1",
            [url],
            |r| r.get(0),
        )?;

        Ok(Bookmark { id, url: url.into(), title: title.into(),
                      folder: folder.into(), created_at: now, favicon: String::new() })
    }

    /// Remove by id.
    pub fn remove(&self, id: i64) -> rusqlite::Result<()> {
        self.connect()?.execute("DELETE FROM bookmarks WHERE id = ?1", [id])?;
        Ok(())
    }

    /// Return true if url is already bookmarked.
    pub fn is_bookmarked(&self, url: &str) -> rusqlite::Result<bool> {
        let conn = self.connect()?;
        let count: i64 = conn.query_row(
            "SELECT COUNT(*) FROM bookmarks WHERE url = ?1", [url], |r| r.get(0)
        )?;
        Ok(count > 0)
    }

    /// List all bookmarks, grouped by folder (sorted folder, then created_at desc).
    pub fn list_all(&self) -> rusqlite::Result<Vec<Bookmark>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare(
            "SELECT id, url, title, folder, created_at, favicon
             FROM bookmarks ORDER BY folder, created_at DESC"
        )?;
        let rows = stmt.query_map([], |r| Ok(Bookmark {
            id:         r.get(0)?,
            url:        r.get(1)?,
            title:      r.get(2)?,
            folder:     r.get(3)?,
            created_at: r.get(4)?,
            favicon:    r.get(5)?,
        }))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Search by URL or title.
    pub fn search(&self, query: &str) -> rusqlite::Result<Vec<Bookmark>> {
        let conn = self.connect()?;
        let pat  = format!("%{}%", query);
        let mut stmt = conn.prepare(
            "SELECT id, url, title, folder, created_at, favicon
             FROM bookmarks WHERE url LIKE ?1 OR title LIKE ?1
             ORDER BY created_at DESC LIMIT 50"
        )?;
        let rows = stmt.query_map([&pat], |r| Ok(Bookmark {
            id:         r.get(0)?,
            url:        r.get(1)?,
            title:      r.get(2)?,
            folder:     r.get(3)?,
            created_at: r.get(4)?,
            favicon:    r.get(5)?,
        }))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    /// Return list of unique folder names.
    pub fn folders(&self) -> rusqlite::Result<Vec<String>> {
        let conn = self.connect()?;
        let mut stmt = conn.prepare("SELECT DISTINCT folder FROM bookmarks ORDER BY folder")?;
        let rows = stmt.query_map([], |r| r.get::<_, String>(0))?;
        Ok(rows.filter_map(|r| r.ok()).collect())
    }
}

fn now_secs() -> i64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}
