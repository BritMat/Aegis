//! browser/userscripts.rs
//!
//! Tampermonkey-compatible userscript manager.
//!
//! Script storage:  ~/.bm-aegis/userscripts/<id>.json (metadata + source)
//! GM_ storage:     ~/.bm-aegis/search-index.db  → gm_storage table
//!
//! Header format parsed:
//!   // ==UserScript==
//!   // @name         My Script
//!   // @namespace    http://tampermonkey.net/
//!   // @version      1.0
//!   // @description  Does something useful
//!   // @author       You
//!   // @match        https://example.com/*
//!   // @match        https://*.example.com/*
//!   // @exclude      https://example.com/private/*
//!   // @include      /regex pattern/
//!   // @grant        GM_getValue
//!   // @grant        GM_xmlhttpRequest
//!   // @run-at       document-idle
//!   // ==/UserScript==

use std::{
    collections::HashMap,
    path::PathBuf,
};
use serde::{Deserialize, Serialize};
use regex::Regex;
use rusqlite::{Connection, OptionalExtension, params};
use uuid::Uuid;

// ═══════════════════════════════════════════════════════════════════════════
//  Data types
// ═══════════════════════════════════════════════════════════════════════════

/// Serialisable script metadata (shown in the UI and stored to disk).
#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct ScriptMeta {
    pub id:          String,
    pub name:        String,
    pub namespace:   String,
    pub version:     String,
    pub description: String,
    pub author:      String,
    pub matches:     Vec<String>,
    pub excludes:    Vec<String>,
    pub includes:    Vec<String>,
    pub grants:      Vec<String>,
    pub run_at:      String,    // "document-start" | "document-end" | "document-idle"
    pub enabled:     bool,
    pub installed_at: String,   // ISO 8601
}

/// What gets sent to the browser webview for injection.
#[derive(Debug, Clone, Serialize)]
pub struct ScriptPayload {
    pub id:          String,
    pub name:        String,
    pub namespace:   String,
    pub version:     String,
    pub description: String,
    pub author:      String,
    pub grants:      Vec<String>,
    pub code:        String,
}

/// On-disk format: metadata + raw source.
#[derive(Debug, Clone, Serialize, Deserialize)]
struct ScriptRecord {
    #[serde(flatten)]
    pub meta:   ScriptMeta,
    pub source: String,
}

// ═══════════════════════════════════════════════════════════════════════════
//  UserscriptManager
// ═══════════════════════════════════════════════════════════════════════════

pub struct UserscriptManager {
    dir: PathBuf,
}

