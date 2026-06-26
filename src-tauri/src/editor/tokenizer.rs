//! editor/tokenizer.rs
//!
//! Stack-based HTML tokenizer and linter.
//!
//! Algorithm:
//!   Process the document character by character (or line by line with
//!   a state machine). Opening tags are pushed onto a stack; closing tags
//!   pop the stack and a mismatch is flagged immediately. Remaining stack
//!   entries at EOF are unclosed-tag warnings.
//!
//!   Handles:
//!     - Void elements (no push/pop needed)
//!     - HTML comments <!-- ... -->
//!     - <script> and <style> raw-text blocks
//!     - Self-closing tags (/>)
//!     - DOCTYPE declarations
//!     - Attribute value quotes so < inside values is ignored
//!
//!   Does NOT enforce:
//!     - Content model (what's allowed inside what) — handled by autocomplete
//!     - Attribute validity — out of scope for a live editor tool

use std::error::Error;
use serde::Serialize;

/// HTML void elements — never require a closing tag.
const VOID_ELEMENTS: &[&str] = &[
    "area", "base", "br", "col", "embed", "hr", "img", "input",
    "link", "meta", "param", "source", "track", "wbr",
    // Deprecated but commonly used
    "basefont", "bgsound", "frame", "keygen",
];

/// Raw-text elements — their content is opaque to the tag scanner.
const RAW_TEXT_ELEMENTS: &[&str] = &["script", "style"];

/// Severity levels matching CodeMirror lint API.
#[derive(Debug, Clone, Serialize, PartialEq)]
#[serde(rename_all = "lowercase")]
pub enum Severity { Error, Warning, Info }

#[derive(Debug, Clone, Serialize)]
pub struct LintWarning {
    pub line:     usize,     // 1-indexed
    pub col:      usize,     // 1-indexed
    pub severity: Severity,
    pub message:  String,
    pub length:   usize,     // character span for CM underlining
}

/// Tokenizer state machine states.
#[derive(Debug, PartialEq, Clone)]
enum State {
    Text,
    MaybeTag,            // saw <, deciding what kind
    OpenTagName,         // reading opening tag name
    OpenTagAttrs,        // inside <tag ... >
    SingleQuotedAttr,    // inside '...'
    DoubleQuotedAttr,    // inside "..."
    ClosingTagName,      // reading closing tag name </tag>
    CommentStart1,       // saw <!-
    CommentStart2,       // saw <!--
    Comment,             // inside <!-- ... -->
    CommentEndDash1,     // saw - inside comment
    CommentEndDash2,     // saw -- inside comment
    BangDecl,            // <!DOCTYPE ...>
    SelfClose,           // ending with />
    RawText(String),     // inside <script>/<style>, waiting for </name>
    RawTextClose(String, String), // collecting </name in raw text
}

/// Stack frame: tag name + location of the opening tag.
#[derive(Debug, Clone)]
struct StackFrame {
    tag:  String,
    line: usize,
    col:  usize,
}

