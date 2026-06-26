/**
 * codemirror-setup.js
 * CodeMirror 6 — BM Dark theme, Rust linter, Rust autocomplete,
 * Emmet Tab expansion, tag auto-rename.
 */

import { EditorState, Compartment }           from "@codemirror/state";
import {
  EditorView, keymap, lineNumbers, highlightActiveLineGutter,
  highlightSpecialChars, drawSelection, dropCursor,
  rectangularSelection, crosshairCursor, highlightActiveLine,
} from "@codemirror/view";
import { defaultKeymap, history, historyKeymap, indentWithTab } from "@codemirror/commands";
import { searchKeymap, highlightSelectionMatches }              from "@codemirror/search";
import { autocompletion, completionKeymap, closeBrackets, closeBracketsKeymap } from "@codemirror/autocomplete";
import { lintGutter, lintKeymap, setDiagnostics }              from "@codemirror/lint";
import { html }       from "@codemirror/lang-html";
import { css }        from "@codemirror/lang-css";
import { javascript } from "@codemirror/lang-javascript";
import {
  syntaxHighlighting, HighlightStyle,
  foldGutter, foldKeymap, indentOnInput, bracketMatching,
} from "@codemirror/language";
import { tags } from "@lezer/highlight";

import { lintHtml, getCompletions } from "./tauri-bridge.js";
import { emmetKeymap }              from "./emmet.js";
import { tagRenamePlugin }          from "./tag-rename.js";

// ── Colour palette ────────────────────────────────────────────────────────
const C = {
  bg:"#12141f", panel:"#181b2e", card:"#1e2235", hover:"#252840", active:"#2a2f50",
  border:"#2d3155", borderHi:"#3d4470",
  text:"#c9d1e3", text2:"#8b91a8", text3:"#555e7a",
  accent:"#7c3aed", accent2:"#06b6d4",
  green:"#a6e3a1", yellow:"#f9e2af", orange:"#fab387",
  red:"#f38ba8", mauve:"#c084fc", cyan:"#67e8f9", blue:"#89b4fa",
  comment:"#495170",
};

// ── EditorView theme ──────────────────────────────────────────────────────
const bmTheme = EditorView.theme({
  "&":                        { backgroundColor: C.bg, color: C.text, height: "100%" },
  ".cm-content":              { caretColor: C.accent2, padding: "0 0 0 3px" },
  ".cm-cursor,.cm-dropCursor":{ borderLeftColor: C.accent2, borderLeftWidth: "2px" },
  "&.cm-focused .cm-selectionBackground,.cm-selectionBackground,::selection":
                              { backgroundColor: "rgba(124,58,237,.22)" },
  ".cm-gutters":              { backgroundColor: C.panel, color: C.text3, border: "none", borderRight: `1px solid ${C.border}`, minWidth: "46px" },
  ".cm-lineNumbers .cm-gutterElement": { padding: "0 10px 0 6px" },
  ".cm-activeLineGutter":     { backgroundColor: C.card, color: C.text2 },
  ".cm-activeLine":           { backgroundColor: "rgba(124,58,237,.05)" },
  ".cm-foldPlaceholder":      { backgroundColor: C.active, border: `1px solid ${C.borderHi}`, color: C.mauve, borderRadius: "2px", padding: "0 5px" },
  ".cm-tooltip":              { backgroundColor: `${C.card} !important`, border: `1px solid ${C.borderHi} !important`, borderRadius: "5px !important" },
  ".cm-tooltip-autocomplete > ul > li":             { padding: "4px 10px !important", color: `${C.text2} !important` },
  ".cm-tooltip-autocomplete > ul > li[aria-selected]": { backgroundColor: `${C.active} !important`, color: `${C.text} !important` },
  ".cm-completionLabel":      { color: C.text },
  ".cm-completionDetail":     { color: C.text3, fontSize: "11px", marginLeft: "8px" },
  ".cm-completionMatchedText":{ color: `${C.cyan} !important`, fontWeight: "600", textDecoration: "none !important" },
  ".cm-search":               { backgroundColor: `${C.card} !important`, borderTop: `1px solid ${C.border} !important` },
  ".cm-search input":         { backgroundColor: `${C.hover} !important`, border: `1px solid ${C.borderHi} !important`, color: `${C.text} !important`, borderRadius: "3px", padding: "2px 8px" },
  ".cm-search button":        { backgroundColor: `${C.active} !important`, border: `1px solid ${C.borderHi} !important`, color: `${C.text2} !important`, borderRadius: "3px", padding: "2px 8px", cursor: "pointer" },
}, { dark: true });

