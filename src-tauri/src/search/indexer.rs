//! search/indexer.rs
//!
//! Local full-text search index backed by SQLite FTS5.
//!
//! Schema
//! ──────
//!   docs          — FTS5 virtual table (path, title, content)
//!   doc_meta      — tracks mtime so we can skip unchanged files
//!
//! The SQLite file lives at: ~/.bm-aegis/search-index.db
//! Bundled SQLite (rusqlite "bundled") is used so no system SQLite is needed
//! and FTS5 support is guaranteed regardless of the host Windows install.

use std::{
    path::{Path, PathBuf},
    time::{SystemTime, UNIX_EPOCH},
};
use rusqlite::{Connection, OptionalExtension, params};
use serde::Serialize;

/// Extensions we bother indexing (binary files skipped automatically by size check).
const INDEXABLE_EXTS: &[&str] = &[
    "html", "htm", "css", "js", "ts", "jsx", "tsx",
    "md",   "txt", "json", "xml", "svg", "yaml", "yml",
    "toml", "ini", "cfg", "conf", "env", "sh",
    "rs",   "py",  "rb",  "go",  "java", "kt", "cs", "cpp", "c", "h",
];

/// Max file size we'll index (1 MB). Larger files are skipped.
const MAX_FILE_BYTES: u64 = 1_024 * 1_024;

#[derive(Debug, Default)]
pub struct IndexRunStats {
    pub indexed: usize,
    pub skipped: usize,
}

#[derive(Debug, Clone, Serialize)]
pub struct SearchResult {
    pub path:    String,
    pub title:   String,
    pub snippet: String,
    pub score:   f64,
}

/// Encapsulates the SQLite connection + FTS5 operations.
pub struct SearchIndexer {
    db_path: PathBuf,
}

impl SearchIndexer {
    /// Open (or create) the index database.
    pub fn open() -> rusqlite::Result<Self> {
        let db_path = db_path();
        if let Some(parent) = db_path.parent() {
            std::fs::create_dir_all(parent).ok();
        }
        let inst = SearchIndexer { db_path };
        inst.init_schema()?;
        Ok(inst)
    }

    fn connect(&self) -> rusqlite::Result<Connection> {
        let conn = Connection::open(&self.db_path)?;
        // WAL mode = better concurrent read performance
        conn.execute_batch("PRAGMA journal_mode=WAL; PRAGMA synchronous=NORMAL;")?;
        Ok(conn)
    }

