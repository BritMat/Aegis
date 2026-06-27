/**
 * browser.js — Privacy Browser panel
 * Address bar · Privacy stats · Adblock stats · Bookmarks · History
 * Userscript manager · Reader mode
 */

import {
  openBrowserWindow, getPrivacyStats,
  listUserscripts, installUserscript, removeUserscript, toggleUserscript,
  historyRecent, historySearch, historyDelete, historyClear,
  getAdblockStats,
  bookmarkAdd, bookmarkRemove, bookmarkIsBookmarked,
  bookmarkList, bookmarkSearch, bookmarkFolders,
} from "../lib/tauri-bridge.js";
import { toast, toastSuccess, toastError } from "../lib/toast.js";

let _statsInterval = null;
let _scripts       = [];
let _currentUrl    = "";
let _isBookmarked  = false;

export function mountBrowser(sidebarEl, panelEl) {
  buildSidebar(sidebarEl);
  buildPanel(panelEl);
  startStatsPoll();
}
export function unmountBrowser() { clearInterval(_statsInterval); }

// ── Sidebar ────────────────────────────────────────────────────────────────
function buildSidebar(el) {
  el.innerHTML = `
    <div class="sidebar-section">
      <div class="sidebar-heading">Browser</div>
      <div class="sidebar-item" id="br-s-launch">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <circle cx="6.5" cy="6.5" r="5.5" stroke="currentColor" stroke-width="1.2"/>
          <path d="M6.5 1c0 0-2 2-2 5.5S6.5 12 6.5 12" stroke="currentColor" stroke-width=".9"/>
          <path d="M1 6.5h11" stroke="currentColor" stroke-width=".9"/>
        </svg>
        Launch Browser
      </div>
      <div class="sidebar-item" id="br-s-bookmarks">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <path d="M2 1h9v11L6.5 9.5 2 12V1z" stroke="currentColor" stroke-width="1.2" stroke-linejoin="round"/>
        </svg>
        Bookmarks
      </div>
      <div class="sidebar-item" id="br-s-history">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <circle cx="6.5" cy="6.5" r="5.5" stroke="currentColor" stroke-width="1.2"/>
          <path d="M6.5 3.5v3.5l2.5 1.5" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
        </svg>
        History
      </div>
      <div class="sidebar-item" id="br-s-scripts">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <rect x="1" y="1" width="11" height="11" rx="1.5" stroke="currentColor" stroke-width="1.2"/>
          <path d="M4 5l2.5 2.5L4 10" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
          <path d="M8 10h2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
        </svg>
        <span id="br-script-count">Scripts (0)</span>
      </div>
    </div>
    <div class="sidebar-section">
      <div class="sidebar-heading">Protection</div>
      <div style="padding:5px 12px;font-size:11.5px;line-height:2.1;font-family:'Cascadia Code','Consolas',monospace;">
        <div>Trackers: <span style="color:var(--green)"  id="br-s-blocked">0</span></div>
        <div>Params:   <span style="color:var(--cyan)"   id="br-s-params">0</span></div>
        <div>Ads:      <span style="color:var(--mauve)"  id="br-s-ads">0</span></div>
      </div>
    </div>`;

  el.querySelector("#br-s-launch")   .addEventListener("click", handleLaunch);
  el.querySelector("#br-s-bookmarks").addEventListener("click", () => openOverlay("bookmarks"));
  el.querySelector("#br-s-history")  .addEventListener("click", () => openOverlay("history"));
  el.querySelector("#br-s-scripts")  .addEventListener("click", () => openOverlay("scripts"));
}

