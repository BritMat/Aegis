# Browser & Userscript Engine

## Architecture

The privacy browser opens as a **secondary Tauri `WebviewWindow`** (separate from the editor). Every page that loads in this window gets the following injected *before* any page scripts execute, via `WebviewBuilder::initialization_script()`:

1. **GM_ API** (`src/lib/gm-api.js`) — provides the full Tampermonkey surface
2. **Userscript loader** — calls `invoke("get_userscripts_for_url")` and evals each matching script

This means scripts run in the page's own context with access to the DOM, `window`, and `document` — exactly as Tampermonkey works.

## Userscript format (Tampermonkey-compatible)

```js
// ==UserScript==
// @name         My Script
// @namespace    http://tampermonkey.net/
// @version      1.0.0
// @description  Does something useful
// @author       You
// @match        https://example.com/*
// @match        https://*.example.com/*
// @exclude      https://example.com/admin/*
// @grant        GM_getValue
// @grant        GM_setValue
// @grant        GM_xmlhttpRequest
// @run-at       document-idle
// ==/UserScript==

(function () {
  'use strict';
  // your code here
})();
```

Multiple `@match` and `@grant` directives are supported. `@include` (glob or `/regex/`) is also parsed.

## @match URL pattern spec

Patterns follow the Chrome extension match pattern format:

```
<scheme>://<host><path>
```

| Pattern | Matches |
|---------|---------|
| `https://example.com/*` | Any path on example.com (https only) |
| `https://*.example.com/*` | Any subdomain of example.com |
| `*://example.com/*` | http and https |
| `<all_urls>` | Everything |

Patterns are converted to Rust `Regex` at match time. `@exclude` patterns use the same format and override `@match`.

## GM_ API surface

| Function | Implementation |
|----------|---------------|
| `GM_getValue(key, default)` | SQLite `gm_storage` table (per-script namespace) |
| `GM_setValue(key, value)` | SQLite `gm_storage` table |
| `GM_deleteValue(key)` | SQLite delete |
| `GM_listValues()` | SQLite SELECT keys |
| `GM_addStyle(css)` | Injects `<style>` into page `<head>` |
| `GM_xmlhttpRequest(details)` | **Rust reqwest proxy** — completely CORS-free |
| `GM_notification(details)` | OS notification via `tauri-plugin-notification` |
| `GM_openInTab(url)` | Opens URL in system browser via `tauri-plugin-shell` |
| `GM_setClipboard(text)` | Clipboard via `tauri-plugin-clipboard-manager` |
| `GM_log(...args)` | `console.log` + Rust `log::info!` |
| `GM.*` (async aliases) | All of the above via `GM` object |
| `GM_info` | Script metadata object (name, version, grants, etc.) |
| `unsafeWindow` | Alias for `window` (already in page scope) |

### GM_xmlhttpRequest — CORS bypass

This is the most powerful feature. When a script calls `GM_xmlhttpRequest`, the request details are sent to Rust via Tauri IPC. Rust uses `reqwest` to make the actual HTTP request — from the native process, not from WebView2. The browser's CORS policy simply doesn't apply. The full response (status, headers, body) is returned to the script.

```js
GM_xmlhttpRequest({
  method: "POST",
  url: "https://api.example.com/data",
  headers: { "Content-Type": "application/json" },
  data: JSON.stringify({ key: "value" }),
  onload: (resp) => {
    console.log(resp.status, resp.responseText);
  },
  onerror: (resp) => console.error(resp),
});
```

## Privacy filter

Two protection layers run **before** any page loads:

### 1. Domain blocklist (`privacy.rs`)

~200 known tracking/advertising domains checked against the URL host. If the host or any parent domain matches, the request is silently blocked and the tracker-blocked counter is incremented. Covers: Google Analytics, GTM, Facebook Pixel, Criteo, Hotjar, Mixpanel, HubSpot, Marketo, LinkedIn Insight, TikTok Pixel, Microsoft Clarity, and many more.

### 2. URL parameter scrubbing (`privacy.rs`)

Before any navigation, known tracking query parameters are stripped from the URL. Covers: all UTM fields, `fbclid`, `gclid`, `msclkid`, `ttclid`, `li_fat_id`, `mc_eid`, `mc_cid`, `_hsenc`, `_hsmi`, `mkt_tok`, `igshid`, and 20+ others.

Both layers are configurable in Settings and can be disabled independently.

## SPA navigation support

The GM_ API loader hooks `history.pushState`, `history.replaceState`, and `popstate` events. When a single-page app navigates without a full page reload, the userscript loader re-runs `get_userscripts_for_url` with the new URL and injects any newly-matching scripts. This is identical to how Tampermonkey handles SPAs.

## Storage location

GM_ values are stored in the `gm_storage` table of `%APPDATA%\bm-aegis\search-index.db`:

```sql
CREATE TABLE gm_storage (
    script_id TEXT NOT NULL,  -- UUID of the installed script
    key       TEXT NOT NULL,
    value     TEXT NOT NULL,  -- JSON-serialised
    PRIMARY KEY (script_id, key)
);
```

Each script has its own namespace. Removing a script also purges its storage.