    fn init_schema(&self) -> rusqlite::Result<()> {
        let conn = self.connect()?;
        conn.execute_batch(r#"
            -- FTS5 virtual table: porter stemmer for English, ascii tokenizer
            CREATE VIRTUAL TABLE IF NOT EXISTS docs USING fts5(
                path UNINDEXED,
                title,
                content,
                tokenize = 'porter unicode61'
            );

            -- Metadata table to detect unchanged files (skip re-indexing)
            CREATE TABLE IF NOT EXISTS doc_meta (
                path  TEXT PRIMARY KEY,
                mtime INTEGER NOT NULL
            );

            -- GM_ API storage (used by browser/userscripts.rs)
            CREATE TABLE IF NOT EXISTS gm_storage (
                script_id TEXT NOT NULL,
                key       TEXT NOT NULL,
                value     TEXT NOT NULL,
                PRIMARY KEY (script_id, key)
            );
        "#)?;
        Ok(())
    }

    // ──────────────────────────────────────────────────────────────────────
    // Indexing
    // ──────────────────────────────────────────────────────────────────────

    /// Walk `dir` and index all eligible text files.
    /// Already-indexed files whose mtime hasn't changed are skipped.
    pub fn index_directory(&self, dir: &Path) -> rusqlite::Result<IndexRunStats> {
        let conn = self.connect()?;
        let mut stats = IndexRunStats::default();

        for entry in walkdir::WalkDir::new(dir)
            .max_depth(12)
            .follow_links(false)
            .into_iter()
            .filter_map(|e| e.ok())
        {
            if !entry.file_type().is_file() { continue; }
            let path = entry.path();

            // Extension check
            if !is_indexable(path) { stats.skipped += 1; continue; }

            // Size check
            let meta = match entry.metadata() {
                Ok(m)  => m,
                Err(_) => { stats.skipped += 1; continue; }
            };
            if meta.len() > MAX_FILE_BYTES { stats.skipped += 1; continue; }

            let path_str = path.to_string_lossy().to_string();
            let mtime    = file_mtime(&meta);

            // Check if up to date
            let existing_mtime: Option<i64> = conn
                .query_row(
                    "SELECT mtime FROM doc_meta WHERE path = ?1",
                    [&path_str],
                    |r| r.get(0),
                )
                .optional()?;

            if existing_mtime == Some(mtime) {
                stats.skipped += 1;
                continue;
            }

            // Read content
            let content = match std::fs::read_to_string(path) {
                Ok(c)  => c,
                Err(_) => { stats.skipped += 1; continue; }
            };

            let title = path
                .file_name()
                .and_then(|n| n.to_str())
                .unwrap_or("")
                .to_string();

            // Remove old entry if it existed
            conn.execute("DELETE FROM docs WHERE path = ?1", [&path_str])?;

            // Insert new
            conn.execute(
                "INSERT INTO docs(path, title, content) VALUES(?1, ?2, ?3)",
                params![path_str, title, content],
            )?;

            // Update metadata
            conn.execute(
                "INSERT OR REPLACE INTO doc_meta(path, mtime) VALUES(?1, ?2)",
                params![path_str, mtime],
            )?;

            stats.indexed += 1;
        }

        Ok(stats)
    }

    #[allow(dead_code)]
    /// Delete all records for files that no longer exist on disk.
    pub fn prune_deleted(&self) -> rusqlite::Result<usize> {
        let paths: Vec<String> = {
        let mut stmt = conn.prepare("SELECT path FROM doc_meta")?;
        let results: Vec<_> = stmt.query_map([], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        results
    };

        let mut pruned = 0;
        for path in paths {
            if !Path::new(&path).exists() {
                conn.execute("DELETE FROM docs     WHERE path = ?1", [&path])?;
                conn.execute("DELETE FROM doc_meta WHERE path = ?1", [&path])?;
                pruned += 1;
            }
        }
        Ok(pruned)
    }

    // ──────────────────────────────────────────────────────────────────────
    // Searching
    // ──────────────────────────────────────────────────────────────────────

    /// Full-text search with BM25 ranking and highlighted snippets.
    pub fn search(&self, query: &str) -> rusqlite::Result<Vec<SearchResult>> {
        if query.trim().is_empty() { return Ok(vec![]); }

        let conn = self.connect()?;

        // Sanitise the query for FTS5 (escape special chars if the query
        // doesn't look like an intentional FTS5 expression).
        let fts_query = sanitise_fts5_query(query);

        let mut stmt = conn.prepare(r#"
            SELECT
                path,
                title,
                snippet(docs, 2, '<mark>', '</mark>', '…', 40),
                rank
            FROM docs
            WHERE docs MATCH ?1
            ORDER BY rank
            LIMIT 60
        "#)?;

        let rows = stmt.query_map([&fts_query], |r| {
            Ok(SearchResult {
                path:    r.get(0)?,
                title:   r.get(1)?,
                snippet: r.get(2)?,
                score:   r.get::<_, f64>(3)?,
            })
        })?;

        Ok(rows.filter_map(|r| r.ok()).collect())
    }

    #[allow(dead_code)]
    /// Return the total number of indexed documents.
    pub fn doc_count(&self) -> rusqlite::Result<i64> {
        let conn = self.connect()?;
        conn.query_row("SELECT COUNT(*) FROM doc_meta", [], |r| r.get(0))
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────

pub fn db_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bm-aegis")
        .join("search-index.db")
}

fn is_indexable(path: &Path) -> bool {
    path.extension()
        .and_then(|e| e.to_str())
        .map(|e| INDEXABLE_EXTS.contains(&e.to_ascii_lowercase().as_str()))
        .unwrap_or(false)
}

fn file_mtime(meta: &std::fs::Metadata) -> i64 {
    meta.modified()
        .ok()
        .and_then(|t| t.duration_since(UNIX_EPOCH).ok())
        .map(|d| d.as_secs() as i64)
        .unwrap_or(0)
}

/// Convert a plain-text query into a safe FTS5 match expression.
/// Wraps each token in double-quotes so special chars are treated literally.
fn sanitise_fts5_query(query: &str) -> String {
    // If it already looks like an FTS5 expression (contains AND/OR/NOT/"), pass through.
    if query.contains('"') || query.to_uppercase().contains(" AND ")
        || query.to_uppercase().contains(" OR ")
        || query.to_uppercase().contains(" NOT ")
    {
        return query.to_string();
    }

    // Otherwise, quote each word so FTS5 treats them as individual terms (OR).
    query
        .split_whitespace()
        .map(|w| format!("\"{}\"", w.replace('"', "")))
        .collect::<Vec<_>>()
        .join(" OR ")
}
