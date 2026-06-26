// Auto-generated completion data — many private helper fns are intentional.
#![allow(dead_code, clippy::too_many_lines)]

//! editor/autocomplete.rs
//!
//! Context-aware HTML5 autocomplete.
//!
//! Uses the tokenizer's stack state to determine what element is currently
//! open, then filters the suggestion list to only what HTML5's content model
//! allows inside that element. Falls back to the full element list when
//! context is unclear (e.g. at root level with no open tags).
//!
//! Attribute-level completions are also provided when the cursor is inside
//! a tag and a space has just been typed.

use std::error::Error;
use serde::Serialize;
use super::tokenizer;

// ═══════════════════════════════════════════════════════════════════════════
//  Public API
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Clone, Serialize)]
pub struct Completion {
    /// Displayed label in the dropdown (e.g. "div", "href")
    pub label:  String,
    /// Right-aligned detail string (e.g. "block element", "string")
    pub detail: String,
    /// Snippet inserted when selected (e.g. `<div>$0</div>`)
    pub insert: String,
    /// CodeMirror completion type ("keyword" | "property" | "text")
    #[serde(rename = "type")]
    pub typ:    String,
    /// Higher = ranked first
    pub boost:  i32,
}

/// Return completions for the given content up to the cursor position.
pub fn completions_for(content: &str, line: usize) -> Result<Vec<Completion>, Box<dyn Error>> {
    // Determine the cursor's tag context via a truncated lint run.
    // We extract the stack from the tokenizer to see what's open.
    let context = infer_context(content, line);
    Ok(build_completions(&context))
}

// ═══════════════════════════════════════════════════════════════════════════
//  Context inference
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Default)]
struct CursorContext {
    /// The nearest open block-level ancestor (e.g. "div", "ul", "tr")
    parent_tag:   Option<String>,
    /// True when cursor is inside an open tag (between < and >)
    in_tag_attrs: bool,
    /// The tag name we're currently inside (when in_tag_attrs is true)
    current_tag:  Option<String>,
    /// True when cursor follows a space inside a tag (attribute mode)
    want_attr:    bool,
}

fn infer_context(content: &str, _line: usize) -> CursorContext {
    let mut ctx = CursorContext::default();

    // Run a quick tokenizer pass to get the stack.
    // We use the warnings-free path here — just care about the stack state.
    // We replicate part of the tokenizer's state machine just to extract
    // the open-tag stack at the end of the content.
    let mut stack: Vec<String> = Vec::new();

    // Simplified tag scan for context — not a full lint pass.
    let bytes = content.as_bytes();
    let len   = bytes.len();
    let mut i = 0;

    const VOID: &[&str] = &[
        "area","base","br","col","embed","hr","img","input",
        "link","meta","param","source","track","wbr",
    ];

    while i < len {
        if bytes[i] == b'<' {
            // Collect tag name
            let j = i + 1;
            if j >= len { break; }

            if bytes[j] == b'/' {
                // Closing tag
                let mut k = j + 1;
                let mut name = String::new();
                while k < len && bytes[k] != b'>' && bytes[k] != b' ' {
                    name.push(bytes[k] as char);
                    k += 1;
                }
                let name = name.to_lowercase();
                if let Some(pos) = stack.iter().rposition(|t| t == &name) {
                    stack.truncate(pos);
                }
                i = k;
            } else if bytes[j] != b'!' {
                // Opening tag
                let mut k = j;
                let mut name = String::new();
                while k < len && bytes[k] != b'>' && bytes[k] != b' ' && bytes[k] != b'/' {
                    name.push(bytes[k] as char);
                    k += 1;
                }
                let name = name.to_lowercase();
                if !name.is_empty() && !VOID.contains(&name.as_str()) {
                    // Skip self-closing
                    let mut scan = k;
                    let mut self_close = false;
                    while scan < len && bytes[scan] != b'>' {
                        if bytes[scan] == b'/' { self_close = true; }
                        scan += 1;
                    }
                    if !self_close {
                        stack.push(name);
                    }
                    i = scan;
                }
            }
        }
        i += 1;
    }

    // Innermost open tag is the completion parent
    ctx.parent_tag = stack.last().cloned();

    // Detect if the last characters are inside an open tag (attribute mode)
    let tail = &content[content.len().saturating_sub(200)..];
    if let Some(open_pos) = tail.rfind('<') {
        let after_open = &tail[open_pos..];
        let has_close = after_open.contains('>');
        if !has_close {
            ctx.in_tag_attrs = true;
            let name: String = after_open
                .chars()
                .skip(1) // skip <
                .take_while(|c| c.is_ascii_alphanumeric() || *c == '-')
                .collect();
            ctx.current_tag  = if name.is_empty() { None } else { Some(name.to_lowercase()) };
            ctx.want_attr    = after_open.contains(' ');
        }
    }

    ctx
}

