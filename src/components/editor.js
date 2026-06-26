/**
 * editor.js — HTML Editor panel
 * Multi-tab, live split preview, Emmet, tag rename, format, word-wrap.
 */

import { createEditor, languageCompartment, languageForFile } from "../lib/codemirror-setup.js";
import { openFile, saveFile, saveFileAs, getRecentFiles } from "../lib/tauri-bridge.js";
import { EditorView } from "@codemirror/view";
import { toastSuccess, toastError, toast } from "../lib/toast.js";

// ── State ─────────────────────────────────────────────────────────────────
const _tabs    = [];        // [{ path, content, view, dirty }]
let _activeIdx = -1;
let _preview   = false;
let _wordWrap  = false;
let _previewTimer = null;

// DOM refs set in mount
let $sidebar, $statusEls;

// ── Mount ──────────────────────────────────────────────────────────────────
export function mountEditor(sidebarEl, panelEl, statusEls) {
  $sidebar   = sidebarEl;
  $statusEls = statusEls;
  buildSidebar(sidebarEl);
  buildPanel(panelEl);
  newTab();          // start with one empty tab
  loadRecent();
}

// ── Sidebar ───────────────────────────────────────────────────────────────
function buildSidebar(el) {
  el.innerHTML = `
    <div class="sidebar-section">
      <div class="sidebar-heading">File</div>
      <div class="sidebar-item" id="ed-open">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <path d="M1 3.5h11v8H1z" stroke="currentColor" stroke-width="1.2"/>
          <path d="M1 3.5l1.5-2h3.5l1 2" stroke="currentColor" stroke-width="1.2"/>
        </svg>
        Open <span class="sidebar-kbd">Ctrl+O</span>
      </div>
      <div class="sidebar-item" id="ed-save">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <rect x="1" y="1" width="11" height="11" rx="1" stroke="currentColor" stroke-width="1.2"/>
          <rect x="3" y="1" width="4" height="3.5" fill="currentColor" opacity=".5"/>
          <rect x="2" y="7" width="9" height="4" rx=".5" stroke="currentColor" stroke-width="1"/>
        </svg>
        Save <span class="sidebar-kbd">Ctrl+S</span>
      </div>
      <div class="sidebar-item" id="ed-save-as">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <rect x="1" y="1" width="11" height="11" rx="1" stroke="currentColor" stroke-width="1.2"/>
          <path d="M6.5 4.5v4M4.5 6.5l2 2 2-2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
        </svg>
        Save As…
      </div>
      <div class="sidebar-item" id="ed-new">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <rect x="1" y="1" width="11" height="11" rx="1" stroke="currentColor" stroke-width="1.2"/>
          <path d="M6.5 4v5M4 6.5h5" stroke="currentColor" stroke-width="1.3" stroke-linecap="round"/>
        </svg>
        New Tab
      </div>
    </div>
    <div class="sidebar-section">
      <div class="sidebar-heading">View</div>
      <div class="sidebar-item" id="ed-toggle-preview">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <rect x="1" y="1" width="5" height="11" rx="1" stroke="currentColor" stroke-width="1.2"/>
          <rect x="7" y="1" width="5" height="11" rx="1" stroke="currentColor" stroke-width="1.2"/>
        </svg>
        Live Preview <span class="sidebar-kbd" id="ed-preview-badge">Off</span>
      </div>
      <div class="sidebar-item" id="ed-toggle-wrap">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <path d="M1 4h11M1 6.5h8a2 2 0 010 4H7" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
          <path d="M5 8.5l2 2-2 2" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
        </svg>
        Word Wrap <span class="sidebar-kbd" id="ed-wrap-badge">Off</span>
      </div>
      <div class="sidebar-item" id="ed-format">
        <svg width="13" height="13" viewBox="0 0 13 13" fill="none">
          <path d="M1 3h11M1 6.5h7M1 10h9" stroke="currentColor" stroke-width="1.2" stroke-linecap="round"/>
        </svg>
        Format <span class="sidebar-kbd">Ctrl+⇧+F</span>
      </div>
    </div>
    <div class="sidebar-section">
      <div class="sidebar-heading">Recent</div>
      <div id="ed-recent"></div>
    </div>
    <div class="sidebar-section" style="flex:1;min-height:0;display:flex;flex-direction:column;">
      <div class="sidebar-heading">Lint</div>
      <div id="ed-lint" style="overflow-y:auto;flex:1;">
        <div class="lint-entry" style="color:var(--text-3)">No issues</div>
      </div>
    </div>`;

  el.querySelector("#ed-open")           .addEventListener("click", handleOpen);
  el.querySelector("#ed-save")           .addEventListener("click", handleSave);
  el.querySelector("#ed-save-as")        .addEventListener("click", handleSaveAs);
  el.querySelector("#ed-new")            .addEventListener("click", () => newTab());
  el.querySelector("#ed-toggle-preview") .addEventListener("click", togglePreview);
  el.querySelector("#ed-toggle-wrap")    .addEventListener("click", toggleWordWrap);
  el.querySelector("#ed-format")         .addEventListener("click", formatDoc);
}

