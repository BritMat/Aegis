# Editor Design

## Stack-based HTML tokenizer

`src-tauri/src/editor/tokenizer.rs`

The linter uses a **character-level state machine** (not a regex) so it handles edge cases that trip up simpler approaches:

| Edge case | Handled |
|-----------|---------|
| `<` inside attribute values `title="a < b"` | ✓ `DoubleQuotedAttr` / `SingleQuotedAttr` states |
| `<!-- <div> -->` (tags inside comments) | ✓ `Comment` → `CommentEndDash1` → `CommentEndDash2` |
| `<script>…</script>` raw text | ✓ `RawText` state, exits only on `</script>` |
| Void elements (`<br>`, `<img>`) | ✓ Never pushed to stack |
| Implicit close (`<p><p>`) | ✓ `implicit_close()` mirrors HTML5 optional-end-tag rules |
| Self-closing `<br/>` | ✓ `SelfClose` state, no push |
| DOCTYPE, XML PIs | ✓ `BangDecl` state, skipped |

### State machine states

```
Text → MaybeTag → OpenTagName → OpenTagAttrs ─┬→ SelfClose
                                               ├→ DoubleQuotedAttr
                                               ├→ SingleQuotedAttr
                                               └→ (close, push stack)
     ↘ ClosingTagName → (pop/check stack)
     ↘ CommentStart1 → CommentStart2 → Comment
     ↘ BangDecl
     ↘ RawText(tag) → RawTextClose(tag)
```

### Warning types produced

| Warning | Severity |
|---------|----------|
| `<tag>` opened but never closed | warning |
| `</tag>` has no matching opening tag | warning |
| `</void>` closing tag on a void element | warning |
| Unknown/non-standard element name | info |
| Mismatched close (stack scan + implicit close) | warning |

## Context-aware autocomplete

`src-tauri/src/editor/autocomplete.rs`

The autocomplete source analyses the **tokenizer's stack state** at the cursor position to determine what HTML5 content model allows at that point.

### Context detection

```
content up to cursor
  → tokenizer stack simulation → parent_tag
  → if cursor is inside <...> → attr mode
  → if space was typed inside tag → attr completion
  → else → tag name completion
```

### Content model coverage

| Parent context | Suggestions |
|----------------|-------------|
| `<ul>` / `<ol>` | Only `<li>` at top priority |
| `<dl>` | Only `<dt>` / `<dd>` |
| `<table>` | `<thead>` / `<tbody>` / `<tfoot>` / `<tr>` / `<caption>` |
| `<tr>` | Only `<td>` / `<th>` |
| `<select>` | Only `<option>` / `<optgroup>` |
| `<head>` | Meta tags only |
| `<p>`, `<h1>`–`<h6>`, `<li>` | Inline elements only |
| `<body>`, `<div>`, `<section>`, `<article>`, … | Full flow content |

Attribute completions are element-specific: `<input>` gets `type`, `required`, `maxlength`, `pattern`; `<a>` gets `href`, `target`, `rel`; etc. All completions include a snippet string with `$0`/`$1` cursor markers.

## Offline guarantee

All editor assets are bundled by Vite into `dist/` at build time and embedded in the Tauri binary. The editor requires **zero network access** — including font loading (system fonts only: `'Cascadia Code', 'Consolas', monospace`).

The Rust linter and autocomplete engine communicate via Tauri IPC over a local socket — no internet involved.