// ═══════════════════════════════════════════════════════════════════════════
//  Completion builder
// ═══════════════════════════════════════════════════════════════════════════

fn build_completions(ctx: &CursorContext) -> Vec<Completion> {
    // Attribute completions when inside a tag
    if ctx.in_tag_attrs && ctx.want_attr {
        if let Some(ref tag) = ctx.current_tag {
            return attribute_completions(tag);
        }
        return global_attribute_completions();
    }

    // Tag name completions
    let parent = ctx.parent_tag.as_deref().unwrap_or("body");
    tag_completions(parent)
}

// ─────────────────────────────────────────────────────────────────────────
// HTML5 content-model-aware tag completions
// ─────────────────────────────────────────────────────────────────────────

fn tag_completions(parent: &str) -> Vec<Completion> {
    let tags: &[TagDef] = match parent {
        "html" => &[
            td("head", "document metadata",  "<head>\n  $0\n</head>", 10),
            td("body", "document body",       "<body>\n  $0\n</body>", 10),
        ],
        "head" => &[
            td("title",  "page title",        "<title>$0</title>", 10),
            td("meta",   "metadata",          "<meta $0>",          9),
            td("link",   "external resource", "<link rel=\"$0\">",  9),
            td("style",  "embedded styles",   "<style>\n  $0\n</style>", 8),
            td("script", "JavaScript",        "<script>\n  $0\n</script>", 8),
            td("base",   "base URL",          "<base href=\"$0\">", 7),
        ],
        "ul" | "ol" => &[
            td("li", "list item", "<li>$0</li>", 10),
        ],
        "dl" => &[
            td("dt", "term",        "<dt>$0</dt>", 10),
            td("dd", "description", "<dd>$0</dd>", 10),
        ],
        "table" => &[
            td("thead",   "table head",    "<thead>\n  $0\n</thead>",   9),
            td("tbody",   "table body",    "<tbody>\n  $0\n</tbody>",   9),
            td("tfoot",   "table foot",    "<tfoot>\n  $0\n</tfoot>",   8),
            td("tr",      "table row",     "<tr>\n  $0\n</tr>",         8),
            td("caption", "table caption", "<caption>$0</caption>",     7),
            td("colgroup","column group",  "<colgroup>$0</colgroup>",   6),
        ],
        "thead" | "tbody" | "tfoot" => &[
            td("tr", "table row", "<tr>\n  $0\n</tr>", 10),
        ],
        "tr" => &[
            td("td", "table data",   "<td>$0</td>", 10),
            td("th", "table header", "<th>$0</th>",  9),
        ],
        "select" => &[
            td("option",   "option",       "<option value=\"$0\"></option>", 10),
            td("optgroup", "option group", "<optgroup label=\"$0\">$1</optgroup>", 9),
        ],
        "figure" => &[
            td("figcaption", "figure caption", "<figcaption>$0</figcaption>", 10),
            td("img", "image", "<img src=\"$0\" alt=\"\">", 9),
        ],
        "details" => &[
            td("summary", "summary", "<summary>$0</summary>", 10),
        ],
        "colgroup" => &[
            td("col", "column", "<col $0>", 10),
        ],
        // Phrasing content parents — only inline elements
        "p" | "span" | "h1" | "h2" | "h3" | "h4" | "h5" | "h6"
        | "li" | "dt" | "dd" | "td" | "th" | "label" | "button" => {
            return inline_completions();
        }
        // Flow content parents — full block + inline
        _ => { return flow_completions(); }
    };

    tags.iter().map(|t| t.into()).collect()
}

