//! browser/adblock.rs
//!
//! EasyList-compatible ad-blocking filter engine.
//!
//! Supports:
//!   ||domain.com^          Domain anchor blocking
//!   ||domain.com^$options  With filter options (third-party, image, etc.)
//!   @@||domain.com^        Exception (whitelist) rules
//!   /pattern*              Path pattern
//!   ##.css-selector        Cosmetic filter (hide element)
//!   domain.com##.selector  Site-specific cosmetic
//!   ! Comment              Ignored
//!
//! The engine has two modes:
//!   1. A built-in ruleset (bundled in binary for offline use)
//!   2. An optional user list at ~/.bm-aegis/filters/custom.txt
//!
//! Matching is done in two passes:
//!   - Domain rules:  HashSet lookup O(1)
//!   - Pattern rules: Linear scan of compiled regex Vec (rare path)

use std::{
    collections::HashSet,
    path::PathBuf,
};
use regex::Regex;
use serde::Serialize;
use url::Url;

// ─────────────────────────────────────────────────────────────────────────
// Built-in filter list
// ~300 of the highest-traffic ad/tracker patterns from EasyList + EasyPrivacy
// ─────────────────────────────────────────────────────────────────────────

const BUILTIN_RULES: &str = r#"
! BM-Aegis Built-in Adblock Rules
! Based on EasyList + EasyPrivacy (trimmed for performance)

! ── Ad networks (domain rules) ──────────────────────────────────────────
||doubleclick.net^
||googlesyndication.com^
||googleadservices.com^
||google-analytics.com^
||googletagmanager.com^
||googletagservices.com^
||googleoptimize.com^
||pagead2.googlesyndication.com^
||adservice.google.com^
||connect.facebook.net^
||an.facebook.com^
||static.ads-twitter.com^
||ads.twitter.com^
||ads.linkedin.com^
||px.ads.linkedin.com^
||bat.bing.com^
||aax.amazon-adsystem.com^
||c.amazon-adsystem.com^
||s.amazon-adsystem.com^
||static.criteo.net^
||widget.criteo.com^
||gum.criteo.com^
||bidder.criteo.com^
||static.hotjar.com^
||script.hotjar.com^
||heapanalytics.com^
||cdn.heapanalytics.com^
||api.segment.io^
||cdn.segment.com^
||api.mixpanel.com^
||munchkin.marketo.net^
||js.hsforms.net^
||js.hs-analytics.net^
||js.hs-scripts.com^
||amplify.outbrain.com^
||widgets.outbrain.com^
||cdn.taboola.com^
||trc.taboola.com^
||pixel.quantserve.com^
||edge.quantserve.com^
||sb.scorecardresearch.com^
||secure-dcr.imrworldwide.com^
||static.chartbeat.com^
||ping.chartbeat.net^
||srv.pixel.parsely.com^
||match.adsrvr.org^
||js.adsrvr.org^
||secure.adnxs.com^
||ib.adnxs.com^
||js.driftt.com^
||event.api.drift.com^
||bat.r.msn.com^
||c.clarity.ms^
||clarity.ms^
||mc.yandex.ru^
||mc.yandex.com^
||js-sec.indexww.com^
||delivery.openx.net^
||fastlane.rubiconproject.com^
||image6.pubmatic.com^
||ads.pubmatic.com^
||analytics.tiktok.com^
||ads.tiktok.com^
||ct.pinterest.com^
||rs.fullstory.com^
||cdn.logrocket.com^
||cdn.lr-in.com^
||idsync.rlcdn.com^
||b.crwdcntrl.net^
||bcp.crwdcntrl.net^
||a.appsflyer.com^
||t.adjust.com^
||cdn.optimizely.com^
||logx.optimizely.com^
||ekr.zdassets.com^
||tr.snapchat.com^
||sc-static.net^
||shareasale.com^
||www.shareasale.com^
||udc.bing.com^
||pi.pardot.com^
||cdn.krxd.net^
||static.hotjar.com^
||vars.hotjar.com^
||insights.hotjar.com^
||widget.intercom.io^
||js.intercomcdn.com^
||nexus-websocket-a.intercom.io^
||track.hubspot.com^
||forms.hubspot.com^
||widget.mixpanel.com^
||push.mixpanel.com^
||api.ipify.org^
||checkip.amazonaws.com^
||ipapi.co^

