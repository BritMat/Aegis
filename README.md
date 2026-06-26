<div align="center">

<img src="docs/assets/banner.png" alt="BM-Aegis" width="100%" />

# BM-Aegis

**Localized HTML Editor · Privacy Browser · Userscript Engine**

[![Build](https://github.com/BritMat/bm-aegis/actions/workflows/build.yml/badge.svg)](https://github.com/BritMat/bm-aegis/actions)
[![License: MIT](https://img.shields.io/badge/License-MIT-7c3aed.svg)](LICENSE)
[![Tauri](https://img.shields.io/badge/Tauri-v2-06b6d4?logo=tauri)](https://tauri.app)
[![Rust](https://img.shields.io/badge/Rust-stable-orange?logo=rust)](https://www.rust-lang.org)

*A lightweight, offline-capable desktop tool for HTML editing, private browsing,  
and local full-text search — engineered for Windows systems with constrained resources.*

[Download](#installation) · [Screenshots](#screenshots) · [Architecture](docs/ARCHITECTURE.md) · [Userscripts](docs/BROWSER_ADDONS.md)

</div>

---

## What It Is

BM-Aegis is a single Tauri application that brings three independent but complementary tools into one low-memory desktop experience:

| Module | What it does |
|--------|-------------|
| **HTML Editor** | CodeMirror 6 editor with a Rust stack-based tokenizer for real-time linting and full HTML5 context-aware autocomplete. Works 100% offline. |
| **Privacy Browser** | Chromium WebView2 browser with a 200-domain tracker blocklist, UTM/click-ID parameter scrubbing, and a built-in Tampermonkey-compatible userscript engine. |
| **Local Search** | SQLite FTS5 full-text index of your own files, plus a privacy metasearch that queries DuckDuckGo's HTML endpoint with no tracking or fingerprinting. |

### Design goals

- **Offline-first.** The editor, linter, and autocomplete engine require no internet. The Vite bundle is fully self-contained.
- **Low memory.** Target footprint is < 150 MB idle. Rust handles all heavy lifting; the WebView2 renderer is lean because it renders our own UI, not a full web app.
- **Privacy by default.** The browser blocks tracking requests at the network layer before any content loads.
- **Tampermonkey parity.** The GM_ API implementation covers `GM_getValue`, `GM_setValue`, `GM_xmlhttpRequest`, `GM_addStyle`, `GM_notification`, `GM_openInTab`, `GM_setClipboard`, `GM_log`, and the `GM.*` async equivalents.

---

## Screenshots

> Screenshots will be added after the first stable build.

---

## Installation

### Download a pre-built installer (recommended)

| Platform | Installer | Notes |
|----------|-----------|-------|
| Windows x64 | `.msi` / `.exe` | WebView2 required (ships with Windows 11, auto-installs on Win 10) |
| Windows x86 | `.exe` | 32-bit fallback for older hardware |
| macOS arm64 | `.dmg` | Apple Silicon |
| macOS x86_64 | `.dmg` | Intel |
| Linux x64 | `.AppImage` / `.deb` | |

Download from the [Releases](https://github.com/BritMat/bm-aegis/releases) page.

### Build from source

**Prerequisites**

| Tool | Version | Install |
|------|---------|---------|
| Rust | stable | `rustup install stable` |
| Node.js | ≥ 20 | [nodejs.org](https://nodejs.org) |
| WebView2 | any | [microsoft.com/webview2](https://developer.microsoft.com/en-us/microsoft-edge/webview2/) |

```bash
# Clone
git clone https://github.com/BritMat/bm-aegis.git
cd bm-aegis

# Install frontend deps
npm install

# Development mode (hot reload)
npm run tauri:dev

# Production build + installer
npm run tauri:build
```

---

## Features

### HTML Editor

- **Stack-based tokenizer** written in Rust — detects unclosed tags, mismatched nesting, and invalid closing tags in real time
- **HTML5 content model autocomplete** — suggestions adapt to the current open-tag context (e.g. inside `<ul>` only `<li>` is suggested at high priority; inside `<tr>` only `<td>`/`<th>`)
- **Attribute completions** — element-specific attributes with detail strings and snippet insertion
- **B&W lint gutter** — CodeMirror 6 wavy underlines (yellow = warning, red = error) with click-to-navigate in the sidebar
- **Multi-file tabs**, Save / Save As, recent files list
- Full **keyboard shortcut** support (Ctrl+S, Ctrl+Z/Y, find/replace, code folding)

### Privacy Browser

- Opens as a dedicated Tauri `WebviewWindow` with the GM_ initialization script injected on every page load
- **200+ tracker domain blocklist** — blocks connections to Google Analytics, GTM, Facebook Pixel, Criteo, Hotjar, Mixpanel, HubSpot, and many more before they load
- **Tracking parameter stripping** — removes UTM fields, `fbclid`, `gclid`, `msclkid`, `ttclid`, HubSpot `_hsenc`/`_hsmi`, Marketo `mkt_tok`, and 30+ others from every URL
- **Privacy stats** — running count of blocked trackers and stripped parameters shown in the status badge
- **Userscript engine** — Tampermonkey-format `.user.js` scripts installed via paste or file. Persistent `GM_getValue`/`GM_setValue` backed by SQLite. `GM_xmlhttpRequest` proxied through Rust (no CORS).

### Search

- **Local mode** — indexes any directory you point it at. Incremental (skips files unchanged since last index). FTS5 porter-stemmer ranking with highlighted snippet extraction.
- **Web mode** — queries DuckDuckGo's JavaScript-free HTML endpoint server-side. No cookies, no fingerprinting, tracking redirects resolved to real URLs.

---

## Project Structure

```
bm-aegis/
├── src/                        # Frontend (Vite + vanilla JS + CodeMirror 6)
│   ├── index.html
│   ├── style.css               # Rich dark UI theme
│   ├── main.js                 # Tab routing + window controls
│   ├── lib/
│   │   ├── tauri-bridge.js     # All invoke() wrappers
│   │   ├── gm-api.js           # GM_ API (injected into browser pages)
│   │   └── codemirror-setup.js # CM6 extensions + BM Dark theme
│   └── components/
│       ├── editor.js           # Editor panel
│       ├── browser.js          # Browser panel
│       ├── search.js           # Search panel
│       └── settings.js         # Settings panel
│
├── src-tauri/                  # Rust backend (Tauri v2)
│   ├── Cargo.toml
│   ├── tauri.conf.json
│   ├── capabilities/
│   │   └── default.json
│   └── src/
│       ├── main.rs             # Entry point
│       ├── lib.rs              # App setup + state
│       ├── commands.rs         # All #[tauri::command] handlers
│       ├── editor/
│       │   ├── tokenizer.rs    # Stack-based HTML linter
│       │   └── autocomplete.rs # HTML5 content-model completions
│       ├── search/
│       │   ├── indexer.rs      # SQLite FTS5 local indexer
│       │   └── metasearch.rs   # DuckDuckGo privacy metasearch
│       └── browser/
│           ├── mod.rs          # WebviewWindow + GM_ HTTP proxy
│           ├── userscripts.rs  # Tampermonkey-compatible engine
│           └── privacy.rs      # Tracker blocklist + URL scrubber
│
├── docs/
│   ├── ARCHITECTURE.md
│   ├── EDITOR_DESIGN.md
│   ├── BROWSER_ADDONS.md
│   └── SEARCH_DESIGN.md
│
└── .github/workflows/
    └── build.yml               # Multi-platform CI
```

---

## Userscript Quickstart

BM-Aegis is compatible with scripts written for Tampermonkey. Open the Browser panel, click **Scripts**, and paste your `.user.js` source.

```js
// ==UserScript==
// @name         Hide Cookie Banners
// @namespace    com.britmat.bm-aegis
// @version      1.0
// @description  Removes common cookie consent popups
// @match        https://*/*
// @grant        GM_addStyle
// ==/UserScript==

(function () {
  'use strict';
  GM_addStyle(`
    #cookiebanner, .cookie-notice, .gdpr-modal,
    [class*="cookie"], [id*="cookie"] { display: none !important; }
  `);
})();
```

**Supported GM_ API surface:**

| Function | Backed by |
|----------|-----------|
| `GM_getValue` / `GM_setValue` | SQLite via Tauri IPC |
| `GM_deleteValue` / `GM_listValues` | SQLite via Tauri IPC |
| `GM_addStyle` | Direct DOM injection |
| `GM_xmlhttpRequest` | Rust reqwest (CORS-free) |
| `GM_notification` | OS native notification |
| `GM_openInTab` | Tauri shell open |
| `GM_setClipboard` | Tauri clipboard plugin |
| `GM_log` | stdout + Rust log |
| `GM.* ` (async aliases) | All of the above |

---

## Data & Privacy

BM-Aegis stores data in your OS user data directory:

| Data | Location | Notes |
|------|----------|-------|
| Settings | `%APPDATA%\bm-aegis\settings.json` | Editable JSON |
| Search index | `%APPDATA%\bm-aegis\search-index.db` | SQLite, deletable |
| Userscripts | `%APPDATA%\bm-aegis\userscripts\` | One JSON per script |
| GM_ storage | `%APPDATA%\bm-aegis\search-index.db` | `gm_storage` table |

No telemetry. No analytics. No network calls except when you explicitly use the browser or web search.

---

## Contributing

PRs and issues are welcome. Before submitting:

```bash
# Lint Rust
cargo clippy --manifest-path src-tauri/Cargo.toml --all-targets -- -D warnings

# Format Rust
cargo fmt --manifest-path src-tauri/Cargo.toml

# Run Rust unit tests
cargo test --manifest-path src-tauri/Cargo.toml --lib
```

---

## Roadmap

- [ ] Live preview pane (split-screen HTML render)
- [ ] Embedded CSS / JS editor with respective linters
- [ ] Script editor within the Settings panel (edit `.user.js` in BM-Aegis itself)
- [ ] FTS5 result file preview with syntax highlighting
- [ ] Browser history and bookmarks
- [ ] Inline browser tab (child Webview embedded in main window rather than separate window)
- [ ] Userscript update checker

---

## License

[MIT](LICENSE) © 2024 BritMat