fn inline_completions() -> Vec<Completion> {
    [
        td("a",       "hyperlink",   "<a href=\"$0\">$1</a>",             10),
        td("span",    "inline text", "<span>$0</span>",                   9),
        td("strong",  "bold text",   "<strong>$0</strong>",               9),
        td("em",      "italic text", "<em>$0</em>",                       9),
        td("code",    "inline code", "<code>$0</code>",                   8),
        td("br",      "line break",  "<br>",                              8),
        td("img",     "image",       "<img src=\"$0\" alt=\"$1\">",       8),
        td("abbr",    "abbreviation","<abbr title=\"$0\">$1</abbr>",      7),
        td("time",    "time",        "<time datetime=\"$0\">$1</time>",   7),
        td("mark",    "highlight",   "<mark>$0</mark>",                   7),
        td("sub",     "subscript",   "<sub>$0</sub>",                     6),
        td("sup",     "superscript", "<sup>$0</sup>",                     6),
        td("kbd",     "keyboard",    "<kbd>$0</kbd>",                     6),
        td("samp",    "sample",      "<samp>$0</samp>",                   5),
        td("var",     "variable",    "<var>$0</var>",                     5),
        td("input",   "form input",  "<input type=\"$0\">",               7),
        td("button",  "button",      "<button type=\"button\">$0</button>",7),
        td("label",   "label",       "<label for=\"$0\">$1</label>",      7),
        td("select",  "select",      "<select name=\"$0\">\n  $1\n</select>",6),
        td("textarea","textarea",    "<textarea name=\"$0\">$1</textarea>",6),
    ].iter().map(|t| t.into()).collect()
}