// ── Panel ──────────────────────────────────────────────────────────────────
let $tabStrip, $editorArea, $previewPane, $divider;

function buildPanel(panelEl) {
  panelEl.innerHTML = `
    <!-- Tab strip + toolbar -->
    <div class="editor-top-bar" id="ed-topbar">
      <div id="ed-tab-strip" style="display:flex;align-items:stretch;flex:1;gap:1px;overflow-x:auto;scrollbar-width:none;"></div>
      <div style="display:flex;gap:4px;align-items:center;flex-shrink:0;margin-left:8px;">
        <span class="lint-badge ok" id="ed-lint-badge">✓ OK</span>
        <button class="icon-btn" id="ed-btn-new">+ New</button>
        <button class="icon-btn" id="ed-btn-format">Format</button>
        <button class="icon-btn" id="ed-btn-preview">⧉ Preview</button>
        <button class="icon-btn primary" id="ed-btn-save">Save</button>
      </div>
    </div>

    <!-- Editor + optional preview pane -->
    <div id="ed-split" style="flex:1;display:flex;overflow:hidden;">
      <div id="ed-editor-pane" style="flex:1;overflow:hidden;display:flex;flex-direction:column;"></div>
      <div id="ed-divider" class="hidden" style="
        width:4px;background:var(--border);cursor:col-resize;flex-shrink:0;
        border-left:1px solid var(--border-hi);"></div>
      <div id="ed-preview-pane" class="hidden" style="flex:1;overflow:hidden;display:flex;flex-direction:column;">
        <div style="
          display:flex;align-items:center;justify-content:space-between;
          padding:4px 10px;background:var(--bg-panel);border-bottom:1px solid var(--border);
          font-size:11px;color:var(--text-3);">
          <span>LIVE PREVIEW</span>
          <span id="ed-preview-url" style="font-family:monospace;"></span>
        </div>
        <iframe id="ed-preview-frame"
          sandbox="allow-scripts allow-same-origin"
          style="flex:1;border:none;background:#fff;"></iframe>
      </div>
    </div>`;

  $tabStrip    = panelEl.querySelector("#ed-tab-strip");
  $editorArea  = panelEl.querySelector("#ed-editor-pane");
  $previewPane = panelEl.querySelector("#ed-preview-pane");
  $divider     = panelEl.querySelector("#ed-divider");

  panelEl.querySelector("#ed-btn-new")    .addEventListener("click", () => newTab());
  panelEl.querySelector("#ed-btn-save")   .addEventListener("click", handleSave);
  panelEl.querySelector("#ed-btn-format") .addEventListener("click", formatDoc);
  panelEl.querySelector("#ed-btn-preview").addEventListener("click", togglePreview);

  wireDividerDrag();

  // Global keyboard shortcuts
  document.addEventListener("keydown", e => {
    if (!e.ctrlKey && !e.metaKey) return;
    if (e.key === "s")  { e.preventDefault(); handleSave(); }
    if (e.key === "S" && e.shiftKey) { e.preventDefault(); formatDoc(); }
    if (e.key === "w" && e.altKey)   { e.preventDefault(); closeTab(_activeIdx); }
  });
}

// ── Tab management ─────────────────────────────────────────────────────────
function newTab(path = null, content = DEFAULT_HTML) {
  const mount = document.createElement("div");
  mount.style.cssText = "flex:1;overflow:hidden;display:none;";
  $editorArea.appendChild(mount);

  const tab = {
    path,
    content,
    dirty: false,
    mount,
    view: null,
  };

  tab.view = createEditor(
    mount,
    content,
    () => { tab.dirty = true; tab.content = tab.view.state.doc.toString(); refreshTabChip(tab); schedulePreview(tab); },
    ({ line, col }) => { if ($statusEls?.pos) $statusEls.pos.textContent = `Ln ${line}, Col ${col}`; },
    (warnings) => renderLint(warnings),
  );

  _tabs.push(tab);
  const idx = _tabs.length - 1;
  activateTab(idx);
  renderTabStrip();
}

