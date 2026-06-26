/**
 * tag-rename.js
 * CodeMirror 6 extension that keeps paired HTML open/close tags in sync.
 *
 * When the cursor is inside a tag name and you edit it, the matching
 * partner tag is updated atomically in the same transaction.
 *
 * Works for:
 *   <div|>  → edit → <section>…</section>   (open edited, close follows)
 *   </div|> → edit → <section>…</section>   (close edited, open follows)
 *
 * Does NOT fire for:
 *   - Void elements (no close tag to update)
 *   - Multi-selection edits (too ambiguous)
 *   - Changes triggered by the extension itself (no recursion)
 */

import { EditorView, ViewPlugin } from "@codemirror/view";
import { Transaction } from "@codemirror/state";

const VOID = new Set([
  "area","base","br","col","embed","hr","img","input",
  "link","meta","param","source","track","wbr",
]);

// State: are we inside a paired rename right now? (prevent recursion)
let _syncing = false;

/**
 * Detect if `pos` is inside an HTML tag name in `doc`.
 * Returns { tagName, nameFrom, nameTo, isClose } or null.
 */
function detectTagNameAt(doc, pos) {
  const text  = doc.toString();
  const len   = text.length;

  // Scan backwards to find '<' (opening bracket)
  let i = pos - 1;
  while (i >= 0 && text[i] !== "<" && text[i] !== ">") i--;
  if (i < 0 || text[i] !== "<") return null;

  const bracketPos = i;
  const isClose    = text[bracketPos + 1] === "/";
  const nameStart  = bracketPos + (isClose ? 2 : 1);

  // Collect tag name chars forward from nameStart
  let j = nameStart;
  while (j < len && /[\w:-]/.test(text[j])) j++;
  const tagName = text.slice(nameStart, j).toLowerCase();

  if (!tagName) return null;
  if (nameStart > pos || j < pos) return null; // cursor not inside name

  return { tagName, nameFrom: nameStart, nameTo: j, isClose };
}

/**
 * Find the matching partner of an open/close tag starting at `bracketPos`.
 * Returns { nameFrom, nameTo } of the partner or null.
 */
function findPartner(text, nameFrom, nameTo, isClose) {
  const tagName = text.slice(nameFrom, nameTo);
  if (VOID.has(tagName.toLowerCase())) return null;

  const openPat  = new RegExp(`<${escapeRe(tagName)}(?=[\\s>])`, "gi");
  const closePat = new RegExp(`<\\/${escapeRe(tagName)}\\s*>`, "gi");

  if (!isClose) {
    // Cursor is in opening tag → find next matching closing tag
    closePat.lastIndex = nameTo;
    let depth = 0;
    let match;

    // Scan forward: count opens and closes
    openPat.lastIndex = nameTo;
    const combined = new RegExp(
      `<\\/?${escapeRe(tagName)}(?=[\\s>\\/])`, "gi"
    );
    combined.lastIndex = nameTo;

    while ((match = combined.exec(text)) !== null) {
      const isThisClose = text[match.index + 1] === "/";
      if (!isThisClose) depth++;
      else if (depth === 0) {
        // Found the matching close
        const closeNameFrom = match.index + 2;
        const closeNameTo   = closeNameFrom + tagName.length;
        return { nameFrom: closeNameFrom, nameTo: closeNameTo };
      } else depth--;
    }
  } else {
    // Cursor is in closing tag → find the matching opening tag (backwards)
    const closeNameFrom = nameFrom;
    let depth  = 0;
    let match;
    let pos    = closeNameFrom;

    const combined = new RegExp(
      `<\\/?${escapeRe(tagName)}(?=[\\s>\\/])`, "gi"
    );
    // Collect all matches before cursor
    const allMatches = [];
    combined.lastIndex = 0;
    while ((match = combined.exec(text)) !== null) {
      if (match.index >= closeNameFrom) break;
      allMatches.push({ index: match.index, close: text[match.index + 1] === "/" });
    }

    // Walk backwards to find the opener
    for (let k = allMatches.length - 1; k >= 0; k--) {
      const m = allMatches[k];
      if (m.close) depth++;
      else if (depth === 0) {
        const openNameFrom = m.index + 1;
        const openNameTo   = openNameFrom + tagName.length;
        return { nameFrom: openNameFrom, nameTo: openNameTo };
      } else depth--;
    }
  }

  return null;
}

function escapeRe(s) {
  return s.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");
}

/**
 * The ViewPlugin that watches transactions and mirrors tag renames.
 */
const tagRenamePlugin = ViewPlugin.fromClass(class {
  update(update) {
    if (_syncing) return;
    if (!update.docChanged) return;
    if (update.transactions.length === 0) return;

    // Only react to user input transactions (not programmatic)
    const tr = update.transactions[0];
    if (!tr.isUserEvent("input") && !tr.isUserEvent("delete")) return;

    const sel   = update.state.selection.main;
    const doc   = update.state.doc;
    const text  = doc.toString();

    const info = detectTagNameAt(doc, sel.head);
    if (!info) return;
    if (VOID.has(info.tagName)) return;

    const partner = findPartner(text, info.nameFrom, info.nameTo, info.isClose);
    if (!partner) return;

    // The new tag name is whatever is currently at info.nameFrom..info.nameTo
    const newName = text.slice(info.nameFrom, info.nameTo);
    const oldPartnerName = text.slice(partner.nameFrom, partner.nameTo);

    if (newName === oldPartnerName) return; // already in sync

    _syncing = true;
    update.view.dispatch({
      changes: {
        from:   partner.nameFrom,
        to:     partner.nameTo,
        insert: newName,
      },
      annotations: Transaction.userEvent.of("tag-rename"),
    });
    _syncing = false;
  }
});

export { tagRenamePlugin };
