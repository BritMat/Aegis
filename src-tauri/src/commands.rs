//! commands.rs
//! All #[tauri::command] handlers. Each delegates to the appropriate module.
//! Thin dispatch layer only — business logic lives in editor/, search/, browser/.

use std::path::PathBuf;
use tauri::{AppHandle, State};
use serde::{Deserialize, Serialize};

use crate::{
    AppState, Settings, SharedState,
    editor::{tokenizer, autocomplete},
    search::{indexer, metasearch},
    browser::{self, userscripts, privacy, adblock, history},
};

// ═══════════════════════════════════════════════════════════════════════════
//  EDITOR COMMANDS
// ═══════════════════════════════════════════════════════════════════════════

/// Lint HTML with the Rust stack-based tokenizer.
#[tauri::command]
pub async fn lint_html(content: String) -> Result<Vec<tokenizer::LintWarning>, String> {
    tokio::task::spawn_blocking(move || tokenizer::lint(&content))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Get context-aware autocomplete suggestions.
#[tauri::command]
pub async fn get_completions(
    content: String,
    line: usize,
) -> Result<Vec<autocomplete::Completion>, String> {
    tokio::task::spawn_blocking(move || autocomplete::completions_for(&content, line))
        .await
        .map_err(|e| e.to_string())?
        .map_err(|e| e.to_string())
}

/// Open a file via system dialog and return its path + content.
#[tauri::command]
pub async fn open_file(
    app: AppHandle,
    state: State<'_, SharedState>,
) -> Result<Option<OpenFileResult>, String> {
    use tauri_plugin_dialog::DialogExt;
    use tokio::fs;

    let path = app
        .dialog()
        .file()
        .add_filter("Web files", &["html", "htm", "css", "js", "ts", "json", "md", "txt"])
        .blocking_pick_file();

    let Some(path) = path else { return Ok(None); };
    let path_buf = path.into_path().map_err(|e| e.to_string())?;
    let content  = fs::read_to_string(&path_buf).await.map_err(|e| e.to_string())?;
    let path_str = path_buf.to_string_lossy().to_string();

    // Update recent files
    {
        let mut s = state.lock();
        s.recent_files.retain(|p| p != &path_buf);
        s.recent_files.insert(0, path_buf.clone());
        s.recent_files.truncate(10);
    }

    Ok(Some(OpenFileResult { path: path_str, content }))
}

#[derive(Serialize)]
pub struct OpenFileResult {
    pub path:    String,
    pub content: String,
}

/// Save content to a known path.
#[tauri::command]
pub async fn save_file(path: String, content: String) -> Result<(), String> {
    tokio::fs::write(&path, content.as_bytes())
        .await
        .map_err(|e| e.to_string())
}

/// Save As — shows a dialog and returns the chosen path.
#[tauri::command]
pub async fn save_file_as(
    app: AppHandle,
    content: String,
) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;

    let path = app
        .dialog()
        .file()
        .add_filter("HTML", &["html", "htm"])
        .add_filter("CSS",  &["css"])
        .add_filter("JavaScript", &["js"])
        .add_filter("All Files", &["*"])
        .blocking_save_file();

    let Some(path) = path else { return Ok(None); };
    let path_buf = path.into_path().map_err(|e| e.to_string())?;
    tokio::fs::write(&path_buf, content.as_bytes())
        .await
        .map_err(|e| e.to_string())?;

    Ok(Some(path_buf.to_string_lossy().to_string()))
}

/// Return list of recently opened file paths.
#[tauri::command]
pub fn get_recent_files(state: State<'_, SharedState>) -> Vec<String> {
    state.lock()
        .recent_files
        .iter()
        .map(|p| p.to_string_lossy().to_string())
        .collect()
}

// ═══════════════════════════════════════════════════════════════════════════
//  SEARCH COMMANDS
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Serialize)]
pub struct IndexStats {
    pub indexed: usize,
    pub skipped: usize,
}

/// Walk a directory and index all text files into SQLite FTS5.
#[tauri::command]
pub async fn index_directory(dir_path: String) -> Result<IndexStats, String> {
    let path = PathBuf::from(&dir_path);
    tokio::task::spawn_blocking(move || {
        let idx = indexer::SearchIndexer::open()?;
        idx.index_directory(&path)
    })
    .await
    .map_err(|e| e.to_string())?
    .map(|s| IndexStats { indexed: s.indexed, skipped: s.skipped })
    .map_err(|e| e.to_string())
}