function activateTab(idx) {
  if (idx < 0 || idx >= _tabs.length) return;

  // Hide old
  if (_activeIdx >= 0 && _tabs[_activeIdx]) {
    _tabs[_activeIdx].mount.style.display = "none";
  }

  _activeIdx = idx;
  const tab  = _tabs[idx];
  tab.mount.style.display = "flex";
  tab.mount.style.flexDirection = "column";

  updateFileStatus(tab);
  renderTabStrip();

  // Focus editor
  setTimeout(() => tab.view?.focus(), 50);

  if (_preview) schedulePreview(tab);
}

function closeTab(idx) {
  const tab = _tabs[idx];
  if (!tab) return;
  if (tab.dirty && !confirm("Close without saving?")) return;

  tab.view?.destroy();
  tab.mount.remove();
  _tabs.splice(idx, 1);

  if (_tabs.length === 0) { newTab(); return; }
  activateTab(Math.min(idx, _tabs.length - 1));
  renderTabStrip();
}

function renderTabStrip() {
  $tabStrip.innerHTML = _tabs.map((tab, i) => {
    const name = tab.path ? tab.path.split(/[\\/]/).pop() : "untitled.html";
    const active = i === _activeIdx;
    return `<div class="file-tab-chip ${active ? "active" : ""}" data-idx="${i}">
      <span>${esc(name)}</span>
      <span class="chip-dirty${tab.dirty ? "" : " hidden"}" style="margin-left:3px">●</span>
      <span data-close="${i}" style="
        margin-left:5px;opacity:.5;font-size:12px;line-height:1;
        cursor:pointer;padding:0 2px;border-radius:2px;" title="Close">✕</span>
    </div>`;
  }).join("");

  $tabStrip.querySelectorAll(".file-tab-chip").forEach(chip => {
    chip.addEventListener("click", e => {
      const closeBtn = e.target.closest("[data-close]");
      if (closeBtn) { closeTab(parseInt(closeBtn.dataset.close, 10)); return; }
      activateTab(parseInt(chip.dataset.idx, 10));
    });
  });
}

function refreshTabChip(tab) {
  const idx = _tabs.indexOf(tab);
  if (idx < 0) return;
  const chips = $tabStrip?.querySelectorAll(".file-tab-chip");
  if (!chips?.[idx]) return;
  const dirty = chips[idx].querySelector(".chip-dirty");
  if (dirty) dirty.classList.toggle("hidden", !tab.dirty);
}

// ── File operations ────────────────────────────────────────────────────────
async function handleOpen() {
  try {
    const result = await openFile();
    if (!result) return;
    // Check if already open
    const existing = _tabs.findIndex(t => t.path === result.path);
    if (existing >= 0) { activateTab(existing); return; }
    newTab(result.path, result.content);
    loadRecent();
  } catch (e) { toastError("Could not open file: " + e); }
}

async function handleSave() {
  const tab = _tabs[_activeIdx];
  if (!tab) return;
  if (!tab.path) return handleSaveAs();
  try {
    await saveFile(tab.path, tab.view.state.doc.toString());
    tab.dirty = false;
    refreshTabChip(tab);
    toastSuccess("Saved");
  } catch (e) { toastError("Could not save: " + e); }
}

async function handleSaveAs() {
  const tab = _tabs[_activeIdx];
  if (!tab) return;
  try {
    const path = await saveFileAs(tab.view.state.doc.toString());
    if (!path) return;
    tab.path  = path;
    tab.dirty = false;
    refreshTabChip(tab);
    updateFileStatus(tab);
    renderTabStrip();
  } catch (e) { toastError("Could not save: " + e); }
}

function updateFileStatus(tab) {
  if ($statusEls?.file) $statusEls.file.textContent = tab?.path || "untitled.html";
  if ($statusEls?.lang) {
    const ext = (tab?.path || "html").split(".").pop().toUpperCase();
    $statusEls.lang.textContent = ext;
  }
}

// ── Live preview ───────────────────────────────────────────────────────────
function togglePreview() {
  _preview = !_preview;
  $previewPane?.classList.toggle("hidden", !_preview);
  $divider?.classList.toggle("hidden", !_preview);
  const _pb = document.getElementById("ed-preview-badge"); if (_pb) _pb.textContent = _preview ? "On" : "Off";
  const btn = document.getElementById("ed-btn-preview");
  if (btn) btn.textContent = _preview ? "⧉ Hide Preview" : "⧉ Preview";
  if (_preview && _activeIdx >= 0) schedulePreview(_tabs[_activeIdx]);
}

