/**
 * search.js — Local FTS5 & Privacy Metasearch panel
 */

import { localSearch, metaSearch, indexDirectory, chooseDirectory } from "../lib/tauri-bridge.js";

let _mode = "local";

export function mountSearch(sidebarEl, panelEl) {
  buildSidebar(sidebarEl);
  buildPanel(panelEl);
}

function buildSidebar(el) {
  el.innerHTML = `
    <div class="sidebar-section">
      <div class="sidebar-heading">Search Mode</div>
      <div class="sidebar-item active" id="s-mode-local" data-mode="local">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <rect x="1" y="1" width="11" height="11" rx="1" stroke="currentColor" stroke-width="1.2"/>
          <path d="M3 4h7M3 6.5h7M3 9h4" stroke="currentColor" stroke-width="1" stroke-linecap="round"/>
        </svg>
        Local Index
      </div>
      <div class="sidebar-item" id="s-mode-web" data-mode="web">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <circle cx="6.5" cy="6.5" r="5.5" stroke="currentColor" stroke-width="1.2"/>
          <path d="M6.5 1c0 0-2 2-2 5.5S6.5 12 6.5 12" stroke="currentColor" stroke-width=".9"/>
          <path d="M1 6.5h11" stroke="currentColor" stroke-width=".9"/>
        </svg>
        Web (Private)
      </div>
    </div>
    <div class="sidebar-section">
      <div class="sidebar-heading">Index</div>
      <div class="sidebar-item" id="s-index-dir">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <path d="M1 4.5h11v7H1z" stroke="currentColor" stroke-width="1.2"/>
          <path d="M1 4.5l1.5-2.5h4.5l1 2.5" stroke="currentColor" stroke-width="1.2"/>
          <path d="M6.5 6.5v3M5 8l1.5 1.5 1.5-1.5" stroke="currentColor" stroke-width="1.1" stroke-linecap="round"/>
        </svg>
        Index Directory…
      </div>
      <div id="s-index-status" style="padding:4px 12px;font-size:11px;color:var(--text-3);min-height:18px;"></div>
    </div>`;

  el.querySelector("#s-mode-local").addEventListener("click", () => setMode("local"));
  el.querySelector("#s-mode-web")  .addEventListener("click", () => setMode("web"));
  el.querySelector("#s-index-dir") .addEventListener("click", handleIndex);
}

function setMode(mode) {
  _mode = mode;
  document.getElementById("s-mode-local")?.classList.toggle("active", mode === "local");
  document.getElementById("s-mode-web")  ?.classList.toggle("active", mode === "web");
  const lbl = document.getElementById("s-mode-label");
  if (lbl) lbl.textContent = mode === "local" ? "Searching local index" : "Searching the web privately";
}

function buildPanel(el) {
  el.innerHTML = `
    <div class="search-bar-area">
      <div class="search-input-wrap">
        <svg width="14" height="14" viewBox="0 0 14 14" fill="none">
          <circle cx="6" cy="6" r="5" stroke="currentColor" stroke-width="1.3"/>
          <path d="M10 10L13 13" stroke="currentColor" stroke-width="1.5" stroke-linecap="round"/>
        </svg>
        <input id="search-input" type="text"
               placeholder="Search your files or the web privately…"
               autocomplete="off" spellcheck="false"/>
        <button id="search-submit">Search</button>
      </div>
      <div class="search-mode-pills">
        <div class="mode-pill active" id="sp-local">
          <svg width="11" height="11" viewBox="0 0 11 11" fill="none"><rect x=".5" y=".5" width="10" height="10" rx="1" stroke="currentColor" stroke-width="1"/><path d="M2 3.5h7M2 5.5h7M2 7.5h4" stroke="currentColor" stroke-width=".8"/></svg>
          Local Index
        </div>
        <div class="mode-pill" id="sp-web">
          <svg width="11" height="11" viewBox="0 0 11 11" fill="none"><circle cx="5.5" cy="5.5" r="4.5" stroke="currentColor" stroke-width="1"/><path d="M5.5 1c0 0-1.8 1.5-1.8 4.5s1.8 4.5 1.8 4.5" stroke="currentColor" stroke-width=".8"/><path d="M1 5.5h9" stroke="currentColor" stroke-width=".8"/></svg>
          Private Web
        </div>
      </div>
      <div id="s-mode-label" style="font-size:11px;color:var(--text-3);">Searching local index</div>
    </div>
    <div id="search-results" style="flex:1;overflow-y:auto;padding:10px;">
      <div class="empty-state">
        <svg width="40" height="40" viewBox="0 0 40 40" fill="none" style="opacity:.2">
          <circle cx="18" cy="18" r="14" stroke="currentColor" stroke-width="2.5"/>
          <path d="M28 28L38 38" stroke="currentColor" stroke-width="3" stroke-linecap="round"/>
        </svg>
        <h3>No results yet</h3>
        <p>Type a query above. Use Local mode to search indexed files, or Web mode for private DuckDuckGo results.</p>
      </div>
    </div>`;

  const input  = el.querySelector("#search-input");
  el.querySelector("#search-submit") .addEventListener("click", () => run(input.value.trim()));
  el.querySelector("#sp-local")      .addEventListener("click", () => { setMode("local"); el.querySelector("#sp-local").classList.add("active"); el.querySelector("#sp-web").classList.remove("active"); });
  el.querySelector("#sp-web")        .addEventListener("click", () => { setMode("web");   el.querySelector("#sp-web").classList.add("active");   el.querySelector("#sp-local").classList.remove("active"); });
  input.addEventListener("keydown", e => { if (e.key === "Enter") run(input.value.trim()); });
}