impl UserscriptManager {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let dir = scripts_dir();
        std::fs::create_dir_all(&dir)?;
        Ok(Self { dir })
    }

    // ──────────────────────────────────────────────────────────────────────
    // CRUD
    // ──────────────────────────────────────────────────────────────────────

    pub fn list(&self) -> Result<Vec<ScriptMeta>, Box<dyn std::error::Error>> {
        let mut scripts: Vec<ScriptMeta> = Vec::new();
        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path  = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") { continue; }
            if let Ok(text) = std::fs::read_to_string(&path) {
                if let Ok(rec) = serde_json::from_str::<ScriptRecord>(&text) {
                    scripts.push(rec.meta);
                }
            }
        }
        scripts.sort_by(|a, b| a.name.cmp(&b.name));
        Ok(scripts)
    }

    pub fn install(&mut self, source: &str) -> Result<ScriptMeta, Box<dyn std::error::Error>> {
        let header = parse_header(source)?;
        let id     = Uuid::new_v4().to_string();

        let meta = ScriptMeta {
            id:           id.clone(),
            name:         header.get("name")       .cloned().unwrap_or_else(|| "Unnamed Script".into()),
            namespace:    header.get("namespace")  .cloned().unwrap_or_default(),
            version:      header.get("version")    .cloned().unwrap_or_else(|| "0.0.0".into()),
            description:  header.get("description").cloned().unwrap_or_default(),
            author:       header.get("author")     .cloned().unwrap_or_default(),
            matches:      header.get_all("match"),
            excludes:     header.get_all("exclude"),
            includes:     header.get_all("include"),
            grants:       header.get_all("grant"),
            run_at:       header.get("run-at").cloned().unwrap_or_else(|| "document-idle".into()),
            enabled:      true,
            installed_at: chrono::Utc::now().to_rfc3339(),
        };

        // Extract code (everything after ==/UserScript==)
        let code = extract_code(source);

        let record = ScriptRecord { meta: meta.clone(), source: source.to_string() };
        let json   = serde_json::to_string_pretty(&record)?;
        std::fs::write(self.dir.join(format!("{}.json", id)), json)?;

        Ok(meta)
    }

    pub fn remove(&mut self, id: &str) -> Result<(), Box<dyn std::error::Error>> {
        let path = self.dir.join(format!("{}.json", id));
        if path.exists() {
            std::fs::remove_file(path)?;
        }
        // Also remove GM_ storage for this script
        GmStorage::new()?.purge_script(id)?;
        Ok(())
    }

    pub fn set_enabled(&mut self, id: &str, enabled: bool)
        -> Result<(), Box<dyn std::error::Error>>
    {
        let path = self.dir.join(format!("{}.json", id));
        let text = std::fs::read_to_string(&path)?;
        let mut rec: ScriptRecord = serde_json::from_str(&text)?;
        rec.meta.enabled = enabled;
        std::fs::write(path, serde_json::to_string_pretty(&rec)?)?;
        Ok(())
    }

    // ──────────────────────────────────────────────────────────────────────
    // URL matching
    // ──────────────────────────────────────────────────────────────────────

    /// Return payloads for all enabled scripts that match `url`.
    pub fn scripts_for_url(&self, url: &str)
        -> Result<Vec<ScriptPayload>, Box<dyn std::error::Error>>
    {
        let mut result = Vec::new();

        for entry in std::fs::read_dir(&self.dir)? {
            let entry = entry?;
            let path  = entry.path();
            if path.extension().and_then(|e| e.to_str()) != Some("json") { continue; }

            let text = match std::fs::read_to_string(&path) {
                Ok(t)  => t,
                Err(_) => continue,
            };
            let rec: ScriptRecord = match serde_json::from_str(&text) {
                Ok(r)  => r,
                Err(_) => continue,
            };

            if !rec.meta.enabled { continue; }

            let code = extract_code(&rec.source);

            // Check @match patterns
            let matched = rec.meta.matches.iter().any(|p| url_matches(url, p))
                || rec.meta.includes.iter().any(|p| url_matches_include(url, p));

            // Check @exclude patterns
            let excluded = rec.meta.excludes.iter().any(|p| url_matches(url, p));

            if matched && !excluded {
                result.push(ScriptPayload {
                    id:          rec.meta.id,
                    name:        rec.meta.name,
                    namespace:   rec.meta.namespace,
                    version:     rec.meta.version,
                    description: rec.meta.description,
                    author:      rec.meta.author,
                    grants:      rec.meta.grants,
                    code,
                });
            }
        }

        Ok(result)
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  GM_ persistent key-value storage
// ═══════════════════════════════════════════════════════════════════════════

pub struct GmStorage {
    conn: Connection,
}

impl GmStorage {
    pub fn new() -> Result<Self, Box<dyn std::error::Error>> {
        let conn = Connection::open(crate::search::indexer::db_path())?;
        conn.execute_batch(
            "CREATE TABLE IF NOT EXISTS gm_storage (
                script_id TEXT NOT NULL,
                key       TEXT NOT NULL,
                value     TEXT NOT NULL,
                PRIMARY KEY (script_id, key)
            );"
        )?;
        Ok(Self { conn })
    }

    pub fn get(&self, script_id: &str, key: &str, default_val: serde_json::Value)
        -> Result<serde_json::Value, rusqlite::Error>
    {
        let row: Option<String> = self.conn
            .query_row(
                "SELECT value FROM gm_storage WHERE script_id=?1 AND key=?2",
                params![script_id, key],
                |r| r.get(0),
            )
            .optional()?;

        match row {
            Some(json_str) => Ok(serde_json::from_str(&json_str).unwrap_or(default_val)),
            None           => Ok(default_val),
        }
    }

    pub fn set(&self, script_id: &str, key: &str, value: serde_json::Value)
        -> Result<(), rusqlite::Error>
    {
        let json_str = serde_json::to_string(&value).unwrap_or_default();
        self.conn.execute(
            "INSERT OR REPLACE INTO gm_storage(script_id, key, value) VALUES(?1,?2,?3)",
            params![script_id, key, json_str],
        )?;
        Ok(())
    }

    pub fn delete(&self, script_id: &str, key: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM gm_storage WHERE script_id=?1 AND key=?2",
            params![script_id, key],
        )?;
        Ok(())
    }

    pub fn list_keys(&self, script_id: &str) -> Result<Vec<String>, rusqlite::Error> {
        let mut stmt = self.conn.prepare(
            "SELECT key FROM gm_storage WHERE script_id=?1"
        )?;
        let keys: Vec<String> = stmt
            .query_map(params![script_id], |r| r.get(0))?
            .filter_map(|r| r.ok())
            .collect();
        Ok(keys)
    }

    pub fn purge_script(&self, script_id: &str) -> Result<(), rusqlite::Error> {
        self.conn.execute(
            "DELETE FROM gm_storage WHERE script_id=?1",
            params![script_id],
        )?;
        Ok(())
    }
}

