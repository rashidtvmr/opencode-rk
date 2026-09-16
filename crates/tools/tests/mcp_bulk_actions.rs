// TOOL-017 contract tests: bounded MCP selection plus bulk-action planner.
// Maps to obligations TOOL-017-T01..T05 in tasks/TOOL-017.md.
use opencode_rk_tools::mcp_bulk_actions::{
    BulkAction, BulkError, MAX_BULK_SELECTION, McpEntry, Outcome, Selection, UNKNOWN_CODE, apply,
};

fn entries(ids: &[&str], enabled: bool) -> Vec<McpEntry> {
    ids.iter().map(|id| McpEntry::new(*id, enabled)).collect()
}

#[test]
fn tool017_t01_select_all_enable() {
    let ids = ["srv-a", "srv-b", "srv-c", "srv-d", "srv-e"];
    let registry = entries(&ids, false);
    let mut sel = Selection::new();
    sel.select_all(ids).unwrap();
    let report = apply(&registry, &sel, BulkAction::Enable, false).unwrap();
    assert_eq!(report.succeeded, 5);
    assert_eq!(report.failed, 0);
    let got: Vec<&str> = report.per_item.iter().map(|i| i.id.as_str()).collect();
    assert_eq!(got, vec!["srv-a", "srv-b", "srv-c", "srv-d", "srv-e"]);
    assert!(
        report
            .per_item
            .iter()
            .all(|i| i.outcome == Outcome::Applied)
    );
}

#[test]
fn tool017_t02_invert_and_clear() {
    let ids = ["srv-a", "srv-b", "srv-c"];
    let mut sel = Selection::new();
    sel.select_all(ids).unwrap();
    sel.invert(ids).unwrap();
    assert!(sel.is_empty());
    sel.select("srv-a").unwrap();
    sel.select("srv-b").unwrap();
    assert!(sel.deselect("srv-a"));
    assert_eq!(sel.ids(), &["srv-b".to_string()]);
    sel.clear();
    assert!(sel.is_empty());
    assert!(!sel.deselect("srv-b"));
}

#[test]
fn tool017_t03_destructive_gating() {
    let registry = entries(&["srv-a", "srv-b"], true);
    let mut sel = Selection::new();
    sel.select_all(["srv-a", "srv-b"]).unwrap();
    let before = registry.clone();
    assert_eq!(
        apply(&registry, &sel, BulkAction::Remove, false),
        Err(BulkError::ConsentRequired)
    );
    assert_eq!(
        apply(&registry, &sel, BulkAction::Install, false),
        Err(BulkError::ConsentRequired)
    );
    assert_eq!(registry, before);
    assert_eq!(sel.len(), 2);
    let report = apply(&registry, &sel, BulkAction::Remove, true).unwrap();
    assert_eq!(report.succeeded, 2);
    assert_eq!(registry, before);
}

#[test]
fn tool017_t04_partial_failure() {
    let registry = entries(&["srv-a", "srv-b"], false);
    let mut sel = Selection::new();
    sel.select("srv-a").unwrap();
    sel.select("ghost").unwrap();
    let report = apply(&registry, &sel, BulkAction::Enable, false).unwrap();
    assert_eq!(report.succeeded, 1);
    assert_eq!(report.failed, 1);
    let ghost = report.per_item.iter().find(|i| i.id == "ghost").unwrap();
    assert_eq!(
        ghost.outcome,
        Outcome::Failed {
            code: UNKNOWN_CODE.to_string()
        }
    );
    let empty = Selection::new();
    assert_eq!(
        apply(&registry, &empty, BulkAction::Enable, false),
        Err(BulkError::EmptySelection)
    );
    let many: Vec<String> = (0..MAX_BULK_SELECTION + 1)
        .map(|i| format!("srv-{i:03}"))
        .collect();
    let mut sel2 = Selection::new();
    assert_eq!(sel2.select_all(many), Err(BulkError::TooManySelected));
    assert!(sel2.is_empty());
}

#[test]
fn tool017_t05_determinism_and_isolation() {
    let registry = entries(&["srv-a", "srv-b"], false);
    let mut sel = Selection::new();
    sel.select("srv-b").unwrap();
    sel.select("srv-a").unwrap();
    let a =
        serde_json::to_vec(&apply(&registry, &sel, BulkAction::Refresh, false).unwrap()).unwrap();
    let b =
        serde_json::to_vec(&apply(&registry, &sel, BulkAction::Refresh, false).unwrap()).unwrap();
    assert_eq!(a, b);
    let text = String::from_utf8(a).unwrap();
    assert!(text.contains("srv-a"));
    assert!(!text.contains("secret"));
    assert!(!text.contains("token"));
    let dbg = format!(
        "{:?}",
        apply(&registry, &sel, BulkAction::Refresh, false).unwrap()
    );
    assert!(!dbg.contains("secret"));
}