function schedulePreview(tab) {
  if (!_preview) return;
  clearTimeout(_previewTimer);
  _previewTimer = setTimeout(() => renderPreview(tab), 400);
}

function renderPreview(tab) {
  const frame = document.getElementById("ed-preview-frame");
  if (!frame) return;
  const doc = tab.view.state.doc.toString();
  const blob = new Blob([doc], { type: "text/html" });
  const url  = URL.createObjectURL(blob);
  // Revoke the old URL after loading
  const prev = frame.src;
  frame.src  = url;
  frame.onload = () => { if (prev?.startsWith("blob:")) URL.revokeObjectURL(prev); };
}

// ── Word wrap ──────────────────────────────────────────────────────────────
function toggleWordWrap() {
  _wordWrap = !_wordWrap;
  const badge = document.getElementById("ed-wrap-badge");
  if (badge) badge.textContent = _wordWrap ? "On" : "Off";
  // Apply to all tabs
  const ext = _wordWrap
    ? EditorView.lineWrapping
    : [];
  _tabs.forEach(tab => {
    if (!tab.view) return;
    tab.view.dispatch({
      effects: EditorView.scrollIntoView(tab.view.state.selection.main.head),
    });
    // Toggle lineWrapping compartment
    const { wordWrapCompartment } = tab.view;
    if (wordWrapCompartment) {
      tab.view.dispatch({ effects: wordWrapCompartment.reconfigure(_wordWrap ? EditorView.lineWrapping : []) });
    }
  });
}

// ── Format HTML ────────────────────────────────────────────────────────────
function formatDoc() {
  const tab = _tabs[_activeIdx];
  if (!tab?.view) return;
  const raw = tab.view.state.doc.toString();
  const formatted = formatHtml(raw);
  if (formatted === raw) return;
  tab.view.dispatch({
    changes: { from: 0, to: tab.view.state.doc.length, insert: formatted },
    selection: { anchor: 0 },
  });
}

/**
 * Simple HTML formatter: re-indents tags based on nesting depth.
 * Not a full pretty-printer but handles the common case well.
 */
function formatHtml(html) {
  const VOID = new Set(["area","base","br","col","embed","hr","img","input",
    "link","meta","param","source","track","wbr"]);
  const INLINE = new Set(["a","abbr","b","bdi","bdo","br","cite","code","data",
    "dfn","em","i","kbd","mark","q","rp","rt","ruby","s","samp","small","span",
    "strong","sub","sup","time","u","var","wbr"]);
  const RAW = new Set(["script","style","pre","textarea"]);

  // Tokenise tags
  const tokens = [];
  const re = /(<[^>]+>|[^<]+)/g;
  let m;
  while ((m = re.exec(html)) !== null) tokens.push(m[0]);

  const lines = [];
  let indent = 0;
  const ind = (n) => "  ".repeat(Math.max(0, n));
  let inRaw = null;

  for (const tok of tokens) {
    const text = tok.trim();
    if (!text) continue;

    if (inRaw) {
      // Inside script/style/pre — preserve content
      lines.push(tok);
      const closeRe = new RegExp(`<\\/${inRaw}\\s*>`, "i");
      if (closeRe.test(tok)) { indent--; inRaw = null; }
      continue;
    }

    if (!text.startsWith("<")) {
      // Text node
      lines.push(ind(indent) + text);
      continue;
    }

    const selfClose  = /\/>$/.test(text);
    const isClose    = /^<\//.test(text);
    const tagMatch   = text.match(/^<\/?([a-zA-Z][a-zA-Z0-9:-]*)/);
    const tagName    = tagMatch?.[1]?.toLowerCase() ?? "";

    if (selfClose || VOID.has(tagName)) {
      lines.push(ind(indent) + text);
    } else if (isClose) {
      indent = Math.max(0, indent - 1);
      lines.push(ind(indent) + text);
    } else if (INLINE.has(tagName)) {
      lines.push(ind(indent) + text);
    } else {
      lines.push(ind(indent) + text);
      if (!selfClose) {
        if (RAW.has(tagName)) { inRaw = tagName; }
        indent++;
      }
    }
  }

  return lines.join("\n");
}

