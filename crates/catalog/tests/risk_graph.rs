//! TOOL-018 risk graph queries, frozen TOOL-018-T01..T05.
//!
//! RED: fails on missing behavior. Module included via `#[path]` so the lane
//! owns exactly `src/risk_graph.rs` + this file; the integrator wires
//! `pub mod risk_graph;` into `lib.rs` later.
//!
//! Edge convention (per risk spec): `Edge { from, to }` means `to` directly
//! depends on `from`. `blast_radius`/`why` walk forward (`from` -> `to`,
//! i.e. toward dependents); `dep_path(a, b)` walks forward from `a` to `b`.

#[path = "../src/risk_graph.rs"]
mod risk_graph;

use risk_graph::{Edge, RiskCaps, RiskError, RiskIndex, SymDef};

fn sym(name: &str, kind: &str, file: &str, line: u32) -> SymDef {
    SymDef {
        name: name.to_owned(),
        kind: kind.to_owned(),
        file: file.to_owned(),
        line,
    }
}

fn edge(from: &str, to: &str) -> Edge {
    Edge {
        from: from.to_owned(),
        to: to.to_owned(),
    }
}

#[test]
fn tool018_t01_blast_radius_bounded() {
    let mut idx = RiskIndex::new(RiskCaps {
        max_nodes: 200,
        ..RiskCaps::default()
    });
    for (n, f, l) in [("a", "a.rs", 1), ("b", "b.rs", 2), ("c", "c.rs", 3)] {
        idx.index_symbol(sym(n, "fn", f, l)).unwrap();
    }
    idx.index_edge(edge("a", "b")).unwrap();
    idx.index_edge(edge("b", "c")).unwrap();
    for i in 0..300u32 {
        let d = format!("dep{i:03}");
        idx.index_symbol(sym(&d, "fn", "extra.rs", 1000 + i))
            .unwrap();
        idx.index_edge(edge("b", &d)).unwrap();
    }
    let r = idx.blast_radius("b").unwrap();
    assert!(r.truncated);
    assert!(r.members.len() <= 200);
    assert!(r.members.iter().any(|m| m.name == "c"));
}

#[test]
fn tool018_t02_why_cites_owner() {
    let mut idx = RiskIndex::new(RiskCaps::default());
    idx.index_symbol(sym("pay", "fn", "pay.rs", 42)).unwrap();
    idx.index_symbol(sym("checkout", "fn", "shop.rs", 7))
        .unwrap();
    idx.index_symbol(sym("refund", "fn", "shop.rs", 9)).unwrap();
    idx.index_edge(edge("pay", "checkout")).unwrap();
    idx.index_edge(edge("pay", "refund")).unwrap();
    let w = idx.why("pay").unwrap();
    assert_eq!(w.def.file, "pay.rs");
    assert_eq!(w.def.line, 42);
    assert_eq!(w.dependents.len(), 2);
}

#[test]
fn tool018_t03_dep_path_depth_bound() {
    let mut idx = RiskIndex::new(RiskCaps::default());
    for n in ["a", "b", "c", "d", "e"] {
        idx.index_symbol(sym(n, "fn", "g.rs", 1)).unwrap();
    }
    for (f, t) in [("a", "b"), ("a", "c"), ("b", "d"), ("c", "d"), ("d", "e")] {
        idx.index_edge(edge(f, t)).unwrap();
    }
    let max_depth = RiskCaps::default().max_depth;
    let p = idx.dep_path("a", "e").unwrap().expect("path a->e");
    assert_eq!(p[0].as_str(), "a");
    assert_eq!(p[p.len() - 1].as_str(), "e");
    assert!(p.len() <= max_depth + 1);
    assert!(idx.dep_path("e", "a").unwrap().is_none());
}

#[test]
fn tool018_t04_failure_states() {
    let mut idx = RiskIndex::new(RiskCaps::default());
    idx.index_symbol(sym("a", "fn", "a.rs", 1)).unwrap();
    let err = idx.blast_radius("ghost").unwrap_err();
    assert_eq!(err, RiskError::UnknownSymbol("ghost".to_owned()));
    let err = idx.index_edge(edge("a", "a")).unwrap_err();
    assert_eq!(err, RiskError::SelfEdge);
    idx.index_symbol(sym("b", "fn", "b.rs", 2)).unwrap();
    idx.index_edge(edge("a", "b")).unwrap();
    let before = idx.stats().1;
    idx.index_edge(edge("a", "b")).unwrap();
    assert_eq!(idx.stats().1, before);
    let mut small = RiskIndex::new(RiskCaps {
        max_symbols: 1,
        ..RiskCaps::default()
    });
    small.index_symbol(sym("only", "fn", "o.rs", 1)).unwrap();
    let err = small
        .index_symbol(sym("extra", "fn", "e.rs", 2))
        .unwrap_err();
    assert_eq!(err, RiskError::CapExceeded);
}

#[test]
fn tool018_t05_opt_out_zero_cost() {
    let mut idx = RiskIndex::disabled();
    assert!(idx.index_symbol(sym("a", "fn", "a.rs", 1)).is_ok());
    assert_eq!(idx.stats(), (0, 0));
    assert_eq!(idx.blast_radius("a").unwrap_err(), RiskError::Disabled);
    assert_eq!(idx.why("a").unwrap_err(), RiskError::Disabled);
    assert_eq!(idx.dep_path("a", "b").unwrap_err(), RiskError::Disabled);
    assert!(idx.owner_of("a.rs").is_empty());
}
