//! WEB-012 FROZEN: plugin/app/tool chooser with approvals (pure selection contract).
//! T01 real capability selection, T02 unavailable/denied never substituted,
//! T03 accessible chooser/approval with focus restore, T04 bounded execution,
//! T05 persistence/audit rehydrate without secrets.
//! `#[path]` include: lib.rs untouched, std-only, no I/O, no clock, no network.
#[path = "../src/web_tool_chooser.rs"]
mod web_tool_chooser;

use web_tool_chooser::{
    ApprovalQueue, Capability, CapabilityKind, CapabilityState, ChooserError, Decision,
    Registry, Selection, MAX_SEARCH_RESULTS, MAX_SELECTION, MAX_SNAPSHOT_BYTES, rehydrate,
    snapshot, turn_contract,
};

fn cap(
    id: &str,
    kind: CapabilityKind,
    state: CapabilityState,
    needs_approval: bool,
    permission: &str,
) -> Capability {
    Capability {
        id: id.to_string(),
        label: format!("{id} label"),
        kind,
        state,
        permission: permission.to_string(),
        needs_approval,
    }
}

fn registry() -> Registry {
    Registry::new(vec![
        cap(
            "web-search",
            CapabilityKind::Tool,
            CapabilityState::Enabled,
            false,
            "",
        ),
        cap(
            "shell-exec",
            CapabilityKind::Tool,
            CapabilityState::Enabled,
            true,
            "run shell commands",
        ),
        cap(
            "photo-picker",
            CapabilityKind::Plugin,
            CapabilityState::Disabled,
            false,
            "",
        ),
        cap(
            "vault-reader",
            CapabilityKind::App,
            CapabilityState::Denied,
            true,
            "read vault",
        ),
    ])
    .expect("fixture registry builds")
}

fn big_registry(n: usize, needs_approval: bool) -> Registry {
    let mut caps = Vec::with_capacity(n);
    for i in 0..n {
        caps.push(cap(
            &format!("tool-{i:02}"),
            CapabilityKind::Tool,
            CapabilityState::Enabled,
            needs_approval,
            "",
        ));
    }
    Registry::new(caps).expect("big fixture registry builds")
}

#[test]
fn web012_t01_real_capability_selection_in_turn_contract() {
    let reg = registry();
    let mut sel = Selection::new();
    // Select in reverse registry order; contract must follow registry order.
    sel.select(&reg, "shell-exec").expect("enabled tool selects");
    sel.select(&reg, "web-search").expect("enabled tool selects");
    assert!(sel.contains("web-search"));
    assert!(sel.contains("shell-exec"));
    let contract = turn_contract(&reg, &sel);
    assert_eq!(contract, vec!["web-search".to_string(), "shell-exec".to_string()]);
    // Idempotent reselect keeps one entry.
    sel.select(&reg, "web-search").expect("reselect is idempotent");
    assert_eq!(sel.len(), 2);
}

#[test]
fn web012_t02_unavailable_denied_never_substituted() {
    let reg = registry();
    let mut sel = Selection::new();
    let err = sel.select(&reg, "photo-picker").unwrap_err();
    assert_eq!(err, ChooserError::Disabled);
    let err = sel.select(&reg, "vault-reader").unwrap_err();
    assert_eq!(err, ChooserError::Denied);
    let err = sel.select(&reg, "no-such-tool").unwrap_err();
    assert_eq!(err, ChooserError::Unknown);
    assert!(sel.is_empty(), "denied/disabled leave no selection behind");
    assert!(turn_contract(&reg, &sel).is_empty());
    // One enabled pick yields exactly itself: no fallback tool is injected.
    sel.select(&reg, "web-search").expect("enabled tool selects");
    assert_eq!(turn_contract(&reg, &sel), vec!["web-search".to_string()]);
}

