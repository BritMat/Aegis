/**
 * settings.js — Settings panel, updated with v2 protection options.
 */

import { getSettings, saveSettings } from "../lib/tauri-bridge.js";
import { toastSuccess, toastError }  from "../lib/toast.js";

let _settings = {};

export async function mountSettings(sidebarEl, panelEl) {
  sidebarEl.innerHTML = `
    <div class="sidebar-section">
      <div class="sidebar-heading">Settings</div>
      <div class="sidebar-item active">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <circle cx="6.5" cy="6.5" r="1.8" stroke="currentColor" stroke-width="1.2"/>
          <path d="M6.5 1v1.4M6.5 10.6V12M1 6.5h1.4M10.6 6.5H12M2.5 2.5l1 1M9.5 9.5l1 1M9.5 2.5l-1 1M3.5 9.5l-1 1"
                stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
        </svg>
        General
      </div>
    </div>`;

  try { _settings = await getSettings() ?? {}; } catch { _settings = {}; }
  buildPanel(panelEl);
}

// ─────────────────────────────────────────────────────────────────────────
function tog(id, key, defaultOn = false) {
  const checked = key in _settings ? _settings[key] : defaultOn;
  return `<label class="toggle-wrap">
    <input type="checkbox" id="${id}"${checked ? " checked" : ""}>
    <span class="toggle-slider"></span>
  </label>`;
}
function num(id, key, def, min, max, w = 76) {
  return `<input class="settings-input" type="number" id="${id}"
           min="${min}" max="${max}" value="${_settings[key] ?? def}" style="width:${w}px">`;
}
function txt(id, key, def, w = 230) {
  return `<input class="settings-input" type="text" id="${id}"
           value="${esc(_settings[key] ?? def)}" style="width:${w}px">`;
}

function section(title, svgPath, rows) {
  return `<div class="settings-section">
    <div class="settings-section-header">
      <svg width="13" height="13" viewBox="0 0 13 13" fill="none">${svgPath}</svg>
      ${title}
    </div>
    ${rows}
  </div>`;
}
function row(label, control) {
  return `<div class="settings-row"><span class="settings-label">${label}</span>${control}</div>`;
}

function buildPanel(el) {
  el.innerHTML = `<div id="settings-panel">

  ${section("Editor",
    `<rect x="1" y="1" width="11" height="11" rx="1" stroke="currentColor" stroke-width="1.2"/>
     <path d="M3.5 5L5.5 7L3.5 9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
     <path d="M7 9h2.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>`,
    row("Font size (px)",           num("s-fontsize",  "editorFontSize", 13, 10, 24))   +
    row("Tab size (spaces)",        num("s-tabsize",   "tabSize",         2,  2,  8))    +
    row("Auto-save on tab switch",  tog("s-autosave",  "autoSave"))                       +
    row("Lint debounce (ms)",       num("s-debounce",  "lintDebounce",  600,200,2000,90)) +
    row("Word wrap by default",     tog("s-wordwrap",  "wordWrap"))
  )}

  ${section("Privacy Browser",
    `<circle cx="6.5" cy="6.5" r="5.5" stroke="currentColor" stroke-width="1.2"/>
     <path d="M6.5 1c0 0-2 2-2 5.5S6.5 12 6.5 12" stroke="currentColor" stroke-width=".9"/>
     <path d="M1 6.5h11" stroke="currentColor" stroke-width=".9"/>`,
    row("Block tracker domains",         tog("s-blocktrackers", "blockTrackers",   true))  +
    row("Strip tracking URL parameters", tog("s-stripparams",   "stripParams",     true))  +
    row("Ad block engine (EasyList)",    tog("s-adblock",       "enableAdblock",   true))  +
    row("Fingerprint protection",        tog("s-fingerprint",   "enableFingerprintProtection", true)) +
    row("Upgrade HTTP → HTTPS",          tog("s-https",         "upgradeHttps",    true))  +
    row("Block third-party cookies",     tog("s-block3p",       "blockThirdPartyCookies"))  +
    row("Default search engine",         txt("s-searchengine",  "searchEngine", "https://duckduckgo.com/?q={query}"))
  )}

  ${section("Search Index",
    `<circle cx="6" cy="6" r="4.5" stroke="currentColor" stroke-width="1.2"/>
     <path d="M9.5 9.5L12 12" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>`,
    row("Max index size (MB)",      num("s-maxindex",   "maxIndexMb",  100, 10, 2000, 90)) +
    row("Index hidden files",       tog("s-indexhidden","indexHidden"))                     +
    row("Reopen last file on start",tog("s-restorefile","restoreLastFile"))
  )}

  <div class="settings-save-row">
    <button class="settings-save-btn" id="s-save">Save Settings</button>
    <span id="settings-save-status"></span>
  </div>
  </div>`;

  el.querySelector("#s-save").addEventListener("click", handleSave);
}

async function handleSave() {
  const g = id => document.getElementById(id);
  const updated = {
    editorFontSize:               +(g("s-fontsize")?.value   ?? 13),
    tabSize:                      +(g("s-tabsize")?.value    ??  2),
    autoSave:                       g("s-autosave")?.checked  ?? false,
    lintDebounce:                 +(g("s-debounce")?.value   ?? 600),
    wordWrap:                       g("s-wordwrap")?.checked  ?? false,
    blockTrackers:                  g("s-blocktrackers")?.checked ?? true,
    stripParams:                    g("s-stripparams")?.checked   ?? true,
    enableAdblock:                  g("s-adblock")?.checked       ?? true,
    enableFingerprintProtection:    g("s-fingerprint")?.checked   ?? true,
    upgradeHttps:                   g("s-https")?.checked         ?? true,
    blockThirdPartyCookies:         g("s-block3p")?.checked       ?? false,
    searchEngine:                   g("s-searchengine")?.value    ?? "",
    maxIndexMb:                   +(g("s-maxindex")?.value   ?? 100),
    indexHidden:                    g("s-indexhidden")?.checked   ?? false,
    restoreLastFile:                g("s-restorefile")?.checked   ?? false,
    customTitlebar: true,
  };
  try {
    await saveSettings(updated);
    _settings = updated;
    toastSuccess("Settings saved");
  } catch (e) { toastError("Save failed: " + e); }
}

const esc = s => String(s)
  .replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;").replace(/"/g,"&quot;");
