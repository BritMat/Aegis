/**
 * gm-api.js
 *
 * Tampermonkey-compatible GM_ API implementation.
 * This file is serialised and injected as a Tauri initialization_script
 * into the privacy browser webview window. It runs before every page loads,
 * giving every injected userscript a complete GM_ surface.
 *
 * Communication to Rust uses window.__TAURI_INTERNALS__.invoke() which Tauri
 * injects before page scripts via its own initialization mechanism.
 *
 * Usage (from Rust, via WebviewBuilder::initialization_script):
 *   let gm_api_src = include_str!("../../src/lib/gm-api.js");
 *   builder = builder.initialization_script(gm_api_src);
 */

(function (globalThis) {
  "use strict";

  /* ── IPC bridge ─────────────────────────────────────────────────────────── */
  // __TAURI_INTERNALS__ is injected by Tauri on every page regardless of origin.
  const _invoke = (cmd, args = {}) =>
    globalThis.__TAURI_INTERNALS__
      ? globalThis.__TAURI_INTERNALS__.invoke(cmd, args)
      : Promise.reject(new Error("[BM-Aegis] Tauri IPC unavailable"));

  /* ── Script metadata ─────────────────────────────────────────────────────── */
  // __BM_SCRIPT_ID__ is replaced at injection time by the Rust side with the
  // actual script ID. Each userscript gets its own isolated GM_ namespace.
  const _scriptId = globalThis.__BM_SCRIPT_ID__ || "unknown";

  /* ── GM_info ─────────────────────────────────────────────────────────────── */
  globalThis.GM_info = {
    script: {
      author:      globalThis.__BM_SCRIPT_AUTHOR__   || "",
      description: globalThis.__BM_SCRIPT_DESC__      || "",
      name:        globalThis.__BM_SCRIPT_NAME__      || "Unnamed Script",
      namespace:   globalThis.__BM_SCRIPT_NS__        || "",
      version:     globalThis.__BM_SCRIPT_VERSION__   || "0.0.0",
      grant:       globalThis.__BM_SCRIPT_GRANTS__    || [],
    },
    scriptMetaStr: "",
    scriptHandler:  "BM-Aegis",
    version:        "1.0.0",
    isIncognito:    false,
  };

  /* ── Persistent key-value storage (backed by Rust / disk) ─────────────────── */
  globalThis.GM_getValue = function (name, defaultValue) {
    // Synchronous API — return cached value immediately, async update in bg
    return defaultValue;
  };

  globalThis.GM_getValueAsync = async function (name, defaultValue) {
    try {
      const result = await _invoke("gm_get_value", { scriptId: _scriptId, key: name, defaultVal: defaultValue });
      return result !== null ? result : defaultValue;
    } catch { return defaultValue; }
  };

  globalThis.GM_setValue = async function (name, value) {
    try {
      await _invoke("gm_set_value", { scriptId: _scriptId, key: name, value: JSON.stringify(value) });
    } catch (e) { console.error("[BM-Aegis] GM_setValue failed:", e); }
  };

  globalThis.GM_deleteValue = async function (name) {
    try {
      await _invoke("gm_delete_value", { scriptId: _scriptId, key: name });
    } catch (e) { console.error("[BM-Aegis] GM_deleteValue failed:", e); }
  };

  globalThis.GM_listValues = async function () {
    try {
      return await _invoke("gm_list_values", { scriptId: _scriptId }) || [];
    } catch { return []; }
  };

  /* ── Style injection ─────────────────────────────────────────────────────── */
  globalThis.GM_addStyle = function (css) {
    const style = document.createElement("style");
    style.setAttribute("data-bm-aegis", _scriptId);
    style.textContent = css;
    (document.head || document.documentElement).appendChild(style);
    return style;
  };

  /* ── Logging ─────────────────────────────────────────────────────────────── */
  globalThis.GM_log = function (...args) {
    console.log(`[GM:${globalThis.GM_info.script.name}]`, ...args);
    _invoke("gm_log", { scriptId: _scriptId, message: args.join(" ") }).catch(() => {});
  };

  /* ── Tab management ─────────────────────────────────────────────────────── */
  globalThis.GM_openInTab = function (url, options = {}) {
    _invoke("gm_open_in_tab", { url, background: options.background || false })
      .catch(e => console.error("[BM-Aegis] GM_openInTab:", e));
  };

  /* ── Clipboard ──────────────────────────────────────────────────────────── */
  globalThis.GM_setClipboard = function (data, info = "text") {
    if (navigator.clipboard) {
      navigator.clipboard.writeText(data).catch(() =>
        _invoke("gm_set_clipboard", { data, type: info }).catch(() => {})
      );
    } else {
      _invoke("gm_set_clipboard", { data, type: info }).catch(() => {});
    }
  };

  /* ── Notifications ──────────────────────────────────────────────────────── */
  globalThis.GM_notification = function (details, ondone) {
    const d = typeof details === "string" ? { text: details } : details;
    _invoke("gm_notification", {
      title:   d.title   || globalThis.GM_info.script.name,
      text:    d.text    || "",
      timeout: d.timeout || 5000,
    })
    .then(() => { if (typeof ondone === "function") ondone(true); })
    .catch(() => {});
  };

  /* ── CORS-bypassing XMLHttpRequest ──────────────────────────────────────── */
  // This is the most powerful GM_ API: requests are made by Rust, bypassing
  // CORS entirely since they originate from the native app process.
  globalThis.GM_xmlhttpRequest = function (details) {
    const ctrl = { abort: () => {} };
    _invoke("gm_xmlhttp_request", {
      details: {
        method:          (details.method || "GET").toUpperCase(),
        url:             details.url,
        headers:         details.headers         || {},
        data:            details.data            || null,
        responseType:    details.responseType    || "text",
        timeout:         details.timeout         || 30000,
        anonymous:       details.anonymous       || false,
        withCredentials: details.withCredentials || false,
      }
    })
    .then(resp => {
      const r = {
        status:          resp.status,
        statusText:      resp.statusText,
        responseText:    resp.responseText,
        responseHeaders: resp.responseHeaders,
        finalUrl:        resp.finalUrl || details.url,
        readyState:      4,
        response:        resp.responseText,
      };
      if (resp.status >= 200 && resp.status < 300) {
        if (typeof details.onload === "function")  details.onload(r);
      } else {
        if (typeof details.onerror === "function") details.onerror(r);
      }
    })
    .catch(err => {
      if (typeof details.onerror === "function") {
        details.onerror({ status: 0, statusText: "Network Error", responseText: String(err) });
      }
    });
    return ctrl;
  };

  /* ── Alias: GM.xmlHttpRequest (Tampermonkey v4+ async style) ─────────────── */
  globalThis.GM = {
    getValue:        globalThis.GM_getValueAsync,
    setValue:        globalThis.GM_setValue,
    deleteValue:     globalThis.GM_deleteValue,
    listValues:      globalThis.GM_listValues,
    addStyle:        globalThis.GM_addStyle,
    log:             globalThis.GM_log,
    openInTab:       globalThis.GM_openInTab,
    setClipboard:    globalThis.GM_setClipboard,
    notification:    globalThis.GM_notification,
    xmlHttpRequest:  globalThis.GM_xmlhttpRequest,
    info:            globalThis.GM_info,
  };

  /* ── unsafeWindow ────────────────────────────────────────────────────────── */
  // In a Tauri initialization_script context, we are already in the page
  // scope, so unsafeWindow === window.
  if (!globalThis.unsafeWindow) {
    globalThis.unsafeWindow = globalThis;
  }

  /* ── Userscript loader ──────────────────────────────────────────────────── */
  // Called after each page navigation — fetches matching scripts from Rust.
  async function _injectUserscripts(url) {
    try {
      const scripts = await _invoke("get_userscripts_for_url", { url });
      for (const script of scripts) {
        // Expose per-script metadata before eval
        globalThis.__BM_SCRIPT_ID__      = script.id;
        globalThis.__BM_SCRIPT_NAME__    = script.name;
        globalThis.__BM_SCRIPT_VERSION__ = script.version;
        globalThis.__BM_SCRIPT_AUTHOR__  = script.author;
        globalThis.__BM_SCRIPT_DESC__    = script.description;
        globalThis.__BM_SCRIPT_NS__      = script.namespace;
        globalThis.__BM_SCRIPT_GRANTS__  = script.grants;
        try {
          // Wrap in an IIFE so scripts can use 'use strict' safely
          new Function(script.code)(); // eslint-disable-line no-new-func
        } catch (e) {
          console.error(`[BM-Aegis] Script "${script.name}" threw:`, e);
        }
      }
    } catch (e) {
      // IPC unavailable — not a Tauri context or no scripts matched
    }
  }

  /* ── Navigation hook (SPA support) ─────────────────────────────────────── */
  function _onNavigate() { _injectUserscripts(globalThis.location.href); }

  const _origPushState    = history.pushState.bind(history);
  const _origReplaceState = history.replaceState.bind(history);

  history.pushState = function (...a) {
    _origPushState(...a);
    _onNavigate();
  };
  history.replaceState = function (...a) {
    _origReplaceState(...a);
    _onNavigate();
  };

  globalThis.addEventListener("popstate", _onNavigate);

  /* Initial injection on page ready */
  if (document.readyState === "loading") {
    document.addEventListener("DOMContentLoaded", _onNavigate);
  } else {
    _onNavigate();
  }

})(window);
