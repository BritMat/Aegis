/**
 * tauri-bridge.js
 * All Tauri invoke() wrappers. Import from here — never call invoke() directly.
 * Uses @tauri-apps/api/core (Tauri v2 ESM import).
 */

import { invoke as _invoke } from "@tauri-apps/api/core";

// Graceful fallback when running outside Tauri (e.g. `npm run dev` in browser)
const invoke = typeof window.__TAURI_INTERNALS__ !== "undefined"
  ? _invoke
  : async (cmd, args) => {
      console.warn(`[dev-shim] invoke("${cmd}", ${JSON.stringify(args)})`);
      return null;
    };

/* ── Editor ───────────────────────────────────────────────────── */
export const lintHtml         = (content)          => invoke("lint_html",           { content });
export const getCompletions   = (content, line)    => invoke("get_completions",     { content, line });
export const openFile         = ()                 => invoke("open_file");
export const saveFile         = (path, content)    => invoke("save_file",           { path, content });
export const saveFileAs       = (content)          => invoke("save_file_as",        { content });
export const getRecentFiles   = ()                 => invoke("get_recent_files");

/* ── Search ───────────────────────────────────────────────────── */
export const indexDirectory   = (dirPath)          => invoke("index_directory",     { dirPath });
export const localSearch      = (query)            => invoke("local_search",        { query });
export const metaSearch       = (query)            => invoke("meta_search",         { query });

/* ── Browser ──────────────────────────────────────────────────── */
export const openBrowserWindow = (url)             => invoke("open_browser_window", { url });
export const browserNavigate  = (url)              => invoke("browser_navigate",    { url });
export const getPrivacyStats  = ()                 => invoke("get_privacy_stats");

/* ── Userscripts ──────────────────────────────────────────────── */
export const listUserscripts    = ()               => invoke("list_userscripts");
export const installUserscript  = (source)         => invoke("install_userscript",  { source });
export const removeUserscript   = (id)             => invoke("remove_userscript",   { id });
export const toggleUserscript   = (id, enabled)    => invoke("toggle_userscript",   { id, enabled });
export const getUserscriptsForUrl = (url)          => invoke("get_userscripts_for_url", { url });

/* ── Settings ─────────────────────────────────────────────────── */
export const getSettings      = ()                 => invoke("get_settings");
export const saveSettings     = (settings)         => invoke("save_settings",       { settings });
export const chooseDirectory  = ()                 => invoke("choose_directory");

/* ── History ──────────────────────────────────────────────────── */
export const historyAdd     = (url, title)  => invoke("history_add",    { url, title });
export const historyRecent  = (limit)       => invoke("history_recent",  { limit });
export const historySearch  = (query)       => invoke("history_search",  { query });
export const historyDelete  = (id)          => invoke("history_delete",  { id });
export const historyClear   = ()            => invoke("history_clear");

/* ── Adblock ──────────────────────────────────────────────────── */
export const getAdblockStats = ()           => invoke("get_adblock_stats");

/* ── Bookmarks ────────────────────────────────────────────────── */
export const bookmarkAdd          = (url, title, folder) => invoke("bookmark_add",           { url, title, folder: folder || "Unsorted" });
export const bookmarkRemove       = (id)                  => invoke("bookmark_remove",        { id });
export const bookmarkIsBookmarked = (url)                 => invoke("bookmark_is_bookmarked", { url });
export const bookmarkList         = ()                    => invoke("bookmark_list");
export const bookmarkSearch       = (query)               => invoke("bookmark_search",        { query });
export const bookmarkFolders      = ()                    => invoke("bookmark_folders");
