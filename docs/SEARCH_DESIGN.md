# Search Design

## Local search — SQLite FTS5

`src-tauri/src/search/indexer.rs`

### Why FTS5?

FTS5 is SQLite's built-in full-text search engine. It provides:
- Porter stemmer (searches "running" also finds "run")
- BM25 ranking (better relevance than simple term frequency)
- `snippet()` function for highlighted excerpt extraction
- WAL journal mode for concurrent reads without locking

`rusqlite` is compiled with the `bundled` feature so a system SQLite install is **not required**. FTS5 support is guaranteed.

### Schema

```sql
-- Full-text index
CREATE VIRTUAL TABLE docs USING fts5(
    path    UNINDEXED,   -- file path (not searched, just returned)
    title,              -- filename
    content,            -- file text
    tokenize = 'porter unicode61'
);

-- Tracks file mtimes for incremental indexing
CREATE TABLE doc_meta (
    path  TEXT PRIMARY KEY,
    mtime INTEGER NOT NULL
);
```

### Indexing algorithm

```
for each file in walkdir(dir, max_depth=12):
  1. Check extension against allowlist (html, css, js, md, txt, rs, py, …)
  2. Check file size < 1 MB
  3. Query doc_meta for stored mtime
  4. If mtime unchanged → skip (incremental)
  5. read_to_string → INSERT/REPLACE INTO docs
  6. UPDATE doc_meta with new mtime
```

Files deleted from disk are removed on the next `prune_deleted()` call.

### Query sanitisation

Bare search terms are auto-quoted and joined with OR:
```
user types:  rust async
FTS5 query:  "rust" OR "async"
```

If the query already looks like an FTS5 expression (contains `"`, `AND`, `OR`, `NOT`) it is passed through unchanged, giving power users full FTS5 syntax access.

### Supported file types

```
html htm css js ts jsx tsx
md txt json xml svg yaml yml
toml ini cfg conf env sh
rs py rb go java kt cs cpp c h
```

## Web metasearch — DuckDuckGo HTML endpoint

`src-tauri/src/search/metasearch.rs`

### Why DuckDuckGo HTML?

`https://html.duckduckgo.com/html/` returns search results as clean, JavaScript-free HTML. Since the request is made **by Rust** (not by a browser), there is:

- No JavaScript execution → no fingerprinting
- No cookie sending → no tracking
- No redirect tracking → real destination URLs extracted from `uddg=` parameters
- No ad pixel loading → requests never leave the Rust process until results arrive

### Result extraction

Results are parsed with the `scraper` crate using CSS selectors:

| Selector | Extracts |
|----------|---------|
| `div.result.results_links` | Result container |
| `h2.result__title a.result__a` | Title + raw href |
| `a.result__snippet` | Snippet text |

DuckDuckGo wraps destination URLs in a redirect like:
```
//duckduckgo.com/l/?uddg=https%3A%2F%2Fexample.com&rut=abc123
```

The `extract_real_url()` function decodes the `uddg` parameter to give the actual destination URL.

### Request configuration

- User-Agent: standard Chrome 124 on Windows (avoids bot detection)
- Cookies: disabled (no session tracking)
- Redirects: manual (to avoid following DDG's own tracking redirects)
- Timeout: 15 seconds
- TLS: `rustls` (no system OpenSSL dependency)
- Results: capped at 25 per query

## Dual-mode UI

The search panel presents both modes with pill toggles. Switching mode is instant — no network call is made until the user presses Search or Enter. Web results open in the privacy browser window when clicked.