// ── Recent files ───────────────────────────────────────────────────────────
async function loadRecent() {
  try {
    const recent = await getRecentFiles() ?? [];
    const list = document.getElementById("ed-recent");
    if (!list) return;
    if (!recent.length) {
      list.innerHTML = `<div class="lint-entry" style="color:var(--text-3)">None</div>`;
      return;
    }
    list.innerHTML = recent.slice(0, 8).map(p => {
      const name = p.split(/[\\/]/).pop();
      return `<div class="sidebar-item recent-item" data-path="${esc(p)}" title="${esc(p)}">
        <svg width="12" height="12" viewBox="0 0 12 12" fill="none">
          <rect x=".5" y=".5" width="11" height="11" rx="1" stroke="currentColor" stroke-width="1" opacity=".4"/>
          <path d="M2.5 4h7M2.5 6h7M2.5 8h4" stroke="currentColor" stroke-width=".8" opacity=".4"/>
        </svg>
        ${esc(name)}
      </div>`;
    }).join("");
    list.querySelectorAll(".recent-item").forEach(el => {
      el.addEventListener("click", () => openPath(el.dataset.path));
    });
  } catch {}
}

async function openPath(path) {
  try {
    const result = await openFile(path);
    if (result) { newTab(result.path, result.content); loadRecent(); }
  } catch {}
}

// ── Lint panel ─────────────────────────────────────────────────────────────
function renderLint(warnings) {
  const panel = document.getElementById("ed-lint");
  const badge = document.getElementById("ed-lint-badge");

  if (!warnings?.length) {
    if (panel) panel.innerHTML = `<div class="lint-entry" style="color:var(--text-3)">No issues</div>`;
    if (badge) { badge.textContent = "✓ OK"; badge.className = "lint-badge ok"; }
    if ($statusEls?.lint)    $statusEls.lint.textContent   = "Lint: OK";
    if ($statusEls?.lintDot) $statusEls.lintDot.className  = "status-dot ok";
    return;
  }

  const errors = warnings.filter(w => w.severity === "error").length;
  const warns  = warnings.filter(w => w.severity === "warning").length;

  if (panel) {
    panel.innerHTML = warnings.slice(0, 50).map(w => `
      <div class="lint-entry ${w.severity}" data-line="${w.line}">
        <span style="color:var(--text-3)">Ln ${w.line}</span> ${esc(w.message)}
      </div>`).join("");

    panel.querySelectorAll(".lint-entry[data-line]").forEach(el => {
      el.addEventListener("click", () => {
        const tab = _tabs[_activeIdx];
        if (!tab?.view) return;
        const ln  = parseInt(el.dataset.line, 10);
        const max = tab.view.state.doc.lines;
        const lineObj = tab.view.state.doc.line(Math.min(ln, max));
        tab.view.dispatch({ selection: { anchor: lineObj.from }, scrollIntoView: true });
        tab.view.focus();
      });
    });
  }

  const label = errors > 0
    ? `✕ ${errors} error${errors > 1 ? "s" : ""}`
    : `⚠ ${warns} warning${warns > 1 ? "s" : ""}`;
  const cls = errors > 0 ? "lint-badge error" : "lint-badge warn";
  if (badge) { badge.textContent = label; badge.className = cls; }
  if ($statusEls?.lint)    $statusEls.lint.textContent  = label;
  if ($statusEls?.lintDot) $statusEls.lintDot.className = errors > 0 ? "status-dot error" : "status-dot warn";
}

// ── Divider drag (resizable preview) ─────────────────────────────────────
function wireDividerDrag() {
  if (!$divider) return;
  let dragging = false;
  let startX, startLeftW;

  $divider.addEventListener("mousedown", e => {
    dragging = true;
    startX     = e.clientX;
    const split = document.getElementById("ed-split");
    startLeftW  = document.getElementById("ed-editor-pane").offsetWidth;
    e.preventDefault();
  });

  document.addEventListener("mousemove", e => {
    if (!dragging) return;
    const split  = document.getElementById("ed-split");
    const totalW = split.offsetWidth - $divider.offsetWidth;
    const delta  = e.clientX - startX;
    const newLeft = Math.max(200, Math.min(totalW - 200, startLeftW + delta));
    document.getElementById("ed-editor-pane").style.flex = "none";
    document.getElementById("ed-editor-pane").style.width = newLeft + "px";
    document.getElementById("ed-preview-pane").style.flex = "1";
  });

  document.addEventListener("mouseup", () => { dragging = false; });
}

// ── Defaults ──────────────────────────────────────────────────────────────
const DEFAULT_HTML = `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Document</title>
</head>
<body>
  
</body>
</html>`;

// ── Utility ───────────────────────────────────────────────────────────────
const esc = s => String(s).replace(/&/g,"&amp;").replace(/</g,"&lt;").replace(/>/g,"&gt;");


