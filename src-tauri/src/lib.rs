pub mod commands;
pub mod editor;
pub mod search;
pub mod browser;

use parking_lot::Mutex;
use std::sync::Arc;
use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct Settings {
    pub editor_font_size:          u8,
    pub tab_size:                  u8,
    pub auto_save:                 bool,
    pub lint_debounce:             u32,
    pub block_trackers:            bool,
    pub strip_params:              bool,
    pub block_third_party_cookies: bool,
    pub search_engine:             String,
    pub max_index_mb:              u32,
    pub index_hidden:              bool,
    pub restore_last_file:         bool,
    pub custom_titlebar:           bool,
    // New v0.2
    pub enable_adblock:            bool,
    pub enable_fingerprint_protection: bool,
    pub upgrade_https:             bool,
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            editor_font_size:              13,
            tab_size:                       2,
            auto_save:                  false,
            lint_debounce:                600,
            block_trackers:              true,
            strip_params:                true,
            block_third_party_cookies:  false,
            search_engine: "https://duckduckgo.com/?q={query}".into(),
            max_index_mb:                100,
            index_hidden:               false,
            restore_last_file:          false,
            custom_titlebar:             true,
            enable_adblock:              true,
            enable_fingerprint_protection: true,
            upgrade_https:               true,
        }
    }
}

#[derive(Debug, Default)]
pub struct AppState {
    pub settings:      Settings,
    pub recent_files:  Vec<std::path::PathBuf>,
    pub privacy_stats: browser::privacy::PrivacyStats,
    pub browser_open:  bool,
}

pub type SharedState = Arc<Mutex<AppState>>;

pub fn run() {
    let state: SharedState = Arc::new(Mutex::new(AppState::default()));

    // Load persisted settings on startup
    if let Some(path) = settings_path() {
        if let Ok(text) = std::fs::read_to_string(&path) {
            if let Ok(s) = serde_json::from_str::<Settings>(&text) {
                state.lock().settings = s;
            }
        }
    }

    tauri::Builder::default()
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_fs::init())
        .plugin(tauri_plugin_shell::init())
        .plugin(tauri_plugin_notification::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .manage(state)
        .invoke_handler(tauri::generate_handler![
            // Editor
            commands::lint_html,
            commands::get_completions,
            commands::open_file,
            commands::save_file,
            commands::save_file_as,
            commands::get_recent_files,
            // Search
            commands::index_directory,
            commands::local_search,
            commands::meta_search,
            // Browser
            commands::open_browser_window,
            commands::browser_navigate,
            commands::get_privacy_stats,
            commands::get_adblock_stats,
            // Userscripts
            commands::list_userscripts,
            commands::install_userscript,
            commands::remove_userscript,
            commands::toggle_userscript,
            commands::get_userscripts_for_url,
            // GM_ API
            commands::gm_get_value,
            commands::gm_set_value,
            commands::gm_delete_value,
            commands::gm_list_values,
            commands::gm_xmlhttp_request,
            commands::gm_notification,
            commands::gm_open_in_tab,
            commands::gm_set_clipboard,
            commands::gm_log,
            // History
            commands::history_add,
            commands::history_recent,
            commands::history_search,
            commands::history_delete,
            commands::history_clear,
            // Bookmarks
            commands::bookmark_add,
            commands::bookmark_remove,
            commands::bookmark_is_bookmarked,
            commands::bookmark_list,
            commands::bookmark_search,
            commands::bookmark_folders,
            // Settings
            commands::get_settings,
            commands::save_settings,
            commands::choose_directory,
        ])
        .run(tauri::generate_context!())
        .expect("error running BM-Aegis");
}

fn settings_path() -> Option<std::path::PathBuf> {
    dirs::data_dir().map(|d| d.join("bm-aegis").join("settings.json"))
}
