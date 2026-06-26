# Changelog

All notable changes to BM-Aegis are documented here.
Format follows [Keep a Changelog](https://keepachangelog.com/en/1.0.0/).

---

## [Unreleased] — v0.1.0

### Added

**HTML Editor**
- CodeMirror 6 editor with full HTML, CSS, and JavaScript language support
- Stack-based Rust tokenizer for real-time lint: detects unclosed tags, mismatched nesting, void-element closing tags, and unknown elements
- Context-aware HTML5 autocomplete: suggestions respect the content model of the current open tag (300+ tag + attribute completions)
- Attribute-level completions with element-specific detail strings and snippet insertion
- Lint sidebar panel with click-to-navigate to the offending line
- File tabs with dirty indicator, Save / Save As, New File, recent files list
- Ctrl+S global save shortcut
- Syntax highlighting via CodeMirror 6 BM Dark theme (custom palette)
- 100% offline — no network requests required for any editor feature

**Privacy Browser**
- Dedicated Tauri `WebviewWindow` for browsing external URLs
- ~200-entry tracker domain blocklist (Google Analytics, GTM, Facebook Pixel, Criteo, Hotjar, LinkedIn Insight, TikTok Pixel, Mixpanel, HubSpot, Marketo, Microsoft Clarity, and more)
- URL parameter scrubbing: removes UTM, fbclid, gclid, msclkid, ttclid, mc_eid, _hsenc, mkt_tok, and 30+ other tracking parameters before navigation
- Privacy stats badge showing cumulative blocked/scrubbed counts
- Address bar with URL normalisation (bare domain → https, bare search term → DuckDuckGo)

**Userscript Engine**
- Tampermonkey-format script installation via paste dialog
- Full `==UserScript==` header parsing: @name, @namespace, @version, @description, @author, @match, @exclude, @include, @grant, @run-at
- URL @match pattern matching (Chrome extension spec: `*`, `*.domain.com`, `<all_urls>`)
- @include supports both glob and `/regex/` formats
- Per-script enable/disable and removal
- GM_ API: `GM_getValue`, `GM_setValue`, `GM_deleteValue`, `GM_listValues`, `GM_addStyle`, `GM_xmlhttpRequest`, `GM_notification`, `GM_openInTab`, `GM_setClipboard`, `GM_log`
- Async `GM.*` alias object (Tampermonkey v4+ style)
- `GM_xmlhttpRequest` proxied through Rust reqwest — fully CORS-free
- Persistent GM_ storage in SQLite (survives restarts, isolated per script ID)
- SPA-aware: re-injects scripts on `pushState`/`popstate` navigation

**Search**
- Local FTS5 index backed by SQLite (bundled, no system install needed)
- Porter-stemmer tokenizer for English-language fuzzy matching
- Incremental indexing: unchanged files (by mtime) are skipped
- BM25 ranked results with `snippet()` highlighted excerpts
- Supports: html, htm, css, js, ts, md, txt, json, xml, yaml, rs, py, and more
- Privacy metasearch via DuckDuckGo HTML endpoint (no JS, no cookies, no fingerprinting)
- Tracking redirect unwrapping — real destination URLs always shown
- Click-to-open web results in the privacy browser

**Settings**
- Editor: font size, tab size, auto-save, lint debounce
- Browser: tracker blocking, param scrubbing, third-party cookies, default search engine
- Search: max index size, index hidden files
- App: restore last file on startup
- All settings persisted to `%APPDATA%\bm-aegis\settings.json`

**Infrastructure**
- Multi-platform GitHub Actions CI: Windows x64/x86, macOS arm64/x64, Linux x64
- Tauri v2 with rustls (no OpenSSL dependency on Windows)
- Release build with LTO + strip → minimal binary size
- All frontend assets bundled by Vite into a fully offline static bundle

---

## [Unreleased] — v0.2.0

### Added

**HTML Editor**
- **Live split preview** — iframe updated 400 ms after last keystroke; draggable divider to resize editor/preview split
- **Emmet abbreviation expansion** — Tab expands `div.container>ul>li*3`, `form>input[type=email]+button{Send}`, `!` (full boilerplate), `lorem`, `lorem30`, and all standard Emmet patterns
- **Tag auto-rename** — editing an open tag name automatically renames the paired close tag in the same CM6 transaction (and vice-versa)
- **Multi-file tabs** — open multiple files simultaneously, each with its own CM6 editor instance, dirty indicator, and close button
- **Format document** — `Ctrl+Shift+F` re-indents the full HTML document respecting void, inline, and raw-text elements
- **Word wrap toggle** — sidebar toggle switches `EditorView.lineWrapping` on all open tabs
- **Toast notifications** — save/open/error operations now surface non-blocking toasts (bottom-right)

**Privacy Browser**
- **EasyList ad-block filter engine** — parses `||domain^`, `/path/*`, `@@` exception, and `##.selector` cosmetic rules; ~300 built-in patterns; user list at `~/.bm-aegis/filters/custom.txt`
- **Cosmetic ad removal** — injects a `<style>` that hides `.advertisement`, `.sponsored-content`, `[data-ad]`, Taboola/Outbrain widgets, cookie banners, and 30+ more selectors
- **Fingerprint protection** — 10-layer JS injection: Canvas noise, AudioContext noise, WebGL vendor/renderer spoof, Navigator normalisation (hardwareConcurrency, deviceMemory, platform), Screen 1920×1080, Performance timing jitter, font enumeration block, Battery API fake, Referrer meta, WebRTC IP leak intercept
- **HTTPS automatic upgrade** — all `http://` URLs upgraded to `https://` before connection
- **Bookmarks** — ☆ toolbar button bookmarks current page; bookmark manager with folder grouping, search, and per-entry delete; SQLite-backed
- **Browser history** — visit log with date grouping ("Just now", "2 hr ago", "Yesterday"), full-text search, per-entry delete, clear all
- **Reader mode** — 📖 button activates a simplified reading layout (strips nav/sidebar, sets comfortable typography)
- **Adblock + tracker stats** — sidebar shows separate counters: tracker domains, URL params, ad patterns/elements

**Application**
- **Keyboard shortcuts panel** — `Ctrl+/` opens a reference of all shortcuts
- **Tab switching** — `Ctrl+1`–`4` jump to Editor / Browser / Search / Settings
- **Toast system** — lightweight non-blocking notifications for all user-facing operations