! ── Pattern rules ───────────────────────────────────────────────────────
/ads/*
/adserver/*
/advertisement/*
/banner/ads/*
/tracking/*
/tracker/*
/analytics/*
/stats/collect*
/pixel.gif*
/pixel.png*
/1x1.gif*
/beacon.gif*
/clicktrack/*
/doubleclick/*
/adsense/*

! ── Cosmetic filters (hide ad containers) ───────────────────────────────
##.advertisement
##.ad-container
##.ad-wrapper
##.ad-banner
##.ad-unit
##.ad-slot
##.ads-container
##.ads-wrapper
##.google-ad
##.adsbox
##[id^="ad-"]
##[id^="ads-"]
##[class^="ad-"]
##[class*=" ad "]
##[data-ad]
##[data-ad-unit]
##[data-google-container-id]
##.sponsored-content
##.sponsored-post
##.promoted-content
##.promoted-post
##[aria-label="Sponsored"]
##[aria-label="Advertisement"]
##.widget-area .google-adsense
##.sidebar-ad
##.header-ad
##.footer-ad
##.inline-ad
##.native-ad
##.outbrain-widget
##.taboola-widget
##[id*="taboola"]
##[id*="outbrain"]
##[class*="taboola"]
##[class*="outbrain"]
##.cookie-banner
##.cookie-notice
##.cookie-consent
##[id*="cookie-banner"]
##[class*="cookie-consent"]
"#;

// ─────────────────────────────────────────────────────────────────────────
// Data types
// ─────────────────────────────────────────────────────────────────────────

#[derive(Debug, Clone)]
struct PatternRule {
    regex:    Regex,
    is_allow: bool,
    options:  RuleOptions,
}

#[derive(Debug, Clone, Default)]
struct RuleOptions {
    third_party: Option<bool>, // Some(true) = only third-party, Some(false) = only first-party
    types:       Vec<String>,  // "image", "script", "stylesheet", "xmlhttprequest", …
}

#[derive(Debug, Default, Clone, Serialize)]
pub struct AdBlockStats {
    pub domains_blocked:  u64,
    pub patterns_blocked: u64,
    pub elements_hidden:  u64,
    pub total_rules:      usize,
    pub cosmetic_rules:   usize,
}

// ─────────────────────────────────────────────────────────────────────────
// Engine
// ─────────────────────────────────────────────────────────────────────────

pub struct AdBlockEngine {
    // Fast O(1) domain lookup
    blocked_domains:   HashSet<String>,
    allowed_domains:   HashSet<String>,

    // Pattern rules (for path/query matching)
    pattern_rules:     Vec<PatternRule>,

    // Cosmetic CSS rules — injected via JS into every page
    cosmetic_selectors: Vec<String>,

    // Stats
    pub stats: AdBlockStats,
}

impl AdBlockEngine {
    /// Build the engine from the built-in ruleset + optional user list.
    pub fn init() -> Self {
        let mut engine = Self {
            blocked_domains:   HashSet::new(),
            allowed_domains:   HashSet::new(),
            pattern_rules:     Vec::new(),
            cosmetic_selectors: Vec::new(),
            stats: AdBlockStats::default(),
        };

        engine.load_rules(BUILTIN_RULES);

        // Load user custom rules if they exist
        if let Ok(custom) = std::fs::read_to_string(user_filter_path()) {
            engine.load_rules(&custom);
        }

        engine.stats.total_rules =
            engine.blocked_domains.len() +
            engine.allowed_domains.len() +
            engine.pattern_rules.len();
        engine.stats.cosmetic_rules = engine.cosmetic_selectors.len();

        engine
    }

    pub fn load_rules(&mut self, list: &str) {
        for raw_line in list.lines() {
            let line = raw_line.trim();
            if line.is_empty() || line.starts_with('!') { continue; }

            // Cosmetic filters: ##.selector or domain.com##.selector
            if let Some(pos) = line.find("##") {
                let selector = line[pos + 2..].trim();
                if !selector.is_empty() {
                    self.cosmetic_selectors.push(selector.to_string());
                }
                continue;
            }

            let is_allow = line.starts_with("@@");
            let rule = if is_allow { &line[2..] } else { line };

            // Split off options ($image,third-party,etc.)
            let (rule_body, opts_str) = if let Some(dpos) = rule.rfind('$') {
                (&rule[..dpos], &rule[dpos + 1..])
            } else {
                (rule, "")
            };

            let options = parse_options(opts_str);

            // Domain anchor: ||example.com^
            if rule_body.starts_with("||") {
                let inner = rule_body[2..].trim_end_matches('^');
                // Strip trailing path
                let domain = inner.split('/').next().unwrap_or(inner).to_lowercase();
                if !domain.is_empty() {
                    if is_allow {
                        self.allowed_domains.insert(domain);
                    } else {
                        self.blocked_domains.insert(domain);
                    }
                }
                continue;
            }

            // Pattern rule: /path* or keywords
            if rule_body.contains('*') || rule_body.starts_with('/') {
                if let Ok(re) = glob_to_regex(rule_body) {
                    self.pattern_rules.push(PatternRule { regex: re, is_allow, options });
                }
                continue;
            }
        }
    }

    // ──────────────────────────────────────────────────────────────────────
    // Decision methods
    // ──────────────────────────────────────────────────────────────────────

    /// Return true if `request_url` should be blocked.
    /// `origin_host` is the host of the page making the request (for third-party).
    pub fn should_block(&mut self, request_url: &str, origin_host: Option<&str>) -> bool {
        let url = match Url::parse(request_url) {
            Ok(u)  => u,
            Err(_) => return false,
        };
        let req_host = url.host_str().unwrap_or("").to_lowercase();

        // ── 1. Allow list (fast path) ──────────────────────────────────
        if self.allowed_domains.contains(&req_host) { return false; }
        for parent in parent_domains(&req_host) {
            if self.allowed_domains.contains(&parent) { return false; }
        }

        // ── 2. Domain block list ───────────────────────────────────────
        if self.blocked_domains.contains(&req_host) {
            self.stats.domains_blocked += 1;
            return true;
        }
        for parent in parent_domains(&req_host) {
            if self.blocked_domains.contains(&parent) {
                self.stats.domains_blocked += 1;
                return true;
            }
        }

        // ── 3. Pattern rules ───────────────────────────────────────────
        let is_third_party = origin_host
            .map(|o| effective_domain(o) != effective_domain(&req_host))
            .unwrap_or(false);

        for rule in &self.pattern_rules {
            // Check third-party option
            if let Some(tp) = rule.options.third_party {
                if tp != is_third_party { continue; }
            }

            if rule.regex.is_match(request_url) {
                if rule.is_allow { return false; }
                self.stats.patterns_blocked += 1;
                return true;
            }
        }

        false
    }

    /// Return a CSS `<style>` block to inject into every page,
    /// hiding known ad containers.
    pub fn cosmetic_css(&self) -> String {
        if self.cosmetic_selectors.is_empty() { return String::new(); }
        let selectors = self.cosmetic_selectors.join(",\n");
        format!(
            "/* BM-Aegis cosmetic ad filter */\n{} {{\n  display: none !important;\n  visibility: hidden !important;\n}}",
            selectors
        )
    }

    /// Return a JS snippet that injects cosmetic CSS into the page.
    pub fn cosmetic_injection_js(&self) -> String {
        let css = self.cosmetic_css();
        if css.is_empty() { return String::new(); }
        let escaped = css.replace('\\', "\\\\").replace('`', "\\`");
        format!(
            r#"(function() {{
  const _bm_style = document.createElement('style');
  _bm_style.setAttribute('data-bm-aegis', 'cosmetic');
  _bm_style.textContent = `{}`;
  (document.head || document.documentElement).appendChild(_bm_style);
}})();"#,
            escaped
        )
    }
}