async function run(query) {
  if (!query) return;
  const box = document.getElementById("search-results");
  box.innerHTML = `<div style="padding:16px;font-size:12px;color:var(--text-3);">Searching…</div>`;
  try {
    if (_mode === "local") {
      const items = await localSearch(query);
      renderLocal(box, items, query);
    } else {
      const items = await metaSearch(query);
      renderWeb(box, items, query);
    }
  } catch (e) {
    box.innerHTML = `<div style="padding:16px;font-size:12px;color:var(--red);">Error: ${esc(String(e))}</div>`;
  }
}

function renderLocal(box, items, query) {
  if (!items?.length) {
    box.innerHTML = `<div class="empty-state"><h3>No results</h3><p>No indexed files matched "${esc(query)}". Try indexing more directories from the sidebar.</p></div>`;
    return;
  }
  box.innerHTML = items.map(r => `
    <div class="search-card">
      <div class="search-card-title">${esc(r.title || r.path.split(/[\\/]/).pop())}</div>
      <div class="search-card-path">${esc(r.path)}</div>
      <div class="search-card-snippet">${highlight(esc(r.snippet), query)}</div>
    </div>`).join("");
}

function renderWeb(box, items, query) {
  if (!items?.length) {
    box.innerHTML = `<div class="empty-state"><h3>No results</h3><p>No web results for "${esc(query)}".</p></div>`;
    return;
  }
  box.innerHTML = items.map(r => `
    <div class="search-card" data-url="${esc(r.url)}">
      <div class="search-card-title">${esc(r.title)}</div>
      <div class="search-card-url">${esc(r.url)}</div>
      <div class="search-card-snippet">${highlight(esc(r.snippet), query)}</div>
    </div>`).join("");

  box.querySelectorAll(".search-card[data-url]").forEach(card => {
    card.addEventListener("click", async () => {
      const { openBrowserWindow } = await import("../lib/tauri-bridge.js");
      openBrowserWindow(card.dataset.url).catch(() => {});
    });
  });
}

async function handleIndex() {
  const status = document.getElementById("s-index-status");
  try {
    const dir = await chooseDirectory();
    if (!dir) return;
    if (status) status.textContent = "Indexing…";
    const result = await indexDirectory(dir);
    if (status) status.textContent = `✓ ${result?.indexed ?? 0} files indexed`;
  } catch (e) {
    if (status) status.textContent = "Error: " + String(e);
  }
}

function highlight(html, query) {
  return query.split(/\s+/).filter(Boolean).reduce((h, w) => {
    const re = new RegExp(`(${w.replace(/[.*+?^${}()|[\]\\]/g,"\\$&")})`, "gi");
    return h.replace(re, "<mark>$1</mark>");
  }, html);
}
const esc = s => String(s).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;");