// ═══════════════════════════════════════════════════════════════════════════
//  Header parser
// ═══════════════════════════════════════════════════════════════════════════

struct Header(Vec<(String, String)>);

impl Header {
    fn get(&self, key: &str) -> Option<&String> {
        self.0.iter().find(|(k, _)| k == key).map(|(_, v)| v)
    }
    fn get_all(&self, key: &str) -> Vec<String> {
        self.0.iter().filter(|(k, _)| k == key).map(|(_, v)| v.clone()).collect()
    }
}

fn parse_header(source: &str) -> Result<Header, Box<dyn std::error::Error>> {
    let in_header  = std::cell::Cell::new(false);
    let mut pairs  = Vec::new();
    let line_re    = Regex::new(r"^//\s*@(\S+)\s*(.*?)\s*$")?;

    for line in source.lines() {
        let trimmed = line.trim();
        if trimmed == "// ==UserScript==" || trimmed == "// ==UserScript== " {
            in_header.set(true);
            continue;
        }
        if trimmed == "// ==/UserScript==" || trimmed == "// ==/UserScript== " {
            break;
        }
        if in_header.get() {
            if let Some(cap) = line_re.captures(trimmed) {
                pairs.push((
                    cap[1].trim().to_ascii_lowercase(),
                    cap[2].trim().to_string(),
                ));
            }
        }
    }

    if pairs.is_empty() && !source.contains("==UserScript==") {
        return Err("Source does not contain a ==UserScript== header".into());
    }

    Ok(Header(pairs))
}

fn extract_code(source: &str) -> String {
    let mut past_header = false;
    let mut lines: Vec<&str> = Vec::new();

    for line in source.lines() {
        let t = line.trim();
        if t == "// ==/UserScript==" || t == "// ==/UserScript== " {
            past_header = true;
            continue;
        }
        if past_header {
            lines.push(line);
        }
    }

    lines.join("\n").trim().to_string()
}

// ═══════════════════════════════════════════════════════════════════════════
//  URL matching — Tampermonkey @match pattern spec
// ═══════════════════════════════════════════════════════════════════════════