/// Run a local FTS5 full-text search.
#[tauri::command]
pub async fn local_search(query: String) -> Result<Vec<indexer::SearchResult>, String> {
    tokio::task::spawn_blocking(move || {
        let idx = indexer::SearchIndexer::open()?;
        idx.search(&query)
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())
}

/// Privacy metasearch — queries DuckDuckGo, strips all tracking.
#[tauri::command]
pub async fn meta_search(query: String) -> Result<Vec<metasearch::WebResult>, String> {
    metasearch::search(&query)
        .await
        .map_err(|e| e.to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
//  BROWSER COMMANDS
// ═══════════════════════════════════════════════════════════════════════════

/// Open or navigate the privacy browser WebviewWindow.
#[tauri::command]
pub async fn open_browser_window(
    app: AppHandle,
    url: String,
    state: State<'_, SharedState>,
) -> Result<(), String> {
    browser::open_or_navigate(&app, &url, state.inner().clone())
        .await
        .map_err(|e| e.to_string())
}

/// Navigate the open browser to a new URL (privacy-scrubbed).
#[tauri::command]
pub fn browser_navigate(
    url: String,
    state: State<'_, SharedState>,
) -> Result<String, String> {
    let scrubbed = privacy::scrub_url(&url, &state.lock().settings);
    Ok(scrubbed)
}

/// Return cumulative privacy statistics.
#[tauri::command]
pub fn get_privacy_stats(state: State<'_, SharedState>) -> privacy::PrivacyStats {
    state.lock().privacy_stats.clone()
}

// ═══════════════════════════════════════════════════════════════════════════
//  USERSCRIPT COMMANDS
// ═══════════════════════════════════════════════════════════════════════════

/// List all installed userscripts.
#[tauri::command]
pub fn list_userscripts() -> Result<Vec<userscripts::ScriptMeta>, String> {
    userscripts::UserscriptManager::new()
        .and_then(|m| m.list())
        .map_err(|e| e.to_string())
}

/// Install a userscript from raw source (Tampermonkey format).
#[tauri::command]
pub fn install_userscript(source: String) -> Result<userscripts::ScriptMeta, String> {
    userscripts::UserscriptManager::new()
        .and_then(|mut m| m.install(&source))
        .map_err(|e| e.to_string())
}

/// Remove a userscript by ID.
#[tauri::command]
pub fn remove_userscript(id: String) -> Result<(), String> {
    userscripts::UserscriptManager::new()
        .and_then(|mut m| m.remove(&id))
        .map_err(|e| e.to_string())
}

/// Enable or disable a userscript.
#[tauri::command]
pub fn toggle_userscript(id: String, enabled: bool) -> Result<(), String> {
    userscripts::UserscriptManager::new()
        .and_then(|mut m| m.set_enabled(&id, enabled))
        .map_err(|e| e.to_string())
}

/// Return scripts whose @match patterns match a given URL.
#[tauri::command]
pub fn get_userscripts_for_url(url: String) -> Result<Vec<userscripts::ScriptPayload>, String> {
    userscripts::UserscriptManager::new()
        .and_then(|m| m.scripts_for_url(&url))
        .map_err(|e| e.to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
//  GM_ API COMMANDS (called from inside the browser webview)
// ═══════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn gm_get_value(script_id: String, key: String, default_val: serde_json::Value)
    -> Result<serde_json::Value, String>
{
    userscripts::GmStorage::new()
        .and_then(|s| s.get(&script_id, &key, default_val))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gm_set_value(script_id: String, key: String, value: serde_json::Value)
    -> Result<(), String>
{
    userscripts::GmStorage::new()
        .and_then(|s| s.set(&script_id, &key, value))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gm_delete_value(script_id: String, key: String) -> Result<(), String> {
    userscripts::GmStorage::new()
        .and_then(|s| s.delete(&script_id, &key))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gm_list_values(script_id: String) -> Result<Vec<String>, String> {
    userscripts::GmStorage::new()
        .and_then(|s| s.list_keys(&script_id))
        .map_err(|e| e.to_string())
}

#[derive(Deserialize)]
pub struct XhrDetails {
    pub method:      String,
    pub url:         String,
    pub headers:     std::collections::HashMap<String, String>,
    pub data:        Option<String>,
    pub response_type: Option<String>,
    pub timeout:     Option<u64>,
    pub anonymous:   Option<bool>,
}

#[derive(Serialize)]
pub struct XhrResponse {
    pub status:           u16,
    pub status_text:      String,
    pub response_text:    String,
    pub response_headers: String,
    pub final_url:        String,
}

/// CORS-bypassing HTTP request for GM_xmlhttpRequest.
/// Runs from Rust so the browser's CORS policy is irrelevant.
#[tauri::command]
pub async fn gm_xmlhttp_request(details: XhrDetails) -> Result<XhrResponse, String> {
    browser::make_gm_request(details).await.map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn gm_notification(title: String, text: String, _timeout: u32, app: AppHandle)
    -> Result<(), String>
{
    use tauri_plugin_notification::NotificationExt;
    app.notification()
        .builder()
        .title(&title)
        .body(&text)
        .show()
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn gm_open_in_tab(url: String, _background: Option<bool>, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_shell::ShellExt;
    app.shell().open(&url, None).map_err(|e| e.to_string())
}

#[tauri::command]
pub async fn gm_set_clipboard(data: String, _info: Option<String>, app: AppHandle) -> Result<(), String> {
    use tauri_plugin_clipboard_manager::ClipboardExt;
    app.clipboard().write_text(data).map_err(|e| e.to_string())
}

#[tauri::command]
pub fn gm_log(script_id: String, message: String) {
    log::info!("[GM:{}] {}", script_id, message);
}

// ═══════════════════════════════════════════════════════════════════════════
//  SETTINGS COMMANDS
// ═══════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_settings(state: State<'_, SharedState>) -> Settings {
    state.lock().settings.clone()
}

#[tauri::command]
pub fn save_settings(settings: Settings, state: State<'_, SharedState>) -> Result<(), String> {
    let settings_path = dirs::data_dir()
        .map(|d| d.join("bm-aegis").join("settings.json"))
        .ok_or("Cannot determine settings path")?;
    let json = serde_json::to_string_pretty(&settings).map_err(|e| e.to_string())?;
    std::fs::write(&settings_path, json).map_err(|e| e.to_string())?;
    state.lock().settings = settings;
    Ok(())
}

#[tauri::command]
pub async fn choose_directory(app: AppHandle) -> Result<Option<String>, String> {
    use tauri_plugin_dialog::DialogExt;
    let path = app.dialog().file().blocking_pick_folder();
    Ok(path.and_then(|p| p.into_path().ok())
           .map(|p| p.to_string_lossy().to_string()))
}



// ═══════════════════════════════════════════════════════════════════════════
//  ADBLOCK STATS
// ═══════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn get_adblock_stats() -> browser::adblock::AdBlockStats {
    browser::adblock_stats()
}

// ═══════════════════════════════════════════════════════════════════════════
//  HISTORY COMMANDS
// ═══════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn history_add(url: String, title: String) -> Result<(), String> {
    browser::history::HistoryStore::open()
        .and_then(|s| s.add_visit(&url, &title))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn history_recent(limit: Option<usize>) -> Result<Vec<browser::history::HistoryEntry>, String> {
    browser::history::HistoryStore::open()
        .and_then(|s| s.recent(limit.unwrap_or(100)))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn history_search(query: String) -> Result<Vec<browser::history::HistoryEntry>, String> {
    browser::history::HistoryStore::open()
        .and_then(|s| s.search(&query))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn history_delete(id: i64) -> Result<(), String> {
    browser::history::HistoryStore::open()
        .and_then(|s| s.delete(id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn history_clear() -> Result<usize, String> {
    browser::history::HistoryStore::open()
        .and_then(|s| s.clear_all())
        .map_err(|e| e.to_string())
}

// ═══════════════════════════════════════════════════════════════════════════
//  BOOKMARK COMMANDS
// ═══════════════════════════════════════════════════════════════════════════

#[tauri::command]
pub fn bookmark_add(url: String, title: String, folder: String)
    -> Result<browser::bookmarks::Bookmark, String>
{
    browser::bookmarks::BookmarkStore::open()
        .and_then(|s| s.add(&url, &title, &folder))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn bookmark_remove(id: i64) -> Result<(), String> {
    browser::bookmarks::BookmarkStore::open()
        .and_then(|s| s.remove(id))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn bookmark_is_bookmarked(url: String) -> Result<bool, String> {
    browser::bookmarks::BookmarkStore::open()
        .and_then(|s| s.is_bookmarked(&url))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn bookmark_list() -> Result<Vec<browser::bookmarks::Bookmark>, String> {
    browser::bookmarks::BookmarkStore::open()
        .and_then(|s| s.list_all())
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn bookmark_search(query: String) -> Result<Vec<browser::bookmarks::Bookmark>, String> {
    browser::bookmarks::BookmarkStore::open()
        .and_then(|s| s.search(&query))
        .map_err(|e| e.to_string())
}

#[tauri::command]
pub fn bookmark_folders() -> Result<Vec<String>, String> {
    browser::bookmarks::BookmarkStore::open()
        .and_then(|s| s.folders())
        .map_err(|e| e.to_string())
}