fn flow_completions() -> Vec<Completion> {
    let mut comps: Vec<Completion> = [
        // Sectioning
        td("div",         "division",        "<div>\n  $0\n</div>",                              9),
        td("section",     "section",         "<section>\n  $0\n</section>",                     9),
        td("article",     "article",         "<article>\n  $0\n</article>",                     9),
        td("aside",       "aside",           "<aside>\n  $0\n</aside>",                         8),
        td("header",      "page header",     "<header>\n  $0\n</header>",                       8),
        td("footer",      "page footer",     "<footer>\n  $0\n</footer>",                       8),
        td("nav",         "navigation",      "<nav>\n  $0\n</nav>",                             8),
        td("main",        "main content",    "<main>\n  $0\n</main>",                           8),
        // Headings
        td("h1", "heading 1", "<h1>$0</h1>", 9),
        td("h2", "heading 2", "<h2>$0</h2>", 9),
        td("h3", "heading 3", "<h3>$0</h3>", 8),
        td("h4", "heading 4", "<h4>$0</h4>", 7),
        td("h5", "heading 5", "<h5>$0</h5>", 7),
        td("h6", "heading 6", "<h6>$0</h6>", 7),
        // Text blocks
        td("p",           "paragraph",       "<p>$0</p>",                                       9),
        td("blockquote",  "blockquote",      "<blockquote>\n  <p>$0</p>\n</blockquote>",        7),
        td("pre",         "preformatted",    "<pre>$0</pre>",                                   7),
        td("code",        "code block",      "<code>$0</code>",                                 7),
        // Lists
        td("ul",          "unordered list",  "<ul>\n  <li>$0</li>\n</ul>",                      8),
        td("ol",          "ordered list",    "<ol>\n  <li>$0</li>\n</ol>",                      8),
        td("dl",          "description list","<dl>\n  <dt>$0</dt>\n  <dd>$1</dd>\n</dl>",       7),
        // Media
        td("img",         "image",           "<img src=\"$0\" alt=\"$1\">",                    8),
        td("video",       "video",           "<video src=\"$0\" controls>\n  $1\n</video>",     7),
        td("audio",       "audio",           "<audio src=\"$0\" controls></audio>",             7),
        td("figure",      "figure",          "<figure>\n  $0\n  <figcaption>$1</figcaption>\n</figure>", 7),
        td("picture",     "picture",         "<picture>\n  <source srcset=\"$0\">\n  <img src=\"$1\" alt=\"$2\">\n</picture>", 6),
        td("canvas",      "canvas",          "<canvas width=\"$0\" height=\"$1\"></canvas>",   6),
        td("iframe",      "iframe",          "<iframe src=\"$0\" title=\"$1\"></iframe>",       6),
        // Forms
        td("form",        "form",            "<form action=\"$0\" method=\"post\">\n  $1\n</form>", 8),
        td("input",       "input",           "<input type=\"$0\" name=\"$1\">",                8),
        td("button",      "button",          "<button type=\"button\">$0</button>",             8),
        td("label",       "label",           "<label for=\"$0\">$1</label>",                   7),
        td("select",      "select",          "<select name=\"$0\">\n  <option value=\"$1\">$2</option>\n</select>", 7),
        td("textarea",    "textarea",        "<textarea name=\"$0\" rows=\"4\">$1</textarea>", 7),
        td("fieldset",    "fieldset",        "<fieldset>\n  <legend>$0</legend>\n  $1\n</fieldset>", 6),
        // Semantic interactive
        td("details",     "details",         "<details>\n  <summary>$0</summary>\n  $1\n</details>", 7),
        td("dialog",      "dialog",          "<dialog>\n  $0\n</dialog>",                       6),
        // Table
        td("table",       "table",           "<table>\n  <thead>\n    <tr><th>$0</th></tr>\n  </thead>\n  <tbody>\n    <tr><td>$1</td></tr>\n  </tbody>\n</table>", 7),
        // Scripting
        td("script",      "script",          "<script>\n  $0\n</script>",                      6),
        td("template",    "template",        "<template>\n  $0\n</template>",                  5),
        // Misc inline
        td("span",        "inline span",     "<span>$0</span>",                                8),
        td("a",           "anchor/link",     "<a href=\"$0\">$1</a>",                          8),
        td("strong",      "bold",            "<strong>$0</strong>",                            7),
        td("em",          "italic",          "<em>$0</em>",                                    7),
        td("mark",        "highlight",       "<mark>$0</mark>",                                6),
        td("hr",          "horizontal rule", "<hr>",                                           5),
    ].iter().map(|t| t.into()).collect();

    comps.sort_by(|a, b| b.boost.cmp(&a.boost));
    comps
}

// ─────────────────────────────────────────────────────────────────────────
// Attribute completions
// ─────────────────────────────────────────────────────────────────────────