// ─────────────────────────────────────────────────────────────────────────
// Helpers
// ─────────────────────────────────────────────────────────────────────────

fn parse_options(opts: &str) -> RuleOptions {
    let mut o = RuleOptions::default();
    for opt in opts.split(',') {
        match opt.trim() {
            "third-party" | "3p" => o.third_party = Some(true),
            "~third-party" => o.third_party = Some(false),
            t if !t.starts_with('~') => o.types.push(t.to_string()),
            _ => {}
        }
    }
    o
}

fn glob_to_regex(pattern: &str) -> Result<Regex, regex::Error> {
    let mut re = String::from("(?i)");
    for ch in pattern.chars() {
        match ch {
            '*' => re.push_str(".*"),
            '?' => re.push('.'),
            c   => {
                for ec in regex::escape(&c.to_string()).chars() { re.push(ec); }
            }
        }
    }
    Regex::new(&re)
}

/// Returns parent domain variants: "sub.a.b.com" → ["a.b.com", "b.com"]
fn parent_domains(host: &str) -> Vec<String> {
    let parts: Vec<&str> = host.split('.').collect();
    (1..parts.len().saturating_sub(1))
        .map(|i| parts[i..].join("."))
        .collect()
}

/// Returns the eTLD+1 of a host for third-party detection.
fn effective_domain(host: &str) -> String {
    let parts: Vec<&str> = host.split('.').collect();
    if parts.len() >= 2 {
        parts[parts.len() - 2..].join(".")
    } else {
        host.to_string()
    }
}

fn user_filter_path() -> PathBuf {
    dirs::data_dir()
        .unwrap_or_else(|| PathBuf::from("."))
        .join("bm-aegis")
        .join("filters")
        .join("custom.txt")
}

// ─────────────────────────────────────────────────────────────────────────
// Tests
// ─────────────────────────────────────────────────────────────────────────

#[cfg(test)]
mod tests {
    use super::*;

    fn engine() -> AdBlockEngine {
        AdBlockEngine::init()
    }

    #[test]
    fn blocks_google_analytics() {
        let mut e = engine();
        assert!(e.should_block("https://www.google-analytics.com/collect?v=1", None));
    }

    #[test]
    fn blocks_doubleclick() {
        let mut e = engine();
        assert!(e.should_block("https://stats.g.doubleclick.net/j/collect", None));
    }

    #[test]
    fn allows_regular_site() {
        let mut e = engine();
        assert!(!e.should_block("https://example.com/page", None));
    }

    #[test]
    fn pattern_blocks_ads_path() {
        let mut e = engine();
        assert!(e.should_block("https://somesite.com/ads/banner.gif", None));
    }

    #[test]
    fn cosmetic_css_not_empty() {
        let e = engine();
        assert!(!e.cosmetic_css().is_empty());
    }
}
