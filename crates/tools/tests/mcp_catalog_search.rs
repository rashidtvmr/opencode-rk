// TOOL-016 contract tests: versioned MCP catalog search, bounded results.
// Maps to obligations TOOL-016-T01..T05 in tasks/TOOL-016.md.
use opencode_rk_tools::mcp_catalog_search::{
    CatalogEntry, CatalogError, CatalogQuery, DEFAULT_MAX_RESULTS, MAX_INDEX_ENTRIES,
    MAX_SNIPPET_BYTES, load_index, search,
};

fn entry(
    name: &str,
    description: &str,
    tags: &[&str],
    provider: &str,
    source: &str,
) -> CatalogEntry {
    CatalogEntry {
        name: name.to_owned(),
        description: description.to_owned(),
        tags: tags.iter().map(|t| t.to_string()).collect(),
        provider: provider.to_owned(),
        source: source.to_owned(),
    }
}

fn fixture_index() -> Vec<CatalogEntry> {
    vec![
        entry(
            "postgres",
            "Postgres MCP server for SQL queries",
            &["db", "sql"],
            "community",
            "github.com/example/postgres",
        ),
        entry(
            "redis",
            "Redis cache MCP server",
            &["db", "cache"],
            "community",
            "github.com/example/redis",
        ),
        entry(
            "s3",
            "Object storage MCP server",
            &["storage"],
            "acme",
            "github.com/acme/s3",
        ),
        entry(
            "github",
            "GitHub PR MCP server",
            &["vcs"],
            "acme",
            "github.com/acme/github-mcp",
        ),
        entry(
            "sqlite",
            "Embedded sqlite MCP server",
            &["db", "sql", "embedded"],
            "community",
            "github.com/example/sqlite",
        ),
    ]
}

fn query(text: &str) -> CatalogQuery {
    CatalogQuery {
        text: text.to_owned(),
        tag: None,
        provider: None,
        max_results: DEFAULT_MAX_RESULTS,
    }
}

#[test]
fn tool016_t01_name_search() {
    let index = fixture_index();
    let result = search(&index, &query("postgres")).unwrap();
    assert!(!result.hits.is_empty());
    let first = &result.hits[0];
    assert!(first.name.contains("postgres"));
    assert!(!first.provider.is_empty());
    assert!(!first.source.is_empty());
    assert!(first.snippet.as_bytes().len() <= MAX_SNIPPET_BYTES);
    assert!(!result.truncated);
}

#[test]
fn tool016_t02_tag_and_provider_filters() {
    let index = fixture_index();
    let tag_only = CatalogQuery {
        text: String::new(),
        tag: Some("db".to_owned()),
        provider: None,
        max_results: 20,
    };
    let hits = search(&index, &tag_only).unwrap();
    assert_eq!(hits.hits.len(), 3);
    assert!(
        hits.hits
            .iter()
            .all(|h| ["postgres", "redis", "sqlite"].contains(&h.name.as_str()))
    );
    let prov_only = CatalogQuery {
        text: String::new(),
        tag: None,
        provider: Some("acme".to_owned()),
        max_results: 20,
    };
    let hits = search(&index, &prov_only).unwrap();
    assert!(hits.hits.iter().all(|h| h.provider == "acme"));
    assert_eq!(hits.hits.len(), 2);
    let both = CatalogQuery {
        text: String::new(),
        tag: Some("db".to_owned()),
        provider: Some("community".to_owned()),
        max_results: 20,
    };
    let hits = search(&index, &both).unwrap();
    assert_eq!(hits.hits.len(), 3);
    let again = search(&index, &both).unwrap();
    assert_eq!(hits, again);
}

#[test]
fn tool016_t03_empty_and_miss() {
    let index = fixture_index();
    assert_eq!(search(&index, &query("   ")), Err(CatalogError::EmptyQuery));
    let empty_q = CatalogQuery {
        text: String::new(),
        tag: None,
        provider: None,
        max_results: 20,
    };
    assert_eq!(search(&index, &empty_q), Err(CatalogError::EmptyQuery));
    let miss = search(&index, &query("zzz-no-such-server")).unwrap();
    assert!(miss.hits.is_empty());
    assert!(!miss.truncated);
}

#[test]
fn tool016_t04_truncation_and_caps() {
    let mut index = Vec::new();
    for i in 0..10 {
        index.push(entry(
            &format!("data-svc-{i:02}"),
            "data service connector",
            &["svc"],
            "community",
            "github.com/example/svc",
        ));
    }
    let q = CatalogQuery {
        text: "svc".to_owned(),
        tag: None,
        provider: None,
        max_results: 2,
    };
    let result = search(&index, &q).unwrap();
    assert_eq!(result.hits.len(), 2);
    assert!(result.truncated);
    let mut big = Vec::new();
    for i in 0..(MAX_INDEX_ENTRIES + 1) {
        big.push(entry(&format!("e{i}"), "d", &["t"], "p", "s"));
    }
    assert_eq!(search(&big, &query("e")), Err(CatalogError::OverCap));
    let bad = br#"{"version":"9","updated":"2026-01-01","entries":[]}"#;
    assert_eq!(load_index(bad), Err(CatalogError::BadIndex));
}

#[test]
fn tool016_t05_determinism_and_safety() {
    let index = fixture_index();
    let q = query("sql");
    let a = serde_json::to_vec(&search(&index, &q).unwrap()).unwrap();
    let b = serde_json::to_vec(&search(&index, &q).unwrap()).unwrap();
    assert_eq!(a, b);
    let text = String::from_utf8(a).unwrap();
    assert!(!text.contains("sk-"));
    assert!(!text.contains("install"));
    // ponytail: inline versioned JSON; upgrade to catalog/mcp-index.json include when file lands.
    // Versioned index parses and carries source attribution.
    let doc = serde_json::json!({"version": "1", "updated": "2026-09-01", "entries": [
        {"name": "postgres", "description": "Postgres MCP server", "tags": ["db"], "provider": "community", "source": "github.com/example/postgres"}
    ]});
    let entries = load_index(&serde_json::to_vec(&doc).unwrap()).unwrap();
    assert!(!entries.is_empty());
    assert!(entries.iter().all(|e| !e.source.trim().is_empty()));
}