// ── Syntax highlight style ─────────────────────────────────────────────────
const bmHighlight = HighlightStyle.define([
  { tag: tags.comment,                  color: C.comment,  fontStyle: "italic" },
  { tag: tags.lineComment,              color: C.comment,  fontStyle: "italic" },
  { tag: tags.blockComment,             color: C.comment,  fontStyle: "italic" },
  { tag: tags.keyword,                  color: C.mauve },
  { tag: tags.controlKeyword,           color: C.red },
  { tag: tags.definitionKeyword,        color: C.mauve },
  { tag: tags.moduleKeyword,            color: C.mauve },
  { tag: tags.string,                   color: C.green },
  { tag: tags.special(tags.string),     color: C.green },
  { tag: tags.regexp,                   color: C.orange },
  { tag: tags.number,                   color: C.orange },
  { tag: tags.bool,                     color: C.orange },
  { tag: tags.null,                     color: C.orange },
  { tag: tags.atom,                     color: C.orange },
  { tag: tags.typeName,                 color: C.yellow },
  { tag: tags.className,                color: C.yellow },
  { tag: tags.propertyName,             color: C.blue },
  { tag: tags.variableName,             color: C.text },
  { tag: tags.definition(tags.variableName), color: C.blue },
  { tag: tags.function(tags.variableName),   color: C.blue },
  { tag: tags.function(tags.propertyName),   color: C.blue },
  { tag: tags.namespace,                color: C.cyan },
  { tag: tags.operator,                 color: C.cyan },
  { tag: tags.punctuation,              color: C.text2 },
  { tag: tags.angleBracket,             color: C.text3 },
  { tag: tags.tagName,                  color: C.red,   fontWeight: "600" },
  { tag: tags.attributeName,            color: C.yellow },
  { tag: tags.attributeValue,           color: C.green },
  { tag: tags.docType,                  color: C.text3 },
  { tag: tags.meta,                     color: C.mauve },
  { tag: tags.link,                     color: C.cyan,  textDecoration: "underline" },
  { tag: tags.invalid,                  color: C.red,   textDecoration: "underline" },
]);

// ── Compartments ──────────────────────────────────────────────────────────
export const languageCompartment  = new Compartment();
export const wordWrapCompartment  = new Compartment();
export const readonlyCompartment  = new Compartment();

// ── Language detection ────────────────────────────────────────────────────
export function languageForFile(filename) {
  if (!filename) return html();
  const ext = filename.split(".").pop().toLowerCase();
  if (ext === "css")                          return css();
  if (["js","mjs","ts","jsx","tsx"].includes(ext)) return javascript();
  return html();
}

// ── Rust-backed linter ────────────────────────────────────────────────────
let _lintTimer = null;
export function scheduleLint(view, onResult) {
  clearTimeout(_lintTimer);
  _lintTimer = setTimeout(async () => {
    try {
      const warnings = await lintHtml(view.state.doc.toString());
      if (!warnings) return;
      onResult?.(warnings);
      const diagnostics = warnings.map(w => {
        const lineCount = view.state.doc.lines;
        const lineNum   = Math.max(1, Math.min(w.line, lineCount));
        const lineObj   = view.state.doc.line(lineNum);
        const from      = lineObj.from + Math.max(0, (w.col ?? 1) - 1);
        const to        = Math.min(lineObj.to, from + Math.max(1, w.length ?? 1));
        return { from, to, severity: w.severity === "error" ? "error" : "warning", message: w.message, source: "BM-Aegis" };
      });
      view.dispatch(setDiagnostics(view.state, diagnostics));
    } catch { /* dev mode */ }
  }, 600);
}

