#[path = "../src/tool_query.rs"]
mod tool_query;

use tool_query::{
    IndexedDoc, MARKER_MAX, MARKER_SUFFIX, QueryConfig, QueryError, TRUNC_MARKER, query,
};

fn def() -> QueryConfig {
    QueryConfig::default()
}

fn doc(path: &str, line: u32, text: &str) -> IndexedDoc {
    IndexedDoc {
        path: path.to_string(),
        line,
        text: text.to_string(),
    }
}

fn fixture_index() -> Vec<IndexedDoc> {
    vec![
        doc("crates/tools/src/executor.rs", 5, "runs tool calls"),
        doc("crates/tools/src/search.rs", 8, "tool search index"),
        doc(
            "crates/tools/src/output_store.rs",
            12,
            "OutputStore retains output",
        ),
        doc("crates/tools/src/tool_quota.rs", 3, "per-tool quota"),
    ]
}

#[test]
fn qry_t01_hit() {
    let idx = fixture_index();
    let r = query(&idx, "OutputStore", &def()).unwrap();
    assert!(!r.hits.is_empty());
    for h in &r.hits {
        assert!(!h.path.is_empty() && h.line > 0);
    }
    assert_eq!(r.hits[0].path, "crates/tools/src/output_store.rs");
}

#[test]
fn qry_t02_miss() {
    let idx = fixture_index();
    let r = query(&idx, "zz-no-such-token-qq", &def()).unwrap();
    assert!(r.hits.is_empty());
    assert!(!r.truncated);
}

#[test]
fn qry_t03_empty_query_error() {
    let idx = fixture_index();
    for q in ["", "   "] {
        assert_eq!(query(&idx, q, &def()), Err(QueryError::EmptyQuery));
    }
}

#[test]
fn qry_t04_byte_budget_truncation_marker() {
    let cfg = QueryConfig {
        enabled: true,
        max_hits: 1000,
        max_bytes: 256,
    };
    let idx: Vec<IndexedDoc> = (0..200u32)
        .map(|i| {
            doc(
                &format!("docs/{i:03}.rs"),
                i + 1,
                "tool output line marker",
            )
        })
        .collect();
    let r = query(&idx, "tool output", &cfg).unwrap();
    assert!(r.truncated);
    let last = r.hits.last().expect("truncated keeps tail hit");
    assert!(
        last.snippet.ends_with(MARKER_SUFFIX),
        "tail snippet must end with marker suffix, got {:?}",
        last.snippet
    );
    let bytes: usize = r.hits.iter().map(|h| h.snippet.len()).sum();
    assert!(
        bytes <= 256 + MARKER_MAX,
        "snippet bytes {bytes} exceed budget + marker"
    );
    let r2 = query(&idx, "tool output", &cfg).unwrap();
    assert_eq!(r.hits, r2.hits);
    assert!(TRUNC_MARKER.contains("[qry:truncated"));
}

#[test]
fn qry_t05_deterministic_order() {
    let idx = fixture_index();
    let r1 = query(&idx, "tool output", &def()).unwrap();
    let r2 = query(&idx, "tool output", &def()).unwrap();
    assert_eq!(r1.hits, r2.hits);
    let tie = vec![
        doc("b.rs", 1, "same content here"),
        doc("a.rs", 1, "same content here"),
    ];
    let r = query(&tie, "same content", &def()).unwrap();
    assert_eq!(r.hits.len(), 2);
    assert!(
        r.hits[0].path < r.hits[1].path,
        "path-asc tiebreak, got {:?}",
        r.hits.iter().map(|h| &h.path).collect::<Vec<_>>()
    );
}
