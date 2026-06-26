# BM-Aegis — Architecture

## Overview

BM-Aegis is a **Tauri v2** desktop application that combines three modules:

```
┌──────────────────────────────────────────────────────────────────┐
│  BM-Aegis (Tauri v2 + WebView2)                                  │
│                                                                  │
│  ┌─────────────┐  ┌──────────────────┐  ┌────────────────────┐  │
│  │ HTML Editor │  │ Privacy Browser  │  │ Search             │  │
│  │ CodeMirror 6│  │ WebviewWindow    │  │ Local FTS5 + Web   │  │
│  │ + Rust lint │  │ + GM_ API        │  │ metasearch         │  │
│  └─────────────┘  └──────────────────┘  └────────────────────┘  │
│           │                │                      │              │
│           └────────────────┴──────────────────────┘              │
│                            │ Tauri IPC (invoke)                  │
│  ┌─────────────────────────▼────────────────────────────────┐    │
│  │  Rust Backend                                            │    │
│  │  commands.rs → editor/ | search/ | browser/             │    │
│  │  SQLite FTS5 (rusqlite bundled) | reqwest | regex        │    │
│  └──────────────────────────────────────────────────────────┘    │
└──────────────────────────────────────────────────────────────────┘
```

## Process model

| Process | Role |
|---------|------|
| Main (Rust) | Event loop, IPC handler, file I/O, HTTP proxy, SQLite |
| WebView2 (main window) | Renders the app UI (editor, search, settings panels) |
| WebView2 (browser window) | Navigates external URLs; has GM_ API injected |

The main window and browser window are **separate WebView2 instances**. They share no DOM but communicate through Tauri IPC.

## Data flow — Editor linting

```
User types → CM6 onChange → tauri-bridge.lintHtml()
  → invoke("lint_html") → Rust tokenizer::lint()
  → Vec<LintWarning> → JS → CM6 setDiagnostics()
  → Wavy underlines + sidebar entries
```
Debounced at 600 ms. The tokenizer runs in a `spawn_blocking` thread so it never blocks the Tauri event loop.

## Data flow — Browser page load

```
User enters URL → openBrowserWindow(url)
  → Rust: privacy::scrub_url()      (strip tracking params)
  → Rust: privacy::is_blocked()     (check tracker blocklist)
  → If blocked: increment stats, return
  → Else: WebviewWindowBuilder with initialization_script
      = GM_ API JS + userscript loader
  → Page loads in separate window
  → GM_ API calls navigate back via Tauri IPC invoke()
      GM_getValue  → gm_storage table in SQLite
      GM_xmlhttp   → Rust reqwest (CORS-free)
      GM_notif     → tauri-plugin-notification
```

## Storage layout

```
%APPDATA%\bm-aegis\
├── settings.json          ← user preferences
├── search-index.db        ← SQLite
│     docs (FTS5)          ← indexed file content
│     doc_meta             ← path + mtime for incremental indexing
│     gm_storage           ← GM_getValue/setValue persistent store
└── userscripts\
      <uuid>.json          ← script metadata + source per file
```

## Key dependencies

| Crate | Purpose |
|-------|---------|
| `tauri 2` | App framework, IPC, window management |
| `rusqlite` (bundled + vtab + full) | SQLite with FTS5, no system SQLite needed |
| `reqwest` (rustls-tls) | HTTP for metasearch + GM_xmlhttpRequest proxy |
| `regex` | HTML tag parsing, @match URL patterns |
| `scraper` | DuckDuckGo HTML result parsing |
| `walkdir` | Recursive directory indexing |
| `uuid` | Userscript IDs |
| `dirs` | OS data directory resolution |
| `parking_lot` | Fast Mutex for shared AppState |

| NPM package | Purpose |
|-------------|---------|
| `@codemirror/*` | Editor engine, HTML/CSS/JS language support, lint, autocomplete |
| `@tauri-apps/api` | Tauri v2 JS bindings |
| `vite` | Frontend bundler — outputs fully offline static assets |