pub fn lint(content: &str) -> Result<Vec<LintWarning>, Box<dyn Error>> {
    let mut warnings:  Vec<LintWarning> = Vec::new();
    let mut stack:     Vec<StackFrame>  = Vec::new();
    let mut state      = State::Text;
    let mut cur_name   = String::new();
    let mut tag_start  = (1usize, 1usize); // (line, col) of <
    let mut line       = 1usize;
    let mut col        = 1usize;

    let chars: Vec<char> = content.chars().collect();
    let len = chars.len();
    let mut i = 0;

    while i < len {
        let ch = chars[i];

        // Track line/col
        if ch == '\n' {
            line += 1;
            col   = 1;
        } else {
            col  += 1;
        }

        match &state.clone() {
            // ── Normal text ──────────────────────────────────────────────
            State::Text => {
                if ch == '<' {
                    tag_start = (line, col);
                    state = State::MaybeTag;
                    cur_name.clear();
                }
            }

            // ── Decide what kind of tag/declaration ──────────────────────
            State::MaybeTag => {
                match ch {
                    '/' => { state = State::ClosingTagName; }
                    '!' => { state = State::CommentStart1; }
                    c if c.is_ascii_alphabetic() => {
                        cur_name.push(c.to_ascii_lowercase());
                        state = State::OpenTagName;
                    }
                    '>' | '\n' | '\t' | ' ' => {
                        // Not a real tag (e.g. comparison operator in text)
                        state = State::Text;
                    }
                    _ => { state = State::Text; }
                }
            }

            // ── Reading the opening tag name ──────────────────────────────
            State::OpenTagName => {
                match ch {
                    c if c.is_ascii_alphanumeric() || c == '-' || c == ':' => {
                        cur_name.push(c.to_ascii_lowercase());
                    }
                    '/' => { state = State::SelfClose; }
                    '>' => {
                        finalise_open_tag(
                            &cur_name, tag_start, &mut stack, &mut warnings,
                        );
                        if RAW_TEXT_ELEMENTS.contains(&cur_name.as_str()) {
                            state = State::RawText(cur_name.clone());
                        } else {
                            state = State::Text;
                        }
                        cur_name.clear();
                    }
                    _ => { state = State::OpenTagAttrs; }
                }
            }

            // ── Inside tag attributes ─────────────────────────────────────
            State::OpenTagAttrs => {
                match ch {
                    '"' => { state = State::DoubleQuotedAttr; }
                    '\'' => { state = State::SingleQuotedAttr; }
                    '/' => {
                        if i + 1 < len && chars[i + 1] == '>' {
                            state = State::SelfClose;
                        }
                    }
                    '>' => {
                        finalise_open_tag(
                            &cur_name, tag_start, &mut stack, &mut warnings,
                        );
                        if RAW_TEXT_ELEMENTS.contains(&cur_name.as_str()) {
                            state = State::RawText(cur_name.clone());
                        } else {
                            state = State::Text;
                        }
                        cur_name.clear();
                    }
                    _ => {}
                }
            }

            State::DoubleQuotedAttr => {
                if ch == '"' { state = State::OpenTagAttrs; }
            }
            State::SingleQuotedAttr => {
                if ch == '\'' { state = State::OpenTagAttrs; }
            }

            // ── Self-closing />  ──────────────────────────────────────────
            State::SelfClose => {
                if ch == '>' {
                    // Self-closing tags: don't push to stack
                    cur_name.clear();
                    state = State::Text;
                }
            }

            // ── Closing tag </name> ───────────────────────────────────────
            State::ClosingTagName => {
                match ch {
                    c if c.is_ascii_alphanumeric() || c == '-' || c == ':' => {
                        cur_name.push(c.to_ascii_lowercase());
                    }
                    '>' => {
                        process_closing_tag(
                            &cur_name, tag_start, &mut stack, &mut warnings,
                        );
                        cur_name.clear();
                        state = State::Text;
                    }
                    _ => {}
                }
            }

            // ── Comment detection  <!--  ──────────────────────────────────
            State::CommentStart1 => {
                // We saw <! — next might be - for comment, or D for DOCTYPE
                match ch {
                    '-' => { state = State::CommentStart2; }
                    'D' | 'd' => { state = State::BangDecl; }
                    _ => { state = State::Text; }
                }
            }
            State::CommentStart2 => {
                if ch == '-' { state = State::Comment; }
                else         { state = State::Text; }
            }
            State::BangDecl => {
                // Skip everything up to >
                if ch == '>' { state = State::Text; }
            }

            // ── Inside comment ────────────────────────────────────────────
            State::Comment => {
                if ch == '-' { state = State::CommentEndDash1; }
            }
            State::CommentEndDash1 => {
                match ch {
                    '-' => { state = State::CommentEndDash2; }
                    _ =>   { state = State::Comment; }
                }
            }
            State::CommentEndDash2 => {
                match ch {
                    '>' => { state = State::Text; }
                    '-' => {} // stay in CommentEndDash2
                    _ =>  { state = State::Comment; }
                }
            }

            // ── Raw text (script/style content) ──────────────────────────
            State::RawText(raw_tag) => {
                if ch == '<' {
                    // Might be the closing tag — switch to watching
                    let rt = raw_tag.clone();
                    state = State::RawTextClose(rt, String::new());
                }
            }
            State::RawTextClose(raw_tag, closing_buf) => {
                let mut closing_buf = closing_buf.clone();
                let raw_tag = raw_tag.clone();
                closing_buf.push(ch.to_ascii_lowercase());

                let expected = format!("/{}>", raw_tag);
                if closing_buf.len() > expected.len() {
                    // Not the closing tag, reset
                    state = State::RawText(raw_tag);
                } else if closing_buf == expected {
                    // Found </script> or </style> — pop from stack
                    process_closing_tag(
                        &raw_tag, tag_start, &mut stack, &mut warnings,
                    );
                    state = State::Text;
                } else if expected.starts_with(&closing_buf) {
                    state = State::RawTextClose(raw_tag, closing_buf);
                } else {
                    state = State::RawText(raw_tag);
                }
            }
        }

        i += 1;
    }

    // Any remaining open tags are unclosed
    for frame in stack.iter().rev() {
        warnings.push(LintWarning {
            line:     frame.line,
            col:      frame.col,
            severity: Severity::Warning,
            message:  format!("<{}> is opened but never closed", frame.tag),
            length:   frame.tag.len() + 2,
        });
    }

    Ok(warnings)
}