// ── Rust-backed autocomplete ──────────────────────────────────────────────
function rustAutocomplete() {
  return async (context) => {
    const before = context.state.doc.sliceString(Math.max(0, context.pos - 60), context.pos);
    const inTag  = before.includes("<") && !before.slice(before.lastIndexOf("<")).includes(">");
    if (!inTag && !context.explicit) return null;

    const match   = context.matchBefore(/[\w:-]*/);
    const line    = context.state.doc.lineAt(context.pos).number - 1;
    const content = context.state.doc.sliceString(0, context.pos);

    try {
      const completions = await getCompletions(content, line);
      if (!completions?.length) return null;
      return {
        from:    match ? match.from : context.pos,
        options: completions.map(c => ({
          label:  c.label,
          detail: c.detail,
          apply:  c.insert || c.label,
          type:   c.typ || "keyword",
          boost:  c.boost ?? 0,
        })),
        validFor: /^[\w:-]*$/,
      };
    } catch { return null; }
  };
}

// ── Editor factory ────────────────────────────────────────────────────────
/**
 * Create and mount a fully-featured CodeMirror 6 editor.
 * Includes: Emmet Tab expansion, tag auto-rename, Rust lint + autocomplete.
 */
export function createEditor(parent, initialDoc = "", onUpdate, onCursor, onLint) {
  const view = new EditorView({
    state: EditorState.create({
      doc: initialDoc,
      extensions: [
        // Core structure
        lineNumbers(),
        highlightActiveLineGutter(),
        highlightSpecialChars(),
        history(),
        foldGutter(),
        drawSelection(),
        dropCursor(),
        EditorState.allowMultipleSelections.of(true),
        indentOnInput(),
        bracketMatching(),
        closeBrackets(),
        rectangularSelection(),
        crosshairCursor(),
        highlightActiveLine(),
        highlightSelectionMatches(),

        // Keymaps — Emmet BEFORE indentWithTab so Tab is intercepted first
        keymap.of([
          emmetKeymap(),           // Tab → Emmet expand
          ...closeBracketsKeymap,
          ...defaultKeymap,
          ...searchKeymap,
          ...historyKeymap,
          ...foldKeymap,
          ...completionKeymap,
          ...lintKeymap,
          indentWithTab,           // Tab fallback: indent
        ]),

        // Language (hot-swappable)
        languageCompartment.of(html()),

        // Word wrap (hot-swappable)
        wordWrapCompartment.of([]),

        // Readonly (hot-swappable)
        readonlyCompartment.of(EditorState.readOnly.of(false)),

        // Theme + syntax highlighting
        bmTheme,
        syntaxHighlighting(bmHighlight),

        // Lint gutter + inline underlines
        lintGutter(),

        // Autocomplete (Rust-backed)
        autocompletion({ override: [rustAutocomplete()] }),

        // Tag auto-rename plugin
        tagRenamePlugin,

        // Change + cursor listener
        EditorView.updateListener.of(update => {
          if (update.docChanged) {
            onUpdate?.({ doc: update.state.doc.toString() });
            scheduleLint(view, onLint);
          }
          if (update.selectionSet) {
            const sel  = update.state.selection.main;
            const line = update.state.doc.lineAt(sel.head);
            onCursor?.({ line: line.number, col: sel.head - line.from + 1 });
          }
        }),
      ],
    }),
    parent,
  });

  // Attach compartment reference so editor.js can toggle word wrap
  view.wordWrapCompartment = wordWrapCompartment;
  return view;
}
