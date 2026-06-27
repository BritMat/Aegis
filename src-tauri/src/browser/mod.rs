//! browser/mod.rs — WebviewWindow factory, adblock, history, fingerprint, HTTPS upgrade

pub mod privacy;
pub mod userscripts;
pub mod adblock;
pub mod history;
pub mod bookmarks;

use parking_lot::Mutex;
use tauri::{AppHandle, Manager, WebviewWindowBuilder, WebviewUrl};
use crate::{AppState, SharedState, commands::{XhrDetails, XhrResponse}};
use adblock::AdBlockEngine;

// ── Embedded JS files (bundled into binary) ─────────────────────────────
const GM_API_JS:      &str = include_str!("../../../src/lib/gm-api.js");
const FINGERPRINT_JS: &str = include_str!("../../../src/lib/fingerprint-protect.js");

// ── Shared adblock engine (lazy-initialised once) ───────────────────────
#[allow(clippy::type_complexity)]
static ADBLOCK: std::sync::OnceLock<Mutex<AdBlockEngine>> = std::sync::OnceLock::new();

fn adblock() -> &'static Mutex<AdBlockEngine> {
    ADBLOCK.get_or_init(|| Mutex::new(AdBlockEngine::init()))
}

// ═══════════════════════════════════════════════════════════════════════
// Browser window
// ═══════════════════════════════════════════════════════════════════════

pub async fn open_or_navigate(
    app:   &AppHandle,
    url:   &str,
    state: SharedState,
) -> Result<(), Box<dyn std::error::Error>> {
    let settings = { state.lock().settings.clone() };

    // 1. HTTPS upgrade: HTTP → HTTPS
    let url = upgrade_https(url);

    // 2. Privacy: strip tracking params
    let url = privacy::scrub_url(&url, &settings);

    // 3. Adblock: domain + pattern check
    if adblock().lock().should_block(&url, None) {
        log::info!("[Adblock] Blocked: {}", url);
        { state.lock().privacy_stats.trackers_blocked += 1; }
        return Ok(());
    }

    // 4. Record in history (title will be updated later by page events)
    if let Ok(hs) = history::HistoryStore::open() {
        let _ = hs.add_visit(&url, "");
    }

    // 5. Build init script: fingerprint protection + GM_ API + cosmetic CSS
    let init = build_init_script(&url, &adblock().lock().cosmetic_injection_js());

    let existing = app.get_webview_window("browser");
    if let Some(win) = existing {
        win.navigate(url.parse()?)?;
    } else {
        WebviewWindowBuilder::new(app, "browser", WebviewUrl::External(url.parse()?))
            .title("BM-Aegis Browser")
            .inner_size(1100.0, 760.0)
            .min_inner_size(600.0, 400.0)
            .resizable(true)
            .decorations(true)
            .initialization_script(&init)
            .build()?;
        state.lock().browser_open = true;
    }

    Ok(())
}

fn build_init_script(initial_url: &str, cosmetic_js: &str) -> String {
    format!(
        r#"
// BM-Aegis Browser Runtime — injected before every page
(function() {{
  window.__BM_INITIAL_URL__ = {url_json};

  // 1. Fingerprint protection
  {fp}

  // 2. GM_ API + userscript loader
  {gm}

  // 3. Cosmetic ad filter
  {cosmetic}

  // 4. Navigation tracking
  function _bmReport(url) {{
    try {{ window.__TAURI_INTERNALS__.invoke('browser_navigate', {{ url }}).catch(()=>{{}}); }} catch(e) {{}}
  }}
  const _pps = history.pushState.bind(history);
  history.pushState = function() {{ _pps.apply(history, arguments); _bmReport(location.href); }};
  window.addEventListener('popstate', () => _bmReport(location.href));

}})();
"#,
        url_json = serde_json::to_string(initial_url).unwrap_or_default(),
        fp       = FINGERPRINT_JS,
        gm       = GM_API_JS,
        cosmetic = cosmetic_js,
    )
}

/// Force HTTP → HTTPS. Returns upgraded URL or original if already HTTPS/other.
fn upgrade_https(url: &str) -> String {
    if url.starts_with("http://") && !url.starts_with("http://localhost") {
        let upgraded = url.replacen("http://", "https://", 1);
        log::info!("[Browser] HTTPS upgrade: {} → {}", url, upgraded);
        return upgraded;
    }
    url.to_string()
}

// ═══════════════════════════════════════════════════════════════════════
// Adblock public API
// ═══════════════════════════════════════════════════════════════════════

pub fn adblock_should_block(url: &str, origin: Option<&str>) -> bool {
    adblock().lock().should_block(url, origin)
}

pub fn adblock_stats() -> adblock::AdBlockStats {
    adblock().lock().stats.clone()
}

// ═══════════════════════════════════════════════════════════════════════
// GM_ xmlhttpRequest proxy (CORS-free)
// ═══════════════════════════════════════════════════════════════════════

pub async fn make_gm_request(
    details: XhrDetails,
) -> Result<XhrResponse, Box<dyn std::error::Error + Send + Sync>> {
    use reqwest::{Client, Method, header::{HeaderMap, HeaderName, HeaderValue}};
    use std::str::FromStr;

    let client = Client::builder()
        .timeout(std::time::Duration::from_millis(details.timeout.unwrap_or(30_000)))
        .build()?;

    let method = Method::from_str(&details.method).unwrap_or(Method::GET);
    let mut req = client.request(method, &details.url);

    let mut header_map = HeaderMap::new();
    for (k, v) in &details.headers {
        if let (Ok(name), Ok(val)) = (HeaderName::from_str(k), HeaderValue::from_str(v)) {
            header_map.insert(name, val);
        }
    }
    req = req.headers(header_map);
    if let Some(data) = &details.data { req = req.body(data.clone()); }

    let resp = req.send().await?;
    let status      = resp.status().as_u16();
    let status_text = resp.status().canonical_reason().unwrap_or("").to_string();
    let final_url   = resp.url().to_string();
    let response_headers = resp.headers().iter()
        .map(|(k, v)| format!("{}: {}", k, v.to_str().unwrap_or("")))
        .collect::<Vec<_>>().join("\r\n");
    let response_text = resp.text().await?;

    Ok(XhrResponse { status, status_text, response_text, response_headers, final_url })
}

// Re-export so commands.rs can reference these easily
pub use adblock::AdBlockStats;
pub use history::HistoryStore;
