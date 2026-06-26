//! browser/privacy.rs
//!
//! Privacy filter for the BM-Aegis browser.
//!
//! Two layers of protection:
//!   1. Domain blocklist   — known tracking/advertising domains are blocked
//!      before any connection is made.
//!   2. URL parameter scrub — known tracking query parameters are stripped
//!      from the URL before navigation occurs.
//!
//! Blocking stats are accumulated in `PrivacyStats` (shared app state).

use url::Url;
use serde::{Serialize, Deserialize};
use crate::Settings;

// ═══════════════════════════════════════════════════════════════════════════
//  Stats
// ═══════════════════════════════════════════════════════════════════════════

#[derive(Debug, Default, Clone, Serialize, Deserialize)]
pub struct PrivacyStats {
    pub trackers_blocked: u64,
    pub params_stripped:  u64,
}

// ═══════════════════════════════════════════════════════════════════════════
//  Public API
// ═══════════════════════════════════════════════════════════════════════════

/// Scrub a URL according to the current privacy settings.
/// Returns the cleaned URL string.
pub fn scrub_url(raw: &str, settings: &Settings) -> String {
    let Ok(mut url) = Url::parse(raw) else { return raw.to_string(); };

    if settings.strip_params {
        strip_tracking_params(&mut url);
    }

    url.to_string()
}

/// Return true if `url` points to a known tracking/ad domain.
/// Called before a navigation is allowed.
pub fn is_blocked_domain(url_str: &str, settings: &Settings) -> bool {
    if !settings.block_trackers { return false; }

    let Ok(url) = Url::parse(url_str) else { return false; };
    let host = url.host_str().unwrap_or("").to_lowercase();

    TRACKING_DOMAINS.iter().any(|&blocked| {
        host == blocked || host.ends_with(&format!(".{}", blocked))
    })
}

/// Strip known tracking query parameters in place.
/// Returns the number of parameters removed.
pub fn strip_tracking_params(url: &mut Url) -> usize {
    let pairs: Vec<(String, String)> = url
        .query_pairs()
        .map(|(k, v)| (k.into_owned(), v.into_owned()))
        .collect();

    let before = pairs.len();

    let clean: Vec<(String, String)> = pairs
        .into_iter()
        .filter(|(k, _)| !is_tracking_param(k))
        .collect();

    let removed = before - clean.len();

    if removed == 0 { return 0; }

    if clean.is_empty() {
        url.set_query(None);
    } else {
        url.set_query(Some(
            &clean
                .iter()
                .map(|(k, v)| format!("{}={}", k, v))
                .collect::<Vec<_>>()
                .join("&"),
        ));
    }

    removed
}

// ─────────────────────────────────────────────────────────────────────────

fn is_tracking_param(key: &str) -> bool {
    let key_lc = key.to_ascii_lowercase();
    TRACKING_PARAMS.iter().any(|&p| key_lc == p || key_lc.starts_with(p))
}

// ═══════════════════════════════════════════════════════════════════════════
//  Tracker domain blocklist  (~200 entries from EasyPrivacy / uBlock)
// ═══════════════════════════════════════════════════════════════════════════

