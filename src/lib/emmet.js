/**
 * emmet.js
 * Lightweight Emmet abbreviation expander for BM-Aegis.
 * Covers the 95% of patterns used daily.
 *
 * Supported syntax:
 *   div                  → <div>|</div>
 *   div.class            → <div class="class">|</div>
 *   div.c1.c2            → <div class="c1 c2">|</div>
 *   div#id               → <div id="id">|</div>
 *   div#id.cls           → <div id="id" class="cls">|</div>
 *   div[attr]            → <div attr="">|</div>
 *   div[href="#" title]  → <div href="#" title="">|</div>
 *   div{text}            → <div>text</div>
 *   parent>child         → <parent><child>|</child></parent>
 *   a+b                  → <a>|</a>\n<b>|</b>
 *   tag*3                → <tag>|</tag> (×3)
 *   (div>p)*2            → group repeated twice
 *   lorem                → lorem ipsum paragraph
 *   !                    → HTML5 boilerplate
 *
 * $0 marks the final cursor position.
 */

// Void elements — never produce a closing tag.
const VOID = new Set([
  "area","base","br","col","embed","hr","img","input",
  "link","meta","param","source","track","wbr",
]);

// Inline elements — no newlines around content.
const INLINE = new Set([
  "a","abbr","acronym","b","bdo","big","br","button","cite","code",
  "dfn","em","i","img","input","kbd","label","map","object","output",
  "q","samp","select","small","span","strong","sub","sup","textarea",
  "time","tt","u","var",
]);

// Known HTML5 short-hands that map to full snippets.
const SNIPPETS = {
  "!": `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>Document</title>
</head>
<body>
  $0
</body>
</html>`,
  "html:5": `<!DOCTYPE html>
<html lang="en">
<head>
  <meta charset="UTF-8">
  <meta name="viewport" content="width=device-width, initial-scale=1.0">
  <title>$0</title>
</head>
<body>
  
</body>
</html>`,
  "link:css": `<link rel="stylesheet" href="$0">`,
  "script:src": `<script src="$0"></script>`,
  "meta:vp":   `<meta name="viewport" content="width=device-width, initial-scale=1.0">`,
  "lorem":     "Lorem ipsum dolor sit amet, consectetur adipiscing elit. Sed do eiusmod tempor incididunt ut labore et dolore magna aliqua. Ut enim ad minim veniam, quis nostrud exercitation ullamco laboris.",
  "lorem30":   "Lorem ipsum dolor sit amet consectetur adipiscing elit sed do eiusmod tempor incididunt ut labore et dolore magna aliqua.",
};

// ─────────────────────────────────────────────────────────────────────────
// Tokeniser
// ─────────────────────────────────────────────────────────────────────────

function tokenise(abbr) {
  // Tokenise into a flat list of nodes that the parser assembles.
  // Returns an array of Token objects.
  const tokens = [];
  let i = 0;

  while (i < abbr.length) {
    const ch = abbr[i];

    if (ch === "(" ) { tokens.push({ t: "lparen" }); i++; continue; }
    if (ch === ")" ) { tokens.push({ t: "rparen" }); i++; continue; }
    if (ch === ">" ) { tokens.push({ t: "child"  }); i++; continue; }
    if (ch === "+" ) { tokens.push({ t: "sibling"}); i++; continue; }
    if (ch === "^" ) { tokens.push({ t: "climb"  }); i++; continue; }

    if (ch === "*") {
      // Read count
      let num = "";
      i++;
      while (i < abbr.length && /\d/.test(abbr[i])) { num += abbr[i++]; }
      tokens.push({ t: "repeat", n: parseInt(num, 10) || 1 });
      continue;
    }

    if (ch === "{") {
      // Text content
      let text = "";
      i++;
      while (i < abbr.length && abbr[i] !== "}") { text += abbr[i++]; }
      i++; // consume }
      tokens.push({ t: "text", v: text });
      continue;
    }

    if (ch === "[") {
      // Attributes
      let raw = "";
      i++;
      while (i < abbr.length && abbr[i] !== "]") { raw += abbr[i++]; }
      i++; // consume ]
      tokens.push({ t: "attrs", v: parseAttrs(raw) });
      continue;
    }

    // Element abbreviation: tag#id.class or just tag
    if (/[a-zA-Z!]/.test(ch) || ch === ":") {
      let name = "";
      let id   = null;
      let classes = [];

      // Tag name (allow colons for html:5 etc.)
      while (i < abbr.length && /[\w:-]/.test(abbr[i])) {
        name += abbr[i++];
      }

      // Parse qualifiers: #id and .class inline
      while (i < abbr.length && (abbr[i] === "#" || abbr[i] === ".")) {
        const qual = abbr[i++];
        let val = "";
        while (i < abbr.length && /[\w-]/.test(abbr[i])) val += abbr[i++];
        if (qual === "#") id = val;
        else classes.push(val);
      }

      tokens.push({ t: "elem", name, id, classes });
      continue;
    }

    // Implicit div: .class or #id at start
    if (ch === "." || ch === "#") {
      let id = null; let classes = [];
      while (i < abbr.length && (abbr[i] === "." || abbr[i] === "#")) {
        const qual = abbr[i++];
        let val = "";
        while (i < abbr.length && /[\w-]/.test(abbr[i])) val += abbr[i++];
        if (qual === "#") id = val;
        else classes.push(val);
      }
      tokens.push({ t: "elem", name: "div", id, classes });
      continue;
    }

    i++; // skip unknown
  }

  return tokens;
}