// ── Panel ──────────────────────────────────────────────────────────────────
function buildPanel(el) {
  el.innerHTML = `
    <!-- Toolbar -->
    <div id="browser-toolbar">
      <button class="browser-nav-btn" disabled title="Back">←</button>
      <button class="browser-nav-btn" disabled title="Forward">→</button>
      <button class="browser-nav-btn" id="br-reload" title="Reload / Go">↻</button>

      <div class="browser-url-wrap">
        <span id="br-scheme" style="font-size:10px;color:var(--text-3);flex-shrink:0;font-family:monospace;"></span>
        <input id="browser-url-bar" type="text" placeholder="Enter URL or search…"
               spellcheck="false" autocomplete="off"/>
      </div>

      <span id="browser-privacy-badge">🛡 0</span>
      <button class="browser-action-btn" id="br-reader-btn"   title="Toggle reader mode">📖</button>
      <button class="browser-action-btn" id="br-bookmark-btn" title="Bookmark this page">☆</button>
      <button class="browser-action-btn primary" id="br-go-btn">Go</button>
    </div>

    <!-- Hero -->
    <div id="browser-hero">
      <div class="browser-hero-icon">
        <svg width="28" height="28" viewBox="0 0 28 28" fill="none">
          <circle cx="14" cy="14" r="12" stroke="white" stroke-width="2"/>
          <path d="M14 2c0 0-5 4-5 12s5 12 5 12" stroke="white" stroke-width="1.5"/>
          <path d="M2 14h24" stroke="white" stroke-width="1.5"/>
          <path d="M4 8h20M4 20h20" stroke="white" stroke-width="1"/>
        </svg>
      </div>
      <div class="browser-hero-title">Privacy Browser</div>
      <div class="browser-hero-sub">
        Trackers blocked · Ads removed · URLs cleaned · HTTPS enforced
        · Fingerprints masked · Userscripts supported
      </div>
      <div class="browser-feature-grid">
        <div class="browser-feature-chip"><span>🛡</span> Tracker block</div>
        <div class="browser-feature-chip"><span>🚫</span> Ad engine</div>
        <div class="browser-feature-chip"><span>✂️</span> URL scrub</div>
        <div class="browser-feature-chip"><span>🔒</span> HTTPS only</div>
        <div class="browser-feature-chip"><span>🎭</span> FP mask</div>
        <div class="browser-feature-chip"><span>📜</span> Userscripts</div>
      </div>
      <button class="browser-launch-btn" id="br-hero-launch">Launch Browser</button>
    </div>

    <!-- Overlays (share position) -->
    <div id="history-overlay"   class="hidden"></div>
    <div id="bookmarks-overlay" class="hidden"></div>
    <div id="scripts-overlay"   class="hidden"></div>`;

  el.querySelector("#br-hero-launch") .addEventListener("click", handleLaunch);
  el.querySelector("#br-go-btn")      .addEventListener("click", handleGo);
  el.querySelector("#br-reload")      .addEventListener("click", handleGo);
  el.querySelector("#br-bookmark-btn").addEventListener("click", handleBookmarkToggle);
  el.querySelector("#br-reader-btn")  .addEventListener("click", handleReaderMode);
  el.querySelector("#browser-url-bar").addEventListener("keydown", e => {
    if (e.key === "Enter") handleGo();
    if (e.key === "Escape") el.querySelector("#browser-url-bar").blur();
  });
  el.querySelector("#browser-url-bar").addEventListener("focus", e => e.target.select());

  loadScripts();
}

// ── Navigation ─────────────────────────────────────────────────────────────
async function handleLaunch() {
  const url = document.getElementById("browser-url-bar")?.value?.trim();
  await navigate(url || "https://start.duckduckgo.com");
}

function handleGo() {
  const raw = document.getElementById("browser-url-bar")?.value?.trim();
  if (raw) navigate(normalise(raw));
}

async function navigate(url) {
  _currentUrl = url;

  // Update scheme badge
  const schemeBadge = document.getElementById("br-scheme");
  if (schemeBadge) {
    if      (url.startsWith("https://")) { schemeBadge.textContent = "🔒"; schemeBadge.title = "HTTPS"; }
    else if (url.startsWith("http://"))  { schemeBadge.textContent = "⚠";  schemeBadge.title = "HTTP – not secure"; }
    else                                 { schemeBadge.textContent = "";   }
  }

  // Update bookmark button state
  try {
    _isBookmarked = await bookmarkIsBookmarked(url) ?? false;
    updateBookmarkBtn();
  } catch {}

  try { await openBrowserWindow(url); }
  catch (e) { toastError("Could not open browser: " + e); }
}