fn finalise_open_tag(
    tag:      &str,
    pos:      (usize, usize),
    stack:    &mut Vec<StackFrame>,
    warnings: &mut Vec<LintWarning>,
) {
    if tag.is_empty() { return; }

    // Void elements never need closing tags
    if VOID_ELEMENTS.contains(&tag) { return; }

    // Warn about obsolete/unknown tags
    if !is_known_html5_tag(tag) {
        warnings.push(LintWarning {
            line:     pos.0,
            col:      pos.1,
            severity: Severity::Info,
            message:  format!("<{}> is not a standard HTML5 element", tag),
            length:   tag.len() + 2,
        });
    }

    // Implicit closing for optional-close elements
    // e.g. <p> closes any open <p> in the same block context
    implicit_close(tag, stack, pos, warnings);

    stack.push(StackFrame {
        tag: tag.to_string(),
        line: pos.0,
        col:  pos.1,
    });
}

/// Certain elements auto-close their predecessor (HTML5 optional close rules).
fn implicit_close(
    new_tag:  &str,
    stack:    &mut Vec<StackFrame>,
    pos:      (usize, usize),
    warnings: &mut Vec<LintWarning>,
) {
    let closes: &[(&str, &[&str])] = &[
        // <p> closes a preceding open <p>
        ("p",  &["p"]),
        // list items close previous li
        ("li", &["li"]),
        // dt/dd close each other and themselves
        ("dt", &["dt", "dd"]),
        ("dd", &["dt", "dd"]),
        // table cells
        ("tr", &["tr"]),
        ("td", &["td", "th"]),
        ("th", &["td", "th"]),
        // optgroup / option
        ("optgroup", &["optgroup"]),
        ("option",   &["option"]),
    ];

    for (opener, closeable_by) in closes {
        if closeable_by.contains(&new_tag) {
            if let Some(top) = stack.last() {
                if &top.tag == opener {
                    stack.pop();
                }
            }
        }
    }
    let _ = pos;
    let _ = warnings;
}

