/**
 * main.js — BM-Aegis bootstrap
 * Tab routing, window controls, global keyboard shortcuts.
 */

import { mountEditor }                  from "./components/editor.js";
import { mountBrowser, unmountBrowser } from "./components/browser.js";
import { mountSearch }                  from "./components/search.js";
import { mountSettings }                from "./components/settings.js";
import { initShortcuts }                from "./lib/shortcuts.js";

/* ── DOM refs ─────────────────────────────────────────────────── */
const $sidebar  = document.getElementById("sidebar");
const statusEls = {
  pos:     document.getElementById("status-pos"),
  lang:    document.getElementById("status-lang"),
  lint:    document.getElementById("status-lint"),
  lintDot: document.getElementById("status-lint-dot"),
  file:    document.getElementById("status-file"),
};

/* ── Tab routing ──────────────────────────────────────────────── */
let _activeTab = "";
const _mounted = {};

function activateTab(name) {
  if (_activeTab === "browser" && name !== "browser") unmountBrowser();
  _activeTab = name;

  document.querySelectorAll(".tab").forEach(t =>
    t.classList.toggle("active", t.dataset.tab === name)
  );
  document.querySelectorAll(".panel").forEach(p =>
    p.classList.toggle("active", p.id === `panel-${name}`)
  );

  $sidebar.innerHTML = "";

  const panel = document.getElementById(`panel-${name}`);
  if (!panel) return;

  if (!_mounted[name]) {
    _mounted[name] = true;
    mountPanel(name, panel);
  } else if (name === "browser") {
    mountBrowser($sidebar, panel);
  }
}

function mountPanel(name, panel) {
  switch (name) {
    case "editor":   mountEditor($sidebar, panel, statusEls); break;
    case "browser":  mountBrowser($sidebar, panel);           break;
    case "search":   mountSearch($sidebar, panel);            break;
    case "settings": mountSettings($sidebar, panel);          break;
  }
}

document.querySelectorAll(".tab").forEach(t =>
  t.addEventListener("click", () => activateTab(t.dataset.tab))
);

/* ── Window controls (Tauri v2) ───────────────────────────────── */
async function wireWindowControls() {
  try {
    const { getCurrentWindow } = await import("@tauri-apps/api/window");
    const win = getCurrentWindow();
    document.getElementById("btn-minimize")
      ?.addEventListener("click", () => win.minimize());
    document.getElementById("btn-maximize")
      ?.addEventListener("click", async () =>
        (await win.isMaximized()) ? win.unmaximize() : win.maximize()
      );
    document.getElementById("btn-close")
      ?.addEventListener("click", () => win.close());
  } catch {
    document.getElementById("btn-close")
      ?.addEventListener("click", () => window.close?.());
  }
}

/* ── Init ─────────────────────────────────────────────────────── */
wireWindowControls();
initShortcuts(activateTab);  // wire Ctrl+/ and Ctrl+1..4
activateTab("editor");