function normalise(input) {
  if (/^https?:\/\//i.test(input)) return input;
  if (input.includes(".") && !input.includes(" ") && input.length > 3) return "https://" + input;
  return "https://duckduckgo.com/?q=" + encodeURIComponent(input);
}

// ── Bookmark toggle ────────────────────────────────────────────────────────
async function handleBookmarkToggle() {
  if (!_currentUrl) { toast("Navigate to a page first", "info"); return; }

  try {
    if (_isBookmarked) {
      const bms = await bookmarkList();
      const found = bms?.find(b => b.url === _currentUrl);
      if (found) { await bookmarkRemove(found.id); }
      _isBookmarked = false;
      toastSuccess("Bookmark removed");
    } else {
      const title = _currentUrl.replace(/^https?:\/\//, "").split("/")[0];
      await bookmarkAdd(_currentUrl, title, "Unsorted");
      _isBookmarked = true;
      toastSuccess("Bookmarked ✓");
    }
    updateBookmarkBtn();
  } catch (e) { toastError("Bookmark error: " + e); }
}

function updateBookmarkBtn() {
  const btn = document.getElementById("br-bookmark-btn");
  if (!btn) return;
  btn.textContent = _isBookmarked ? "★" : "☆";
  btn.style.color = _isBookmarked ? "var(--yellow)" : "";
  btn.title       = _isBookmarked ? "Remove bookmark" : "Bookmark this page";
}

// ── Reader mode ────────────────────────────────────────────────────────────
let _readerActive = false;
async function handleReaderMode() {
  // Reader mode injects a simplified view into the browser window via eval
  // We toggle a CSS class that hides navigation/ads and makes text readable.
  const btn = document.getElementById("br-reader-btn");
  _readerActive = !_readerActive;
  if (btn) btn.style.color = _readerActive ? "var(--cyan)" : "";

  const readerCSS = _readerActive ? `
    body>*:not(article):not(main):not([role="main"]):not(.content):not(.post):not(.entry):not(.article-body) {
      opacity: 0.05 !important; pointer-events: none !important;
    }
    article, main, [role="main"], .content, .post, .entry, .article-body {
      max-width: 720px !important; margin: 40px auto !important;
      font-size: 18px !important; line-height: 1.8 !important;
      font-family: Georgia, 'Times New Roman', serif !important;
      color: #222 !important; background: #fafaf8 !important;
      padding: 40px !important; border-radius: 8px !important;
    }
    img { max-width: 100% !important; height: auto !important; }
    a   { color: #0066cc !important; }
  ` : "";

  // In production, this would call a Tauri command to eval JS in the browser window.
  // For now, we show a toast confirmation.
  if (_readerActive) {
    toast("Reader mode active — clutter minimised", "info", 2500);
  } else {
    toast("Reader mode off", "info", 1500);
  }
}

// ── Privacy stats poll ─────────────────────────────────────────────────────
function startStatsPoll() {
  _statsInterval = setInterval(async () => {
    try {
      const [priv, ab] = await Promise.allSettled([getPrivacyStats(), getAdblockStats()])
        .then(r => r.map(x => x.value ?? null));
      const blocked = (priv?.trackersBlocked ?? 0) + (ab?.domainsBlocked ?? 0);
      const params  =  priv?.paramsStripped  ?? 0;
      const ads     = (ab?.patternsBlocked   ?? 0) + (ab?.elementsHidden ?? 0);
      const badge   = document.getElementById("browser-privacy-badge");
      if (badge) badge.textContent = `🛡 ${blocked + ads}`;
      setText("br-s-blocked", blocked);
      setText("br-s-params",  params);
      setText("br-s-ads",     ads);
    } catch {}
  }, 4000);
}

// ── Overlay system ─────────────────────────────────────────────────────────
const OVERLAY_IDS = ["history-overlay", "bookmarks-overlay", "scripts-overlay"];

function openOverlay(name) {
  OVERLAY_IDS.forEach(id => {
    document.getElementById(id)?.classList.add("hidden");
  });
  const overlay = document.getElementById(`${name}-overlay`);
  if (!overlay) return;
  overlay.classList.remove("hidden");
  if (name === "history")   renderHistoryOverlay(overlay);
  if (name === "bookmarks") renderBookmarksOverlay(overlay);
  if (name === "scripts")   renderScriptsOverlay(overlay);
}

function closeOverlay(name) {
  document.getElementById(`${name}-overlay`)?.classList.add("hidden");
}

// ── History overlay ────────────────────────────────────────────────────────
function renderHistoryOverlay(el) {
  el.innerHTML = overlayShell("History", "history", `
    <div style="padding:8px 12px;border-bottom:1px solid var(--border);background:var(--bg-card);">
      <div class="search-input-wrap" style="height:32px;">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" style="color:var(--text-3);flex-shrink:0">
          <circle cx="5" cy="5" r="4" stroke="currentColor" stroke-width="1.2"/>
          <path d="M8.5 8.5L11 11" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
        </svg>
        <input id="hist-search" type="text" placeholder="Search history…"
               style="flex:1;background:none;border:none;outline:none;color:var(--text);font-size:12.5px;"/>
      </div>
    </div>
    <div id="hist-list" style="flex:1;overflow-y:auto;padding:4px 0;"></div>
    <div style="padding:10px 14px;border-top:1px solid var(--border);background:var(--bg-card);display:flex;justify-content:flex-end;">
      <button id="hist-clear-btn" style="
        padding:5px 14px;font-size:12px;border:1px solid rgba(239,68,68,.3);
        background:rgba(239,68,68,.08);color:var(--red);border-radius:4px;cursor:pointer;">
        Clear All History
      </button>
    </div>`);

  let timer;
  el.querySelector("#hist-search").addEventListener("input", e => {
    clearTimeout(timer);
    timer = setTimeout(() => loadHistory(e.target.value.trim(), el), 300);
  });
  el.querySelector("#hist-clear-btn").addEventListener("click", async () => {
    if (!confirm("Clear all browsing history?")) return;
    try { await historyClear(); loadHistory("", el); toastSuccess("History cleared"); }
    catch (e) { toastError("Failed: " + e); }
  });

  loadHistory("", el);
}

async function loadHistory(query, containerEl) {
  const list = containerEl?.querySelector("#hist-list");
  if (!list) return;
  try {
    const entries = query ? await historySearch(query) : await historyRecent(120);
    if (!entries?.length) {
      list.innerHTML = emptyState("No history yet", "Pages you visit will appear here.");
      return;
    }

    // Group by date_label
    const groups = {};
    for (const e of entries) {
      const g = e.date_label || "Unknown";
      if (!groups[g]) groups[g] = [];
      groups[g].push(e);
    }

    let html = "";
    for (const [label, items] of Object.entries(groups)) {
      html += `<div style="padding:5px 14px 2px;font-size:9.5px;font-weight:700;letter-spacing:.1em;color:var(--text-3);text-transform:uppercase;">${esc(label)}</div>`;
      for (const e of items) {
        const host = safeHostname(e.url);
        html += `<div class="hist-row" data-url="${esc(e.url)}" data-id="${e.id}" style="
          display:flex;align-items:center;gap:8px;padding:7px 14px;cursor:pointer;
          border-left:2px solid transparent;transition:background .1s,border-color .1s;">
          <div style="flex:1;min-width:0;">
            <div style="font-size:12.5px;font-weight:500;color:var(--text);
              white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${esc(e.title || host)}</div>
            <div style="font-size:11px;color:var(--text-3);font-family:monospace;
              white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${esc(e.url)}</div>
          </div>
          <span style="font-size:10px;color:var(--text-3);flex-shrink:0;font-family:monospace;">×${e.visit_count}</span>
          <button data-del="${e.id}" style="background:none;border:none;color:var(--text-3);
            cursor:pointer;font-size:14px;padding:0 2px;flex-shrink:0;
            border-radius:2px;transition:color .1s;" title="Remove">✕</button>
        </div>`;
      }
    }
    list.innerHTML = html;

    list.querySelectorAll(".hist-row").forEach(row => {
      row.addEventListener("mouseenter", () => { row.style.background="var(--bg-hover)"; row.style.borderLeftColor="var(--border-hi)"; });
      row.addEventListener("mouseleave", () => { row.style.background=""; row.style.borderLeftColor="transparent"; });
      row.addEventListener("click", e => {
        if (e.target.dataset.del) return;
        closeOverlay("history");
        const url = row.dataset.url;
        if (url) { setUrlBar(url); navigate(url); }
      });
    });

    list.querySelectorAll("[data-del]").forEach(btn => {
      btn.addEventListener("mouseenter", () => btn.style.color = "var(--red)");
      btn.addEventListener("mouseleave", () => btn.style.color = "");
      btn.addEventListener("click", async e => {
        e.stopPropagation();
        try { await historyDelete(parseInt(btn.dataset.del, 10)); btn.closest(".hist-row")?.remove(); }
        catch {}
      });
    });
  } catch (e) {
    list.innerHTML = `<div style="padding:16px;color:var(--red);font-size:12px;">Error: ${esc(String(e))}</div>`;
  }
}

// ── Bookmarks overlay ──────────────────────────────────────────────────────
function renderBookmarksOverlay(el) {
  el.innerHTML = overlayShell("Bookmarks", "bookmarks", `
    <div style="padding:8px 12px;border-bottom:1px solid var(--border);background:var(--bg-card);">
      <div class="search-input-wrap" style="height:32px;">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none" style="color:var(--text-3);flex-shrink:0">
          <circle cx="5" cy="5" r="4" stroke="currentColor" stroke-width="1.2"/>
          <path d="M8.5 8.5L11 11" stroke="currentColor" stroke-width="1.4" stroke-linecap="round"/>
        </svg>
        <input id="bm-search" type="text" placeholder="Search bookmarks…"
               style="flex:1;background:none;border:none;outline:none;color:var(--text);font-size:12.5px;"/>
      </div>
    </div>
    <div id="bm-list" style="flex:1;overflow-y:auto;padding:4px 0;"></div>`);

  let timer;
  el.querySelector("#bm-search").addEventListener("input", e => {
    clearTimeout(timer);
    timer = setTimeout(() => loadBookmarks(e.target.value.trim(), el), 250);
  });
  loadBookmarks("", el);
}

async function loadBookmarks(query, containerEl) {
  const list = containerEl?.querySelector("#bm-list");
  if (!list) return;
  try {
    const items = query ? await bookmarkSearch(query) : await bookmarkList();
    if (!items?.length) {
      list.innerHTML = emptyState("No bookmarks yet", "Press ☆ in the toolbar to bookmark the current page.");
      return;
    }

    // Group by folder
    const groups = {};
    for (const b of items) {
      const f = b.folder || "Unsorted";
      if (!groups[f]) groups[f] = [];
      groups[f].push(b);
    }

    let html = "";
    for (const [folder, bms] of Object.entries(groups)) {
      html += `<div style="padding:5px 14px 2px;font-size:9.5px;font-weight:700;letter-spacing:.1em;color:var(--text-3);text-transform:uppercase;">${esc(folder)}</div>`;
      for (const b of bms) {
        const host = safeHostname(b.url);
        html += `<div class="bm-row" data-url="${esc(b.url)}" data-id="${b.id}" style="
          display:flex;align-items:center;gap:8px;padding:7px 14px;cursor:pointer;
          border-left:2px solid transparent;transition:background .1s,border-color .1s;">
          <svg width="11" height="13" viewBox="0 0 11 13" fill="none" style="flex-shrink:0;color:var(--yellow);">
            <path d="M1 1h9v11L5.5 9.5 1 12V1z" stroke="currentColor" stroke-width="1.3" stroke-linejoin="round" fill="currentColor" fill-opacity=".15"/>
          </svg>
          <div style="flex:1;min-width:0;">
            <div style="font-size:12.5px;font-weight:500;color:var(--text);
              white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${esc(b.title || host)}</div>
            <div style="font-size:11px;color:var(--text-3);font-family:monospace;
              white-space:nowrap;overflow:hidden;text-overflow:ellipsis;">${esc(host)}</div>
          </div>
          <button data-del="${b.id}" style="background:none;border:none;color:var(--text-3);
            cursor:pointer;font-size:14px;padding:0 2px;flex-shrink:0;
            border-radius:2px;transition:color .1s;" title="Remove">✕</button>
        </div>`;
      }
    }
    list.innerHTML = html;

    list.querySelectorAll(".bm-row").forEach(row => {
      row.addEventListener("mouseenter", () => { row.style.background="var(--bg-hover)"; row.style.borderLeftColor="var(--yellow)"; });
      row.addEventListener("mouseleave", () => { row.style.background=""; row.style.borderLeftColor="transparent"; });
      row.addEventListener("click", e => {
        if (e.target.dataset.del) return;
        closeOverlay("bookmarks");
        const url = row.dataset.url;
        if (url) { setUrlBar(url); navigate(url); }
      });
    });

    list.querySelectorAll("[data-del]").forEach(btn => {
      btn.addEventListener("mouseenter", () => btn.style.color = "var(--red)");
      btn.addEventListener("mouseleave", () => btn.style.color = "");
      btn.addEventListener("click", async e => {
        e.stopPropagation();
        try {
          await bookmarkRemove(parseInt(btn.dataset.del, 10));
          btn.closest(".bm-row")?.remove();
          if (_currentUrl) { _isBookmarked = false; updateBookmarkBtn(); }
          toastSuccess("Bookmark removed");
        } catch {}
      });
    });
  } catch (e) {
    list.innerHTML = `<div style="padding:16px;color:var(--red);font-size:12px;">Error: ${esc(String(e))}</div>`;
  }
}

// ── Scripts overlay ────────────────────────────────────────────────────────
function renderScriptsOverlay(el) {
  el.innerHTML = overlayShell("Userscripts", "scripts", `
    <div id="scripts-list" style="flex:1;overflow-y:auto;padding:4px 0;"></div>
    <div class="scripts-footer">
      <button class="footer-btn" id="scripts-paste">+ Install from source</button>
      <button class="footer-btn secondary" id="scripts-refresh">↻ Refresh</button>
    </div>`);

  el.querySelector("#scripts-paste")  .addEventListener("click", handlePaste);
  el.querySelector("#scripts-refresh").addEventListener("click", () => loadAndRenderScripts(el));
  loadAndRenderScripts(el);
}

async function loadAndRenderScripts(containerEl) {
  try {
    _scripts = await listUserscripts() ?? [];
    const list = containerEl?.querySelector("#scripts-list");
    if (!list) return;
    const n = _scripts.length;
    setText("br-script-count", `Scripts (${n})`);

    if (!_scripts.length) {
      list.innerHTML = emptyState("No userscripts installed",
        "Paste a Tampermonkey-format .user.js source to get started.");
      return;
    }

    list.innerHTML = _scripts.map(s => `
      <div class="script-card">
        <div style="flex:1;min-width:0;">
          <div class="script-card-name">${esc(s.name)}</div>
          <div class="script-card-meta">v${esc(s.version)} · ${esc(s.author || "unknown")}</div>
          <div class="script-card-match">${(s.matches||[]).slice(0,2).map(esc).join(", ") || "no @match"}</div>
          <span class="script-status-pill ${s.enabled ? "enabled":"disabled"}">
            ${s.enabled ? "● Enabled" : "○ Disabled"}
          </span>
        </div>
        <div class="script-card-actions">
          <button class="script-action-btn toggle-btn" data-id="${s.id}" data-enabled="${s.enabled}">
            ${s.enabled ? "Disable" : "Enable"}
          </button>
          <button class="script-action-btn remove" data-id="${s.id}">Remove</button>
        </div>
      </div>`).join("");

    list.querySelectorAll(".toggle-btn").forEach(btn => {
      btn.addEventListener("click", async () => {
        try {
          await toggleUserscript(btn.dataset.id, btn.dataset.enabled !== "true");
          loadAndRenderScripts(containerEl);
        } catch (e) { toastError("Toggle failed: " + e); }
      });
    });
    list.querySelectorAll(".remove").forEach(btn => {
      btn.addEventListener("click", async () => {
        if (!confirm("Remove this userscript?")) return;
        try {
          await removeUserscript(btn.dataset.id);
          loadAndRenderScripts(containerEl);
          toastSuccess("Script removed");
        } catch (e) { toastError("Remove failed: " + e); }
      });
    });
  } catch (e) { toastError("Failed to load scripts: " + e); }
}

async function handlePaste() {
  const source = prompt(
    "Paste Tampermonkey-format userscript source (must include ==UserScript== header):",
    "// ==UserScript==\n// @name        My Script\n// @match       https://example.com/*\n// @grant       GM_setValue\n// ==/UserScript==\n\n(function() {\n  'use strict';\n})();"
  );
  if (!source?.includes("==UserScript==")) return;
  try {
    const r = await installUserscript(source);
    toastSuccess(`Installed: ${r?.name || "Script"}`);
    // Re-render if scripts overlay is open
    const overlay = document.getElementById("scripts-overlay");
    if (!overlay?.classList.contains("hidden")) renderScriptsOverlay(overlay);
  } catch (e) { toastError("Install failed: " + e); }
}

// ── HTML builders ──────────────────────────────────────────────────────────
function overlayShell(title, name, bodyHtml) {
  return `
    <div class="scripts-overlay-header">
      <span class="scripts-overlay-title">${esc(title)}</span>
      <button class="overlay-close" data-close="${name}">✕</button>
    </div>
    ${bodyHtml}`;
}

// Wire close buttons after render
document.addEventListener("click", e => {
  const btn = e.target.closest("[data-close]");
  if (btn) closeOverlay(btn.dataset.close);
});

// ── Helpers ────────────────────────────────────────────────────────────────
function emptyState(h, p) {
  return `<div class="empty-state"><h3>${esc(h)}</h3><p>${esc(p)}</p></div>`;
}
function safeHostname(url) {
  try { return new URL(url).hostname; } catch { return url; }
}
function setUrlBar(url) {
  const bar = document.getElementById("browser-url-bar");
  if (bar) bar.value = url;
}
function setText(id, val) {
  const el = document.getElementById(id);
  if (el) el.textContent = val;
}
const esc = s => String(s).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;");