fn process_closing_tag(
    tag:      &str,
    pos:      (usize, usize),
    stack:    &mut Vec<StackFrame>,
    warnings: &mut Vec<LintWarning>,
) {
    if tag.is_empty() { return; }

    // Void elements never have closing tags — flag it
    if VOID_ELEMENTS.contains(&tag) {
        warnings.push(LintWarning {
            line:     pos.0,
            col:      pos.1,
            severity: Severity::Warning,
            message:  format!("<{}> is a void element and cannot have a closing tag", tag),
            length:   tag.len() + 3,
        });
        return;
    }

    if let Some(frame) = stack.last() {
        if frame.tag == tag {
            stack.pop();
        } else {
            // Scan for a matching opener further down the stack
            let matching_idx = stack.iter().rposition(|f| f.tag == tag);
            if let Some(idx) = matching_idx {
                // Warn about all skipped (implicitly unclosed) tags between idx+1 and top
                for skipped in stack[idx + 1..].iter().rev() {
                    warnings.push(LintWarning {
                        line:     skipped.line,
                        col:      skipped.col,
                        severity: Severity::Warning,
                        message:  format!(
                            "<{}> opened on line {} is closed by </{}> before being explicitly closed",
                            skipped.tag, skipped.line, tag
                        ),
                        length: skipped.tag.len() + 2,
                    });
                }
                stack.truncate(idx);
            } else {
                // No matching opener at all
                warnings.push(LintWarning {
                    line:     pos.0,
                    col:      pos.1,
                    severity: Severity::Warning,
                    message:  format!(
                        "</{}> has no matching opening tag — expected </{}>",
                        tag,
                        stack.last().map(|f| f.tag.as_str()).unwrap_or("…")
                    ),
                    length: tag.len() + 3,
                });
            }
        }
    } else {
        warnings.push(LintWarning {
            line:     pos.0,
            col:      pos.1,
            severity: Severity::Warning,
            message:  format!("</{}> has no matching opening tag", tag),
            length:   tag.len() + 3,
        });
    }
}

/// Return true for known HTML5 tag names.
fn is_known_html5_tag(tag: &str) -> bool {
    matches!(tag,
        "a" | "abbr" | "address" | "area" | "article" | "aside" | "audio" |
        "b" | "base" | "bdi" | "bdo" | "blockquote" | "body" | "br" | "button" |
        "canvas" | "caption" | "cite" | "code" | "col" | "colgroup" |
        "data" | "datalist" | "dd" | "del" | "details" | "dfn" | "dialog" |
        "div" | "dl" | "dt" |
        "em" | "embed" |
        "fieldset" | "figcaption" | "figure" | "footer" | "form" |
        "h1" | "h2" | "h3" | "h4" | "h5" | "h6" | "head" | "header" | "hgroup" |
        "hr" | "html" |
        "i" | "iframe" | "img" | "input" | "ins" |
        "kbd" | "label" | "legend" | "li" | "link" |
        "main" | "map" | "mark" | "menu" | "meta" | "meter" |
        "nav" | "noscript" |
        "object" | "ol" | "optgroup" | "option" | "output" |
        "p" | "picture" | "portal" | "pre" | "progress" |
        "q" | "rp" | "rt" | "ruby" |
        "s" | "samp" | "script" | "search" | "section" | "select" | "slot" |
        "small" | "source" | "span" | "strong" | "style" | "sub" | "summary" |
        "sup" | "table" | "tbody" | "td" | "template" | "textarea" | "tfoot" |
        "th" | "thead" | "time" | "title" | "tr" | "track" |
        "u" | "ul" | "var" | "video" | "wbr"
    )
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clean_document_has_no_warnings() {
        let html = r#"<!DOCTYPE html><html><head><title>Test</title></head><body><p>Hello</p></body></html>"#;
        let warnings = lint(html).unwrap();
        assert!(warnings.is_empty(), "Got warnings: {:?}", warnings);
    }

    #[test]
    fn detects_unclosed_div() {
        let html = "<html><body><div><p>text</p></body></html>";
        let warnings = lint(html).unwrap();
        assert!(warnings.iter().any(|w| w.message.contains("div")));
    }

    #[test]
    fn void_element_no_warning() {
        let html = "<html><body><br><img src='x.png'><hr></body></html>";
        let warnings = lint(html).unwrap();
        // Should not warn about void elements
        assert!(!warnings.iter().any(|w| w.message.contains("br") || w.message.contains("img")));
    }

    #[test]
    fn mismatch_detected() {
        let html = "<div><p>text</div></p>";
        let warnings = lint(html).unwrap();
        assert!(!warnings.is_empty());
    }

    #[test]
    fn comment_content_ignored() {
        let html = "<html><!-- <div> unclosed --><body></body></html>";
        let warnings = lint(html).unwrap();
        assert!(warnings.is_empty());
    }
}