function parseAttrs(raw) {
  // "href='#' title class" → [{k:"href",v:"#"},{k:"title",v:""},{k:"class",v:""}]
  const attrs = [];
  const parts = raw.match(/[\w-]+(?:=(?:"[^"]*"|'[^']*'|[^\s\]]+))?/g) || [];
  for (const part of parts) {
    const eq = part.indexOf("=");
    if (eq === -1) {
      attrs.push({ k: part, v: "" });
    } else {
      const k = part.slice(0, eq);
      let v = part.slice(eq + 1).replace(/^["']|["']$/g, "");
      attrs.push({ k, v });
    }
  }
  return attrs;
}

// ─────────────────────────────────────────────────────────────────────────
// Code generator
// ─────────────────────────────────────────────────────────────────────────

/**
 * Expand an Emmet abbreviation string to an HTML snippet.
 * Returns { snippet, cursorOffset } where cursorOffset is the
 * character index of $0 within snippet.
 */
export function expand(abbr, indent = 0) {
  abbr = abbr.trim();

  // Check snippets first (exact match)
  if (SNIPPETS[abbr]) {
    const s = SNIPPETS[abbr];
    const off = s.indexOf("$0");
    return { snippet: s.replace("$0", ""), cursorOffset: off === -1 ? s.length : off };
  }

  // Special: lorem / loremN
  if (/^lorem\d*$/.test(abbr)) {
    const count = parseInt(abbr.slice(5), 10) || 0;
    const lorem = SNIPPETS["lorem"];
    const words = lorem.split(" ");
    const text = count > 0 ? words.slice(0, count).join(" ") : lorem;
    return { snippet: text, cursorOffset: text.length };
  }

  try {
    const tokens = tokenise(abbr);
    const { html, cursor } = generate(tokens, indent);
    const off = html.indexOf("$0");
    return {
      snippet: html.replace("$0", ""),
      cursorOffset: off === -1 ? html.length : off,
    };
  } catch {
    return null; // Not a valid abbreviation
  }
}

function generate(tokens, indent) {
  // Flat token list → tree of nodes → HTML string
  const nodes = buildTree(tokens);
  let html = "";
  let cursor = 0;

  for (let i = 0; i < nodes.length; i++) {
    if (i > 0) html += "\n" + "  ".repeat(indent);
    const { h, c } = renderNode(nodes[i], indent);
    cursor = html.length + c;
    html += h;
  }

  return { html, cursor };
}

function buildTree(tokens) {
  // Build a list of root-level nodes from the flat token list.
  // Each node may have children (from >) or siblings (from +).
  const roots = [];
  let i = 0;
  let cur = null;

  function nextElem() {
    // Find next elem token at current level
    while (i < tokens.length) {
      const tok = tokens[i++];
      if (tok.t === "elem") {
        return {
          name: tok.name, id: tok.id, classes: tok.classes,
          attrs: [], text: null, children: [], repeat: 1,
        };
      }
    }
    return null;
  }

  // Simple left-to-right parse
  while (i < tokens.length) {
    const tok = tokens[i];

    if (tok.t === "elem") {
      i++;
      cur = {
        name: tok.name, id: tok.id, classes: tok.classes,
        attrs: [], text: null, children: [], repeat: 1,
      };
      roots.push(cur);
      continue;
    }

    if (!cur) { i++; continue; }

    if (tok.t === "repeat") { i++; cur.repeat = tok.n; continue; }
    if (tok.t === "text")   { i++; cur.text   = tok.v; continue; }
    if (tok.t === "attrs")  { i++; cur.attrs   = tok.v; continue; }

    if (tok.t === "child") {
      i++;
      // Collect child subtree
      const childTokens = [];
      let depth = 0;
      while (i < tokens.length) {
        const t = tokens[i];
        if (t.t === "lparen") depth++;
        if (t.t === "rparen") { if (depth === 0) break; depth--; }
        if ((t.t === "sibling" || t.t === "climb") && depth === 0) break;
        childTokens.push(t);
        i++;
      }
      cur.children = buildTree(childTokens);
      continue;
    }

    if (tok.t === "sibling") {
      i++;
      cur = null; // Next elem becomes a new root
      continue;
    }

    i++;
  }

  return roots;
}

function renderNode(node, indent) {
  const parts = [];

  // Build opening tag
  let openTag = `<${node.name}`;
  if (node.id) openTag += ` id="${node.id}"`;
  if (node.classes?.length) openTag += ` class="${node.classes.join(" ")}"`;
  for (const a of (node.attrs || [])) {
    openTag += a.v ? ` ${a.k}="${a.v}"` : ` ${a.k}="${a.k === "src" || a.k === "href" ? "$0" : ""}"`;
  }

  const inline  = INLINE.has(node.name);
  const isVoid  = VOID.has(node.name);
  const hasKids = node.children?.length > 0;
  const hasText = node.text != null;

  let html = "";
  let cursorOff = -1;

  // Repeat handling
  const repeat = node.repeat || 1;
  for (let r = 0; r < repeat; r++) {
    if (r > 0) html += "\n" + "  ".repeat(indent);

    if (isVoid) {
      html += openTag + ">";
      if (r === 0) cursorOff = html.length;
    } else if (hasText) {
      html += `${openTag}>${node.text}</${node.name}>`;
      if (r === 0) cursorOff = html.length;
    } else if (hasKids) {
      const childIndent = indent + 1;
      const pad = "  ".repeat(childIndent);
      let childHtml = "";
      for (let ci = 0; ci < node.children.length; ci++) {
        if (ci > 0) childHtml += "\n" + pad;
        const { h } = renderNode(node.children[ci], childIndent);
        childHtml += h;
      }
      html += `${openTag}>\n${pad}${childHtml}\n${"  ".repeat(indent)}</${node.name}>`;
      if (r === 0) cursorOff = openTag.length + 2;
    } else if (inline) {
      html += `${openTag}>$0</${node.name}>`;
      if (r === 0) cursorOff = openTag.length + 1;
    } else {
      // Block element, cursor inside
      const inner = "$0";
      html += `${openTag}>${inner}</${node.name}>`;
      if (r === 0) cursorOff = openTag.length + 1;
    }
  }

  return { h: html, c: cursorOff === -1 ? html.length : cursorOff };
}

// ─────────────────────────────────────────────────────────────────────────
// CodeMirror 6 keybinding integration
// ─────────────────────────────────────────────────────────────────────────

import { EditorView } from "@codemirror/view";

/**
 * Returns a CM6 keymap entry that expands Emmet abbreviations on Tab.
 * Should be added BEFORE the indentWithTab keybind so Tab is intercepted.
 */
export function emmetKeymap() {
  return {
    key: "Tab",
    run(view) {
      const { state } = view;
      const sel = state.selection.main;
      if (!sel.empty) return false; // Don't intercept range selections

      // Get the word/abbr before cursor on this line
      const line    = state.doc.lineAt(sel.head);
      const lineText = line.text.slice(0, sel.head - line.from);

      // Find abbreviation: last contiguous chunk of Emmet chars
      // Stop at whitespace, <, >, =, "
      const match = lineText.match(/[\w!:.#\[\](){}*+>^@$|'"-]+$/);
      if (!match) return false;

      const abbr     = match[0];
      const abbrFrom = sel.head - abbr.length;

      const result = expand(abbr, 0);
      if (!result) return false;

      const { snippet, cursorOffset } = result;

      view.dispatch({
        changes: { from: abbrFrom, to: sel.head, insert: snippet },
        selection: { anchor: abbrFrom + cursorOffset },
        scrollIntoView: true,
      });

      return true;
    },
  };
}