/// Match a URL against a Tampermonkey @match pattern.
/// Pattern format: <scheme>://<host><path>
///   scheme: *, http, https, ftp, file
///   host:   *, *.example.com, example.com
///   path:   /*, /page, /dir/*
fn url_matches(url: &str, pattern: &str) -> bool {
    if pattern == "<all_urls>" || pattern == "*" { return true; }

    let re = match match_pattern_to_regex(pattern) {
        Ok(r)  => r,
        Err(_) => return false,
    };

    re.is_match(url)
}

/// Match against @include patterns (which can be globs OR /regex/).
fn url_matches_include(url: &str, pattern: &str) -> bool {
    // /regex/ format
    if pattern.starts_with('/') && pattern.ends_with('/') && pattern.len() > 2 {
        let inner = &pattern[1..pattern.len() - 1];
        return Regex::new(inner).map(|re| re.is_match(url)).unwrap_or(false);
    }
    // Treat as a @match-style glob
    url_matches(url, pattern)
}

fn match_pattern_to_regex(pattern: &str) -> Result<Regex, Box<dyn std::error::Error>> {
    // Split into scheme, host+path
    let (scheme_pat, rest) = if let Some(pos) = pattern.find("://") {
        (&pattern[..pos], &pattern[pos + 3..])
    } else {
        return Err("Invalid @match pattern".into());
    };

    let (host_pat, path_pat) = if let Some(pos) = rest.find('/') {
        (&rest[..pos], &rest[pos..])
    } else {
        (rest, "/")
    };

    // Build regex parts
    let scheme_re = if scheme_pat == "*" {
        "https?|ftp|file".to_string()
    } else {
        regex::escape(scheme_pat)
    };

    let host_re = if host_pat == "*" {
        "[^/]+".to_string()
    } else if let Some(stripped) = host_pat.strip_prefix("*.") {
        format!("([^/]+\\.)?{}", regex::escape(stripped))
    } else {
        regex::escape(host_pat)
    };

    let path_re = glob_to_regex(path_pat);

    let full = format!(r"^(?:{})://{}{}$", scheme_re, host_re, path_re);
    Ok(Regex::new(&full)?)
}

/// Convert a simple glob path (containing * and ?) to a regex fragment.
fn glob_to_regex(glob: &str) -> String {
    let mut re = String::new();
    for ch in glob.chars() {
        match ch {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            c   => re.push_str(&regex::escape(&c.to_string())),
        }
    }
    re
}

fn scripts_dir() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bm-aegis")
        .join("userscripts")
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn match_exact_url() {
        assert!(url_matches("https://example.com/page", "https://example.com/page"));
    }

    #[test]
    fn match_wildcard_path() {
        assert!(url_matches("https://example.com/any/path", "https://example.com/*"));
    }

    #[test]
    fn match_wildcard_subdomain() {
        assert!(url_matches("https://sub.example.com/page", "https://*.example.com/*"));
        assert!(url_matches("https://example.com/page", "https://*.example.com/*"));
    }

    #[test]
    fn all_urls_matches_everything() {
        assert!(url_matches("https://anywhere.com/foo", "<all_urls>"));
    }

    #[test]
    fn does_not_match_different_domain() {
        assert!(!url_matches("https://other.com/page", "https://example.com/*"));
    }

    #[test]
    fn parse_basic_header() {
        let src = r#"
// ==UserScript==
// @name         Test Script
// @version      1.2.3
// @match        https://example.com/*
// @grant        GM_getValue
// ==/UserScript==
(function() { 'use strict'; })();
"#;
        let header = parse_header(src).unwrap();
        assert_eq!(header.get("name").unwrap(), "Test Script");
        assert_eq!(header.get("version").unwrap(), "1.2.3");
        assert_eq!(header.get_all("match"), vec!["https://example.com/*"]);
        assert_eq!(header.get_all("grant"), vec!["GM_getValue"]);
    }
}