fn attribute_completions(tag: &str) -> Vec<Completion> {
    let mut comps: Vec<Completion> = global_attribute_completions();

    let specific: &[AttrDef] = match tag {
        "a" => &[
            ad("href",       "URL",                    "href=\"$0\"",          10),
            ad("target",     "_blank | _self | …",     "target=\"_blank\"",     9),
            ad("rel",        "relationship",            "rel=\"noopener\"",      8),
            ad("download",   "filename",               "download",              7),
            ad("hreflang",   "language code",          "hreflang=\"$0\"",       6),
        ],
        "img" => &[
            ad("src",        "URL",                    "src=\"$0\"",           10),
            ad("alt",        "alt text",               "alt=\"$0\"",           10),
            ad("width",      "pixels",                 "width=\"$0\"",          8),
            ad("height",     "pixels",                 "height=\"$0\"",         8),
            ad("loading",    "lazy | eager",           "loading=\"lazy\"",      7),
            ad("srcset",     "image set",              "srcset=\"$0\"",         6),
            ad("sizes",      "sizes",                  "sizes=\"$0\"",          5),
            ad("decoding",   "async | sync | auto",   "decoding=\"async\"",    5),
        ],
        "input" => &[
            ad("type",       "input type",             "type=\"$0\"",          10),
            ad("name",       "field name",             "name=\"$0\"",          10),
            ad("value",      "initial value",          "value=\"$0\"",          9),
            ad("placeholder","hint text",              "placeholder=\"$0\"",    9),
            ad("required",   "required field",         "required",              8),
            ad("disabled",   "disable input",          "disabled",              7),
            ad("readonly",   "read only",              "readonly",              7),
            ad("checked",    "checked state",          "checked",               7),
            ad("maxlength",  "max characters",         "maxlength=\"$0\"",      6),
            ad("min",        "minimum value",          "min=\"$0\"",            6),
            ad("max",        "maximum value",          "max=\"$0\"",            6),
            ad("step",       "step value",             "step=\"$0\"",           5),
            ad("pattern",    "regex pattern",          "pattern=\"$0\"",        5),
            ad("autocomplete","autocomplete hint",     "autocomplete=\"off\"",  5),
        ],
        "form" => &[
            ad("action",     "submit URL",             "action=\"$0\"",        10),
            ad("method",     "get | post",             "method=\"post\"",      10),
            ad("enctype",    "encoding type",          "enctype=\"multipart/form-data\"", 7),
            ad("novalidate", "skip validation",        "novalidate",            6),
            ad("target",     "_blank | _self",         "target=\"$0\"",         5),
        ],
        "button" => &[
            ad("type",       "submit | button | reset", "type=\"button\"",     10),
            ad("name",       "button name",            "name=\"$0\"",           8),
            ad("value",      "button value",           "value=\"$0\"",          8),
            ad("disabled",   "disable button",         "disabled",              7),
            ad("form",       "associated form id",     "form=\"$0\"",           6),
        ],
        "label" => &[
            ad("for",        "input id",               "for=\"$0\"",           10),
        ],
        "link" => &[
            ad("rel",        "relationship",            "rel=\"stylesheet\"",   10),
            ad("href",       "URL",                    "href=\"$0\"",          10),
            ad("type",       "MIME type",              "type=\"text/css\"",     8),
            ad("media",      "media query",            "media=\"$0\"",          7),
            ad("crossorigin","CORS",                   "crossorigin=\"anonymous\"", 5),
        ],
        "meta" => &[
            ad("name",       "metadata name",          "name=\"$0\"",          10),
            ad("content",    "metadata content",       "content=\"$0\"",       10),
            ad("charset",    "character set",          "charset=\"UTF-8\"",     9),
            ad("property",   "Open Graph property",    "property=\"$0\"",       7),
            ad("http-equiv", "HTTP header",            "http-equiv=\"$0\"",     6),
        ],
        "script" => &[
            ad("src",        "script URL",             "src=\"$0\"",            9),
            ad("type",       "MIME type",              "type=\"module\"",        8),
            ad("defer",      "defer execution",        "defer",                  8),
            ad("async",      "async execution",        "async",                  8),
            ad("crossorigin","CORS",                   "crossorigin=\"anonymous\"", 6),
            ad("integrity",  "SRI hash",               "integrity=\"$0\"",       5),
        ],
        "style" => &[
            ad("media",      "media query",            "media=\"$0\"",          8),
            ad("type",       "MIME type",              "type=\"text/css\"",     6),
        ],
        "select" => &[
            ad("name",       "field name",             "name=\"$0\"",          10),
            ad("multiple",   "allow multiple",         "multiple",               7),
            ad("size",       "visible rows",           "size=\"$0\"",            6),
            ad("required",   "required",               "required",               7),
            ad("disabled",   "disable",                "disabled",               6),
        ],
        "textarea" => &[
            ad("name",       "field name",             "name=\"$0\"",          10),
            ad("rows",       "row count",              "rows=\"$0\"",            8),
            ad("cols",       "column count",           "cols=\"$0\"",            7),
            ad("placeholder","hint text",              "placeholder=\"$0\"",     8),
            ad("required",   "required",               "required",               7),
            ad("readonly",   "read only",              "readonly",               6),
            ad("maxlength",  "max characters",         "maxlength=\"$0\"",       6),
            ad("wrap",       "hard | soft",            "wrap=\"$0\"",            5),
        ],
        "video" | "audio" => &[
            ad("src",        "media URL",              "src=\"$0\"",           10),
            ad("controls",   "show controls",          "controls",               9),
            ad("autoplay",   "autoplay",               "autoplay",               7),
            ad("loop",       "loop playback",          "loop",                   7),
            ad("muted",      "muted",                  "muted",                  7),
            ad("preload",    "none | metadata | auto", "preload=\"metadata\"",   6),
            ad("poster",     "poster image URL",       "poster=\"$0\"",          6),
            ad("width",      "width",                  "width=\"$0\"",           6),
            ad("height",     "height",                 "height=\"$0\"",          6),
        ],
        "iframe" => &[
            ad("src",        "page URL",               "src=\"$0\"",           10),
            ad("title",      "accessibility title",    "title=\"$0\"",         10),
            ad("width",      "width",                  "width=\"$0\"",           8),
            ad("height",     "height",                 "height=\"$0\"",          8),
            ad("loading",    "lazy | eager",           "loading=\"lazy\"",       7),
            ad("sandbox",    "sandbox policy",         "sandbox=\"$0\"",         6),
            ad("allow",      "permissions",            "allow=\"$0\"",           5),
            ad("frameborder","border",                 "frameborder=\"0\"",      4),
        ],
        "table" | "td" | "th" => &[
            ad("colspan",    "column span",            "colspan=\"$0\"",         9),
            ad("rowspan",    "row span",               "rowspan=\"$0\"",         9),
            ad("scope",      "col | row | colgroup",   "scope=\"col\"",          7),
            ad("headers",    "associated headers",     "headers=\"$0\"",         6),
        ],
        "details" | "dialog" => &[
            ad("open",       "initially open",         "open",                  8),
        ],
        _ => &[],
    };

    for attr in specific {
        comps.insert(0, attr.into());
    }

    comps.sort_by(|a, b| b.boost.cmp(&a.boost));
    comps
}