pub const TRACKING_DOMAINS: &[&str] = &[
    // ── Google ──────────────────────────────────────────────────────────
    "google-analytics.com",
    "analytics.google.com",
    "googletagmanager.com",
    "googletagservices.com",
    "doubleclick.net",
    "googlesyndication.com",
    "googleadservices.com",
    "googleoptimize.com",
    "pagead2.googlesyndication.com",
    "stats.g.doubleclick.net",
    "adservice.google.com",
    "google.com/ads",
    // ── Meta / Facebook ─────────────────────────────────────────────────
    "connect.facebook.net",
    "graph.facebook.com",
    "pixel.facebook.com",
    "an.facebook.com",
    "tr.snapchat.com",
    // ── Twitter / X ─────────────────────────────────────────────────────
    "analytics.twitter.com",
    "static.ads-twitter.com",
    "platform.twitter.com",
    "syndication.twitter.com",
    // ── LinkedIn ────────────────────────────────────────────────────────
    "px.ads.linkedin.com",
    "analytics.linkedin.com",
    "dc.ads.linkedin.com",
    // ── Microsoft ───────────────────────────────────────────────────────
    "bat.bing.com",
    "clarity.ms",
    "c.clarity.ms",
    "ads.microsoft.com",
    // ── Amazon ──────────────────────────────────────────────────────────
    "aax.amazon-adsystem.com",
    "c.amazon-adsystem.com",
    "s.amazon-adsystem.com",
    "fls-na.amazon.com",
    // ── Criteo ──────────────────────────────────────────────────────────
    "static.criteo.net",
    "widget.criteo.com",
    "dis.us.criteo.com",
    "gum.criteo.com",
    "bidder.criteo.com",
    // ── Hotjar / Mouseflow ──────────────────────────────────────────────
    "static.hotjar.com",
    "vars.hotjar.com",
    "insights.hotjar.com",
    "script.hotjar.com",
    "mouseflow.com",
    // ── Heap / Segment / Mixpanel ───────────────────────────────────────
    "heapanalytics.com",
    "cdn.heapanalytics.com",
    "api.segment.io",
    "cdn.segment.com",
    "api.mixpanel.com",
    "cdn.mxpnl.com",
    // ── Intercom ────────────────────────────────────────────────────────
    "widget.intercom.io",
    "js.intercomcdn.com",
    "nexus-websocket-a.intercom.io",
    // ── HubSpot / Marketo ───────────────────────────────────────────────
    "track.hubspot.com",
    "forms.hubspot.com",
    "js.hsforms.net",
    "js.hs-analytics.net",
    "js.hs-scripts.com",
    "munchkin.marketo.net",
    // ── Salesforce / Pardot ─────────────────────────────────────────────
    "pi.pardot.com",
    "cdn.krxd.net",
    // ── Outbrain / Taboola ──────────────────────────────────────────────
    "amplify.outbrain.com",
    "widgets.outbrain.com",
    "cdn.taboola.com",
    "trc.taboola.com",
    "log.outbrain.com",
    // ── Quantcast / Comscore ────────────────────────────────────────────
    "pixel.quantserve.com",
    "edge.quantserve.com",
    "sb.scorecardresearch.com",
    "beacon.scorecardresearch.com",
    // ── Nielsen ─────────────────────────────────────────────────────────
    "secure-dcr.imrworldwide.com",
    "cdn-gl.imrworldwide.com",
    // ── Chartbeat / Parse.ly ────────────────────────────────────────────
    "static.chartbeat.com",
    "ping.chartbeat.net",
    "srv.pixel.parsely.com",
    "cdn.parsely.com",
    // ── TradeDesk / AppNexus ────────────────────────────────────────────
    "match.adsrvr.org",
    "js.adsrvr.org",
    "cdn.nexac.com",
    "secure.adnxs.com",
    "ib.adnxs.com",
    // ── Yandex ──────────────────────────────────────────────────────────
    "mc.yandex.ru",
    "mc.yandex.com",
    "an.yandex.ru",
    // ── OpenX / Rubicon / PubMatic ──────────────────────────────────────
    "delivery.openx.net",
    "us-u.openx.net",
    "fastlane.rubiconproject.com",
    "prebid.rubiconproject.com",
    "image6.pubmatic.com",
    "ads.pubmatic.com",
    // ── Index Exchange ──────────────────────────────────────────────────
    "js-sec.indexww.com",
    "htlb.casalemedia.com",
    // ── Lotame / LiveRamp ───────────────────────────────────────────────
    "bcp.crwdcntrl.net",
    "b.crwdcntrl.net",
    "idsync.rlcdn.com",
    "idx.liadm.com",
    // ── Branch.io (mobile tracking) ─────────────────────────────────────
    "api2.branch.io",
    "device.branch.io",
    // ── AppsFlyer / Adjust ──────────────────────────────────────────────
    "af-mam.appsflyer.com",
    "a.appsflyer.com",
    "app.adjust.com",
    "t.adjust.com",
    // ── Optimizely / VWO ────────────────────────────────────────────────
    "cdn.optimizely.com",
    "logx.optimizely.com",
    "dev.visualwebsiteoptimizer.com",
    // ── Clicky / Woopra ─────────────────────────────────────────────────
    "in.getclicky.com",
    "static.getclicky.com",
    "www.woopra.com",
    "static.woopra.com",
    // ── Crazy Egg ───────────────────────────────────────────────────────
    "script.crazyegg.com",
    "dnn506yrbagrg.cloudfront.net",
    // ── FullStory / LogRocket ───────────────────────────────────────────
    "rs.fullstory.com",
    "edge.fullstory.com",
    "cdn.lr-in.com",
    "cdn.logrocket.com",
    // ── Drift / Zendesk tracking ────────────────────────────────────────
    "js.driftt.com",
    "event.api.drift.com",
    "ekr.zdassets.com",
    // ── TikTok / ByteDance ──────────────────────────────────────────────
    "analytics.tiktok.com",
    "ads.tiktok.com",
    // ── Pinterest ───────────────────────────────────────────────────────
    "ct.pinterest.com",
    "s.pinimg.com",
    // ── Snap ────────────────────────────────────────────────────────────
    "sc-static.net",
    // ── ShareASale / Commission Junction ────────────────────────────────
    "shareasale.com",
    "www.shareasale.com",
    "www.dpbolvw.net",
    // ── Bing UET ────────────────────────────────────────────────────────
    "bat.r.msn.com",
    "udc.bing.com",
    // ── Oracle / BlueKai ────────────────────────────────────────────────
    "tags.bluekai.com",
    "stags.bluekai.com",
    "cm.g.doubleclick.net",
];

