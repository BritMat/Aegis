/**
 * shortcuts.js — Keyboard shortcut reference panel.
 * Ctrl+/ (or Ctrl+?) toggles the panel.
 * Escape closes it.
 */

const SHORTCUTS = {
  "Editor": [
    { keys: ["Ctrl", "S"],       label: "Save current file"            },
    { keys: ["Ctrl", "Shift", "S"], label: "Save As…"                  },
    { keys: ["Ctrl", "O"],       label: "Open file"                    },
    { keys: ["Ctrl", "Shift", "F"], label: "Format document (HTML)"    },
    { keys: ["Tab"],             label: "Emmet expand abbreviation"    },
    { keys: ["Ctrl", "Z"],       label: "Undo"                         },
    { keys: ["Ctrl", "Y"],       label: "Redo"                         },
    { keys: ["Ctrl", "F"],       label: "Find in file"                 },
    { keys: ["Ctrl", "H"],       label: "Find & Replace"               },
    { keys: ["Ctrl", "G"],       label: "Go to line"                   },
    { keys: ["Ctrl", "D"],       label: "Select next occurrence"       },
    { keys: ["Alt", "Click"],    label: "Add cursor (multi-cursor)"    },
    { keys: ["Ctrl", "["],       label: "Fold code"                    },
    { keys: ["Ctrl", "]"],       label: "Unfold code"                  },
  ],
  "Browser": [
    { keys: ["Enter"],           label: "Navigate to URL / search"     },
    { keys: ["Escape"],          label: "Cancel URL edit"              },
    { keys: ["Alt", "←"],        label: "Back"                         },
    { keys: ["Alt", "→"],        label: "Forward"                      },
    { keys: ["Ctrl", "R"],       label: "Reload"                       },
    { keys: ["Ctrl", "D"],       label: "Bookmark current page"        },
    { keys: ["Ctrl", "H"],       label: "Open history"                 },
    { keys: ["Ctrl", "B"],       label: "Open bookmarks"               },
  ],
  "Search": [
    { keys: ["Enter"],           label: "Run search"                   },
    { keys: ["Ctrl", "L"],       label: "Focus search input"           },
  ],
  "Application": [
    { keys: ["Ctrl", "/"],       label: "Show / hide this panel"       },
    { keys: ["Escape"],          label: "Close overlay / panel"        },
    { keys: ["Ctrl", "1"],       label: "Switch to Editor tab"         },
    { keys: ["Ctrl", "2"],       label: "Switch to Browser tab"        },
    { keys: ["Ctrl", "3"],       label: "Switch to Search tab"         },
    { keys: ["Ctrl", "4"],       label: "Switch to Settings tab"       },
    { keys: ["F11"],             label: "Toggle fullscreen"            },
  ],
  "Emmet (in Editor, press Tab)": [
    { keys: ["div"],             label: "<div></div>"                   },
    { keys: ["div.cls#id"],      label: "<div class=\"cls\" id=\"id\">"  },
    { keys: ["ul>li*3"],         label: "<ul> with 3 <li> items"       },
    { keys: ["a[href=\"#\"]"],   label: "<a href=\"#\"></a>"            },
    { keys: ["form>input+button"], label: "Form with input and button" },
    { keys: ["!"],               label: "HTML5 document boilerplate"   },
    { keys: ["lorem"],           label: "Lorem ipsum paragraph"        },
    { keys: ["lorem30"],         label: "Lorem ipsum (30 words)"       },
  ],
};

let _panel = null;

function buildPanel() {
  const el = document.createElement("div");
  el.id = "shortcut-panel";
  el.className = "hidden";

  const sections = Object.entries(SHORTCUTS).map(([title, rows]) => `
    <div class="shortcut-section">
      <h3>${title}</h3>
      ${rows.map(r => `
        <div class="shortcut-row">
          <span>${r.label}</span>
          <span class="shortcut-keys">
            ${r.keys.map(k => `<kbd class="kbd">${k}</kbd>`).join(' + ')}
          </span>
        </div>`).join("")}
    </div>`).join("");

  el.innerHTML = `
    <div class="shortcut-card">
      <h2>
        Keyboard Shortcuts
        <button class="shortcut-close" id="sc-close" title="Close">✕</button>
      </h2>
      ${sections}
      <p style="font-size:11px;color:var(--text-3);margin-top:12px;text-align:center;">
        Press <kbd class="kbd">Ctrl+/</kbd> or <kbd class="kbd">Esc</kbd> to close
      </p>
    </div>`;

  el.addEventListener("click", e => {
    if (e.target === el) hidePanel();
  });
  el.querySelector("#sc-close").addEventListener("click", hidePanel);

  document.body.appendChild(el);
  return el;
}

export function showPanel() {
  if (!_panel) _panel = buildPanel();
  _panel.classList.remove("hidden");
}

export function hidePanel() {
  _panel?.classList.add("hidden");
}

export function togglePanel() {
  if (!_panel) _panel = buildPanel();
  _panel.classList.toggle("hidden");
}

/** Call once from main.js to wire global shortcuts. */
export function initShortcuts(tabSwitcher) {
  document.addEventListener("keydown", e => {
    // Ctrl+/ — keyboard shortcut reference
    if ((e.ctrlKey || e.metaKey) && (e.key === "/" || e.key === "?")) {
      e.preventDefault();
      togglePanel();
      return;
    }

    // Escape — close any open overlay or this panel
    if (e.key === "Escape") {
      if (_panel && !_panel.classList.contains("hidden")) {
        hidePanel();
        return;
      }
    }

    // Ctrl+1..4 — switch tabs
    if ((e.ctrlKey || e.metaKey) && e.key >= "1" && e.key <= "4") {
      const tabs = ["editor", "browser", "search", "settings"];
      const idx  = parseInt(e.key, 10) - 1;
      if (tabs[idx]) { e.preventDefault(); tabSwitcher(tabs[idx]); }
      return;
    }
  });
}
