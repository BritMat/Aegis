//! search/metasearch.rs
//!
//! Privacy-first web metasearch.
//!
//! Uses DuckDuckGo's JavaScript-free HTML endpoint so:
//!   - No tracking pixels load (we're making a server-side request)
//!   - No fingerprinting
//!   - Results arrive as clean HTML we parse with `scraper`
//!   - All DuckDuckGo redirect wrappers are stripped; you get real URLs
//!
//! The reqwest client sets a standard User-Agent so DDG doesn't block it,
//! sends no cookies, and follows no cross-site redirects.

use std::error::Error;
use reqwest::Client;
use scraper::{Html, Selector};
use serde::Serialize;
use url::Url;

#[derive(Debug, Clone, Serialize)]
pub struct WebResult {
    pub title:   String,
    pub url:     String,
    pub snippet: String,
}

/// Run a privacy search query. Returns up to 25 results.
pub async fn search(query: &str) -> Result<Vec<WebResult>, Box<dyn Error + Send + Sync>> {
    if query.trim().is_empty() {
        return Ok(vec![]);
    }

    let client = build_client()?;
    let raw_html = fetch_ddg(&client, query).await?;
    let results  = parse_results(&raw_html);

    Ok(results)
}

// ─────────────────────────────────────────────────────────────────────────
// HTTP client
// ─────────────────────────────────────────────────────────────────────────

fn build_client() -> Result<Client, reqwest::Error> {
    Client::builder()
        // Masquerade as a regular browser to avoid bot detection
        .user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) \
             AppleWebKit/537.36 (KHTML, like Gecko) \
             Chrome/124.0.0.0 Safari/537.36"
        )
        // No cookies — privacy
        .cookie_store(false)
        // Reasonable timeout
        .timeout(std::time::Duration::from_secs(15))
        // Do not follow DuckDuckGo's own tracking redirects
        .redirect(reqwest::redirect::Policy::none())
        .build()
}

async fn fetch_ddg(client: &Client, query: &str) -> Result<String, Box<dyn Error + Send + Sync>> {
    let resp = client
        .get("https://html.duckduckgo.com/html/")
        .query(&[
            ("q",  query),
            ("kl", "wt-wt"),   // region-neutral
            ("kp", "-1"),      // safe search off (neutral)
            ("kn", "1"),       // https everywhere
            ("ka", "t"),       // use sans-serif
        ])
        .header("Accept", "text/html,application/xhtml+xml")
        .header("Accept-Language", "en-US,en;q=0.9")
        .send()
        .await?;

    // DDG sometimes returns 303 → follow manually to the HTML page
    let html = if resp.status().is_redirection() {
        if let Some(loc) = resp.headers().get("location") {
            let redirect_url = loc.to_str().unwrap_or("");
            let full_url = if redirect_url.starts_with('/') {
                format!("https://html.duckduckgo.com{}", redirect_url)
            } else {
                redirect_url.to_string()
            };
            client.get(&full_url).send().await?.text().await?
        } else {
            resp.text().await?
        }
    } else {
        resp.text().await?
    };

    Ok(html)
}

// ─────────────────────────────────────────────────────────────────────────
// HTML parser
// ─────────────────────────────────────────────────────────────────────────

fn parse_results(html: &str) -> Vec<WebResult> {
    let document = Html::parse_document(html);

    // DDG HTML result selectors (as of 2024 — verified against live output)
    let result_sel   = Selector::parse("div.result.results_links").unwrap();
    let title_sel    = Selector::parse("h2.result__title a.result__a").unwrap();
    let snippet_sel  = Selector::parse("a.result__snippet").unwrap();
    let url_sel      = Selector::parse("a.result__url").unwrap();

    let mut results = Vec::new();

    for result_el in document.select(&result_sel) {
        let title = result_el
            .select(&title_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .unwrap_or_default();

        if title.is_empty() { continue; }

        // DDG wraps real URLs in a redirect — extract the real destination
        let raw_href = result_el
            .select(&title_sel)
            .next()
            .and_then(|el| el.value().attr("href"))
            .unwrap_or("");

        let real_url = extract_real_url(raw_href);

        let snippet = result_el
            .select(&snippet_sel)
            .next()
            .map(|el| el.text().collect::<String>().trim().to_string())
            .or_else(|| {
                // Fallback: try the URL element text
                result_el
                    .select(&url_sel)
                    .next()
                    .map(|el| el.text().collect::<String>().trim().to_string())
            })
            .unwrap_or_default();

        // Filter out DDG ads and non-web results
        if real_url.is_empty()
            || real_url.contains("duckduckgo.com")
            || real_url.starts_with("javascript:")
        {
            continue;
        }

        results.push(WebResult {
            title,
            url: real_url,
            snippet,
        });

        if results.len() >= 25 { break; }
    }

    results
}

/// DuckDuckGo wraps result URLs like:
///   //duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&rut=...
/// We extract the `uddg` parameter which is the real URL.
fn extract_real_url(href: &str) -> String {
    // Handle relative protocol
    let href = if href.starts_with("//") {
        format!("https:{}", href)
    } else {
        href.to_string()
    };

    // Try to parse and extract uddg param
    if let Ok(url) = Url::parse(&href) {
        if let Some(uddg) = url.query_pairs().find(|(k, _)| k == "uddg") {
            return uddg.1.into_owned();
        }
    }

    // If not a DDG redirect, return as-is
    if href.starts_with("http") {
        href
    } else {
        String::new()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extract_url_from_ddg_redirect() {
        let href = "//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com%2Fpath&rut=abc";
        assert_eq!(extract_real_url(href), "https://example.com/path");
    }

    #[test]
    fn pass_through_plain_url() {
        let href = "https://example.com/page";
        assert_eq!(extract_real_url(href), "https://example.com/page");
    }
}
