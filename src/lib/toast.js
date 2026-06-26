/**
 * toast.js — Lightweight toast notification system.
 * No dependencies. Injects its own styles on first use.
 * Usage:
 *   import { toast } from "./toast.js";
 *   toast("File saved");
 *   toast("Error saving file", "error");
 *   toast("Indexing complete — 142 files indexed", "success", 4000);
 */

let _styleInjected = false;

function injectStyles() {
  if (_styleInjected) return;
  _styleInjected = true;
  const style = document.createElement("style");
  style.textContent = `
    #bm-toast-container {
      position: fixed;
      bottom: 32px;
      right: 20px;
      z-index: 9999;
      display: flex;
      flex-direction: column-reverse;
      gap: 8px;
      pointer-events: none;
    }
    .bm-toast {
      display: flex;
      align-items: center;
      gap: 10px;
      padding: 10px 16px;
      border-radius: 6px;
      font-size: 12.5px;
      font-family: -apple-system, BlinkMacSystemFont, 'Segoe UI', system-ui, sans-serif;
      color: #fff;
      max-width: 360px;
      min-width: 200px;
      pointer-events: all;
      cursor: default;
      border: 1px solid rgba(255,255,255,.1);
      opacity: 0;
      transform: translateY(12px);
      transition: opacity .18s ease, transform .18s ease;
      box-shadow: 0 4px 16px rgba(0,0,0,.4);
    }
    .bm-toast.show {
      opacity: 1;
      transform: translateY(0);
    }
    .bm-toast.hide {
      opacity: 0;
      transform: translateY(8px);
    }
    .bm-toast.info    { background: #1e2235; border-color: #3d4470; }
    .bm-toast.success { background: #14291f; border-color: rgba(34,197,94,.4); }
    .bm-toast.error   { background: #291414; border-color: rgba(239,68,68,.4); }
    .bm-toast.warning { background: #29231a; border-color: rgba(234,179,8,.4); }
    .bm-toast-icon    { font-size: 15px; flex-shrink: 0; }
    .bm-toast-msg     { flex: 1; line-height: 1.5; }
    .bm-toast-close   {
      background: none; border: none; color: rgba(255,255,255,.4);
      cursor: pointer; font-size: 14px; padding: 0; flex-shrink: 0;
      line-height: 1; transition: color .1s;
    }
    .bm-toast-close:hover { color: rgba(255,255,255,.8); }
  `;
  document.head.appendChild(style);
}

function getContainer() {
  let c = document.getElementById("bm-toast-container");
  if (!c) {
    c = document.createElement("div");
    c.id = "bm-toast-container";
    document.body.appendChild(c);
  }
  return c;
}

const ICONS = {
  info:    "ℹ",
  success: "✓",
  error:   "✕",
  warning: "⚠",
};

/**
 * Show a toast notification.
 * @param {string} message   — Text to display
 * @param {"info"|"success"|"error"|"warning"} type
 * @param {number} duration  — ms before auto-dismiss (0 = manual only)
 * @returns {function} dismiss — call to dismiss immediately
 */
export function toast(message, type = "info", duration = 3000) {
  injectStyles();
  const container = getContainer();

  const el = document.createElement("div");
  el.className = `bm-toast ${type}`;
  el.innerHTML = `
    <span class="bm-toast-icon">${ICONS[type] ?? "ℹ"}</span>
    <span class="bm-toast-msg">${String(message).replace(/</g,"&lt;")}</span>
    <button class="bm-toast-close" title="Dismiss">✕</button>
  `;

  container.appendChild(el);

  // Trigger enter animation
  requestAnimationFrame(() => {
    requestAnimationFrame(() => el.classList.add("show"));
  });

  function dismiss() {
    el.classList.remove("show");
    el.classList.add("hide");
    el.addEventListener("transitionend", () => el.remove(), { once: true });
    // Safety remove if transition doesn't fire
    setTimeout(() => el.remove(), 400);
  }

  el.querySelector(".bm-toast-close").addEventListener("click", dismiss);

  if (duration > 0) {
    setTimeout(dismiss, duration);
  }

  return dismiss;
}

/** Convenience shorthand helpers */
export const toastSuccess = (msg, ms = 3000) => toast(msg, "success", ms);
export const toastError   = (msg, ms = 5000) => toast(msg, "error",   ms);
export const toastWarning = (msg, ms = 4000) => toast(msg, "warning", ms);
export const toastInfo    = (msg, ms = 3000) => toast(msg, "info",    ms);