#[test]
fn web012_t03_chooser_search_and_approval_focus_restore() {
    let reg = registry();
    let first = reg.search("search", 10);
    let second = reg.search("search", 10);
    assert!(first.iter().any(|c| c.id == "web-search"));
    assert_eq!(
        first.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
        second.iter().map(|c| c.id.clone()).collect::<Vec<_>>(),
        "search must be deterministic"
    );
    assert!(reg.search("e", 4).len() <= 4, "search honors the limit");
    assert!(
        reg.search("e", usize::MAX).len() <= MAX_SEARCH_RESULTS,
        "search honors the hard cap"
    );

    let mut queue = ApprovalQueue::new();
    // Tools that need no approval must not mint approvals.
    let err = queue
        .request(&reg, "web-search", "composer-tools-btn")
        .unwrap_err();
    assert_eq!(err, ChooserError::ApprovalNotRequired);
    // Denied tools must not escape denial through approvals.
    let err = queue
        .request(&reg, "vault-reader", "composer-tools-btn")
        .unwrap_err();
    assert_eq!(err, ChooserError::Denied);

    let id = queue
        .request(&reg, "shell-exec", "composer-tools-btn")
        .expect("approval-needing tool requests");
    let detail = queue.get(id).expect("pending approval is visible");
    assert_eq!(detail.tool_id, "shell-exec");
    assert_eq!(detail.permission, "run shell commands");
    let focus = queue.resolve(id, true).expect("approve resolves");
    assert_eq!(focus.return_to, "composer-tools-btn");
    assert_eq!(queue.live_approvals(), 0);
    assert_eq!(
        queue.audit(),
        &[web_tool_chooser::DecisionRecord {
            tool_id: "shell-exec".to_string(),
            decision: Decision::Approved,
            seq: 0,
        }]
    );

    let id2 = queue
        .request(&reg, "shell-exec", "composer-tools-btn")
        .expect("second request works");
    let focus2 = queue.cancel(id2).expect("cancel resolves focus");
    assert_eq!(focus2.return_to, "composer-tools-btn");
    assert_eq!(
        queue.audit().last().map(|r| r.decision),
        Some(Decision::Cancelled)
    );
}

#[test]
fn web012_t04_bounded_execution_and_cancel_reclaims() {
    let reg = big_registry(20, false);
    let mut sel = Selection::new();
    for i in 0..MAX_SELECTION {
        sel.select(&reg, &format!("tool-{i:02}"))
            .expect("selection fills to cap");
    }
    let err = sel.select(&reg, "tool-19").unwrap_err();
    assert_eq!(err, ChooserError::SelectionFull);

    let reg2 = big_registry(20, true);
    let mut queue = ApprovalQueue::new();
    let mut ids = Vec::new();
    for i in 0..web_tool_chooser::MAX_PENDING_APPROVALS {
        ids.push(
            queue
                .request(&reg2, &format!("tool-{i:02}"), "chooser-trigger")
                .expect("pending fills to cap"),
        );
    }
    let err = queue
        .request(&reg2, "tool-19", "chooser-trigger")
        .unwrap_err();
    assert_eq!(err, ChooserError::PendingFull);
    let dropped = queue.cancel_all();
    assert_eq!(dropped, web_tool_chooser::MAX_PENDING_APPROVALS);
    assert_eq!(queue.live_approvals(), 0);
    // Cancelled approvals own nothing: resolving one afterwards is unknown.
    let err = queue.resolve(ids[0], true).unwrap_err();
    assert_eq!(err, ChooserError::ApprovalUnknown);
}

#[test]
fn web012_t05_persistence_audit_rehydrate_without_secrets() {
    let reg = registry();
    let mut sel = Selection::new();
    sel.select(&reg, "web-search").expect("select");
    sel.select(&reg, "shell-exec").expect("select");
    let mut queue = ApprovalQueue::new();
    let id = queue
        .request(&reg, "shell-exec", "composer-tools-btn")
        .expect("request");
    queue.resolve(id, true).expect("approve");

    let bytes = snapshot(&sel, &queue);
    assert!(bytes.len() <= MAX_SNAPSHOT_BYTES);
    for banned in ["secret", "token", "password"] {
        assert!(
            !bytes.to_lowercase().contains(banned),
            "snapshot must carry no credentials: found {banned:?}"
        );
    }
    let (sel2, audit2) = rehydrate(&bytes, &reg).expect("roundtrip rehydrates");
    assert_eq!(sel2.ids(), sel.ids());
    assert_eq!(audit2, queue.audit());
    // Rehydrated selection still drives the real contract.
    assert_eq!(
        turn_contract(&reg, &sel2),
        vec!["web-search".to_string(), "shell-exec".to_string()]
    );

    // Unknown ids drop; disabled/denied never reactivate into the contract.
    let text = "rk-chooser-v1\nselected web-search\nselected ghost-tool\nselected photo-picker\nselected vault-reader\ndecision ghost-tool approved 0\n";
    let (sel3, audit3) = rehydrate(text, &reg).expect("unknown entries drop");
    assert_eq!(sel3.ids(), &["web-search".to_string()]);
    assert!(audit3.is_empty(), "unknown decision tools drop");
    assert_eq!(
        turn_contract(&reg, &sel3),
        vec!["web-search".to_string()]
    );

    // Malformed input fails atomically; oversize fails before parsing.
    assert_eq!(
        rehydrate("garbage-bytes", &reg).unwrap_err(),
        ChooserError::MalformedSnapshot
    );
    assert_eq!(
        rehydrate("rk-chooser-v1\nselected web-search extra-token", &reg).unwrap_err(),
        ChooserError::MalformedSnapshot
    );
    let big = "x".repeat(MAX_SNAPSHOT_BYTES + 1);
    assert_eq!(
        rehydrate(&big, &reg).unwrap_err(),
        ChooserError::SnapshotTooLarge
    );
}