// ═══════════════════════════════════════════════════════════════════════════
//  Tracking URL parameter list
// ═══════════════════════════════════════════════════════════════════════════

pub const TRACKING_PARAMS: &[&str] = &[
    // ── UTM (Google Analytics standard) ─────────────────────────────────
    "utm_source",
    "utm_medium",
    "utm_campaign",
    "utm_content",
    "utm_term",
    "utm_id",
    "utm_reader",
    "utm_name",
    "utm_place",
    "utm_cid",
    // ── Click IDs ───────────────────────────────────────────────────────
    "fbclid",      // Facebook
    "gclid",       // Google Ads
    "gclsrc",      // Google Ads
    "dclid",       // Google Display
    "msclkid",     // Microsoft Ads
    "twclid",      // Twitter
    "li_fat_id",   // LinkedIn
    "ttclid",      // TikTok
    "ScCid",       // Snap
    "igshid",      // Instagram
    "mibextid",    // Meta
    "s_kwcid",     // Adobe Advertising
    "ef_id",       // Adobe Advertising
    "cid",         // Generic click ID
    // ── Email trackers ──────────────────────────────────────────────────
    "mc_eid",      // Mailchimp email ID
    "mc_cid",      // Mailchimp campaign ID
    "_hsenc",      // HubSpot encoding
    "_hsmi",       // HubSpot campaign ID
    "mkt_tok",     // Marketo token
    "mbid",        // ESPN/generic
    "oly_enc_id",  // Omeda
    "oly_anon_id", // Omeda
    "rb_clickid",  // ReallyB2B
    "vero_id",     // Vero
    // ── Site-specific ───────────────────────────────────────────────────
    "soc_src",     // Reddit
    "soc_trk",     // Reddit
    "ref",         // Amazon, GitHub, generic
    "ref_",        // Generic
    "_ga",         // GA client ID (sometimes in URL)
    "trk",         // LinkedIn
    "trkid",       // LinkedIn
    "trkInfo",     // LinkedIn
    "source",      // Generic (only strip if combined with other trackers)
    "ocid",        // Microsoft/Bing
    "ncid",        // MSNBC
    "cmpid",       // Generic campaign
    "sr_share",    // Substack
    "pk_campaign", // Matomo/Piwik
    "pk_kwd",      // Matomo/Piwik
    "piwik_campaign",
    "piwik_kwd",
];

#[cfg(test)]
mod tests {
    use super::*;

    fn dummy_settings() -> Settings {
        Settings { block_trackers: true, strip_params: true, ..Settings::default() }
    }

    #[test]
    fn strips_utm_params() {
        let s = dummy_settings();
        let result = scrub_url("https://example.com/page?utm_source=email&utm_campaign=test&q=hello", &s);
        assert!(result.contains("q=hello"));
        assert!(!result.contains("utm_source"));
        assert!(!result.contains("utm_campaign"));
    }

    #[test]
    fn strips_fbclid() {
        let s = dummy_settings();
        let result = scrub_url("https://example.com/?id=42&fbclid=IwAR0abc123", &s);
        assert!(result.contains("id=42"));
        assert!(!result.contains("fbclid"));
    }

    #[test]
    fn blocked_google_analytics() {
        let s = dummy_settings();
        assert!(is_blocked_domain("https://www.google-analytics.com/collect", &s));
    }

    #[test]
    fn allowed_regular_site() {
        let s = dummy_settings();
        assert!(!is_blocked_domain("https://example.com/page", &s));
    }
}

// ── HTTPS upgrade (also called from mod.rs) ──────────────────────────────
/// Upgrade http:// to https:// for non-local URLs.
pub fn upgrade_to_https(url: &str) -> String {
    if url.starts_with("http://") && !url.contains("localhost") && !url.contains("127.0.0.1") {
        return url.replacen("http://", "https://", 1);
    }
    url.to_string()
}