fn global_attribute_completions() -> Vec<Completion> {
    let globals: &[AttrDef] = &[
        ad("id",          "unique identifier",        "id=\"$0\"",             8),
        ad("class",       "CSS class names",          "class=\"$0\"",          8),
        ad("style",       "inline CSS",               "style=\"$0\"",          7),
        ad("title",       "tooltip text",             "title=\"$0\"",          6),
        ad("data-",       "custom data attr",         "data-$0=\"$1\"",        6),
        ad("tabindex",    "tab order",                "tabindex=\"$0\"",        5),
        ad("aria-label",  "accessible label",         "aria-label=\"$0\"",     5),
        ad("aria-hidden", "hide from a11y tree",      "aria-hidden=\"true\"",  5),
        ad("aria-live",   "live region",              "aria-live=\"polite\"",  4),
        ad("role",        "ARIA role",                "role=\"$0\"",           5),
        ad("lang",        "language",                 "lang=\"$0\"",           4),
        ad("dir",         "text direction",           "dir=\"ltr\"",           4),
        ad("hidden",      "hide element",             "hidden",                5),
        ad("draggable",   "draggable",                "draggable=\"true\"",    3),
        ad("contenteditable","editable",              "contenteditable=\"true\"", 3),
    ];
    globals.iter().map(|a| a.into()).collect()
}

// ═══════════════════════════════════════════════════════════════════════════
//  Helper types and constructors
// ═══════════════════════════════════════════════════════════════════════════

struct TagDef(&'static str, &'static str, &'static str, i32);
struct AttrDef(&'static str, &'static str, &'static str, i32);

fn td(label: &'static str, detail: &'static str, insert: &'static str, boost: i32) -> TagDef {
    TagDef(label, detail, insert, boost)
}
fn ad(label: &'static str, detail: &'static str, insert: &'static str, boost: i32) -> AttrDef {
    AttrDef(label, detail, insert, boost)
}

impl From<&TagDef> for Completion {
    fn from(t: &TagDef) -> Self {
        Completion {
            label:  t.0.to_string(),
            detail: t.1.to_string(),
            insert: t.2.to_string(),
            typ:    "keyword".to_string(),
            boost:  t.3,
        }
    }
}
impl From<&AttrDef> for Completion {
    fn from(a: &AttrDef) -> Self {
        Completion {
            label:  a.0.to_string(),
            detail: a.1.to_string(),
            insert: a.2.to_string(),
            typ:    "property".to_string(),
            boost:  a.3,
        }
    }
}
