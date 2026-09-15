//! WEB-014 RED: chat navigation, search and long-history UX (REQ-040).
//! Sidebar pin/archive/search/share/temporary/paged-history contract over
//! caller-owned persisted-shaped rows. Pure: no IO, no clock, no network.
//! `#[path]` include: `chat_nav_lane.rs` is owned by this lane and is NOT
//! wired into `lib.rs` (integrator assembles shared files).
#[path = "../src/chat_nav_lane.rs"]
mod chat_nav_lane;

use chat_nav_lane::{
    decode_nav, encode_nav, move_focus, page_messages, search_chats, set_archived, set_pin,
    share_state, shortcuts, sidebar_order, upsert_chat, ChatEntry, ChatNavError, FocusTarget,
    MessageRow, NavKey, SearchCancel, ShareState, MAX_NAV_SESSIONS, MAX_PAGE_ITEMS,
    MAX_QUERY_BYTES, MAX_SEARCH_RESULTS,
};

fn entry(id: &str, title: &str, updated: i64) -> ChatEntry {
    ChatEntry {
        id: id.to_owned(),
        title: title.to_owned(),
        pinned: false,
        archived: false,
        temporary: false,
        updated_us: updated,
    }
}

fn msg(id: &str, seq: u64, body: &str) -> MessageRow {
    MessageRow {
        id: id.to_owned(),
        seq,
        body: body.to_owned(),
    }
}

// WEB-014-T01: pin/unpin, title/content search, archive and progressive
// history loading operate on real persisted-shaped rows.
#[test]
fn web014_t01_navigation_search_and_paging() {
    let mut rows = vec![
        entry("a", "Alpha Project", 30),
        entry("b", "Beta Notes", 20),
        entry("c", "Gamma Tasks", 10),
    ];
    for e in rows.drain(..).collect::<Vec<_>>() {
        upsert_chat(&mut Vec::new(), e).unwrap();
    }
    let mut rows = vec![
        entry("a", "Alpha Project", 30),
        entry("b", "Beta Notes", 20),
        entry("c", "Gamma Tasks", 10),
    ];
    set_pin(&mut rows, "c", true).unwrap();
    let order = sidebar_order(&rows, false);
    assert_eq!(order[0].id, "c");
    assert!(order[0].pinned);

    set_archived(&mut rows, "b", true).unwrap();
    let order = sidebar_order(&rows, false);
    assert_eq!(order.len(), 2);
    assert!(order.iter().all(|e| e.id != "b"));
    let with_archived = sidebar_order(&rows, true);
    assert_eq!(with_archived.len(), 3);

    set_pin(&mut rows, "c", false).unwrap();
    let order = sidebar_order(&rows, false);
    assert_eq!(order[0].id, "a");

    let cancel = SearchCancel::new();
    let bodies = [("a", "alpha kickoff"), ("c", "gamma deploy checklist")];
    let hits = search_chats(&rows, &bodies, "alpha", &cancel).unwrap();
    assert!(hits.iter().any(|h| h.session_id == "a"));
    let hits = search_chats(&rows, &bodies, "deploy", &cancel).unwrap();
    assert_eq!(hits.len(), 1);
    assert_eq!(hits[0].session_id, "c");

    // Progressive oldest-first paging over a long history.
    let history: Vec<MessageRow> = (0..120u64).map(|s| msg(&format!("m{s}"), s, "x")).collect();
    let p1 = page_messages(&history, None, 50);
    assert_eq!(p1.items.len(), 50);
    assert_eq!(p1.items[0].seq, 0);
    assert_eq!(p1.items[49].seq, 49);
    let cursor = p1.next_cursor.expect("more pages remain");
    let p2 = page_messages(&history, Some(cursor), 50);
    assert_eq!(p2.items[0].seq, 50);
    assert_eq!(p2.items[49].seq, 99);
    let p3 = page_messages(&history, p2.next_cursor, 50);
    assert_eq!(p3.items.len(), 20);
    assert_eq!(p3.items[0].seq, 100);
    assert!(p3.next_cursor.is_none());
    let seen: Vec<u64> = p1
        .items
        .iter()
        .chain(p2.items.iter())
        .chain(p3.items.iter())
        .map(|m| m.seq)
        .collect();
    assert_eq!(seen.len(), 120);
}

// WEB-014-T02: temporary chat is explicitly non-durable; unavailable
// sharing/search states are never silently converted to ordinary chat.
#[test]
fn web014_t02_temporary_and_share_states_explicit() {
    let mut rows = vec![entry("t", "Scratch", 5)];
    rows[0].temporary = true;
    let normal = entry("n", "Scratch work", 6);

    let cancel = SearchCancel::new();
    let bodies = [("t", "scratch secret"), ("n", "scratch secret")];
    let all = vec![rows[0].clone(), normal.clone()];
    let hits = search_chats(&all, &bodies, "scratch", &cancel).unwrap();
    assert!(hits.iter().all(|h| h.session_id != "t"));
    assert!(hits.iter().any(|h| h.session_id == "n"));

    assert_eq!(share_state(&rows[0]), ShareState::UnavailableTemporary);
    let mut archived = normal.clone();
    archived.archived = true;
    assert_eq!(share_state(&archived), ShareState::UnavailableArchived);
    assert_eq!(share_state(&normal), ShareState::Available);

    // Temporary flag survives an encode/decode round trip, never dropped.
    let text = encode_nav(&all);
    let back = decode_nav(&text).unwrap();
    assert!(back.iter().find(|e| e.id == "t").unwrap().temporary);
}

// WEB-014-T03: sidebar/search/paging/action controls are keyboard
// navigable with discoverable shortcuts and correct focus after navigation.
#[test]
fn web014_t03_keyboard_focus_and_shortcuts() {
    assert_eq!(
        move_focus(FocusTarget::Sidebar, NavKey::FocusSearch),
        FocusTarget::SearchBox
    );
    assert_eq!(
        move_focus(FocusTarget::SearchBox, NavKey::Enter),
        FocusTarget::MessageList
    );
    assert_eq!(
        move_focus(FocusTarget::Sidebar, NavKey::Enter),
        FocusTarget::MessageList
    );
    assert_eq!(
        move_focus(FocusTarget::MessageList, NavKey::Escape),
        FocusTarget::Sidebar
    );
    assert_eq!(
        move_focus(FocusTarget::SearchBox, NavKey::Escape),
        FocusTarget::Sidebar
    );
    // In-list keys keep focus inside the list.
    assert_eq!(
        move_focus(FocusTarget::Sidebar, NavKey::Down),
        FocusTarget::Sidebar
    );
    assert_eq!(
        move_focus(FocusTarget::MessageList, NavKey::Up),
        FocusTarget::MessageList
    );

    let keys = shortcuts();
    assert!(keys.len() >= 4);
    for (key, label) in &keys {
        assert!(!key.is_empty());
        assert!(!label.is_empty());
    }
    assert!(keys.iter().any(|(k, _)| *k == "/"));
}

// WEB-014-T04: search and paging are bounded, cancellable and
// virtualizable; paging starts at the oldest message, not the newest.
#[test]
fn web014_t04_history_bounds_and_cancel() {
    let history: Vec<MessageRow> = (0..200u64).map(|s| msg(&format!("m{s}"), s, "b")).collect();
    let page = page_messages(&history, None, 5000);
    assert_eq!(page.items.len(), MAX_PAGE_ITEMS);
    assert_eq!(page.items[0].seq, 0);
    assert!(page.next_cursor.is_some());

    let rows = vec![entry("a", "Alpha", 1)];
    let cancel = SearchCancel::new();
    let long = "q".repeat(MAX_QUERY_BYTES + 1);
    assert_eq!(
        search_chats(&rows, &[], &long, &cancel).unwrap_err(),
        ChatNavError::QueryTooLong {
            max: MAX_QUERY_BYTES,
            actual: MAX_QUERY_BYTES + 1
        }
    );

    let mut cancelled = SearchCancel::new();
    cancelled.cancel();
    assert!(cancelled.is_cancelled());
    assert_eq!(
        search_chats(&rows, &[], "alpha", &cancelled).unwrap_err(),
        ChatNavError::Cancelled
    );

    // Result set is capped for virtualized rendering.
    let mut many = Vec::new();
    for i in 0..(MAX_SEARCH_RESULTS + 50) {
        many.push(entry(&format!("s{i}"), "common topic", i as i64));
    }
    let cancel = SearchCancel::new();
    let hits = search_chats(&many, &[], "common", &cancel).unwrap();
    assert_eq!(hits.len(), MAX_SEARCH_RESULTS);

    // Sidebar itself is bounded.
    let mut full = Vec::new();
    for i in 0..MAX_NAV_SESSIONS {
        upsert_chat(&mut full, entry(&format!("s{i}"), "t", i as i64)).unwrap();
    }
    assert_eq!(
        upsert_chat(&mut full, entry("overflow", "t", 0)).unwrap_err(),
        ChatNavError::TooManySessions {
            max: MAX_NAV_SESSIONS,
            actual: MAX_NAV_SESSIONS
        }
    );
}

// WEB-014-T05: pins, archive state, search results and message ordering
// remain stable after reload.
#[test]
fn web014_t05_reload_order_fidelity() {
    let mut rows = vec![
        entry("a", "Alpha\twith tab", 30),
        entry("b", "Beta\nnewline", 20),
        entry("c", "Gamma \\ backslash", 10),
    ];
    set_pin(&mut rows, "c", true).unwrap();
    set_archived(&mut rows, "b", true).unwrap();

    let text = encode_nav(&rows);
    let back = decode_nav(&text).unwrap();
    assert_eq!(back, rows);
    assert_eq!(sidebar_order(&back, true), sidebar_order(&rows, true));
    assert_eq!(sidebar_order(&back, false), sidebar_order(&rows, false));

    let cancel = SearchCancel::new();
    let bodies = [("a", "alpha body"), ("c", "gamma body")];
    let before = search_chats(&rows, &bodies, "body", &cancel).unwrap();
    let after = search_chats(&back, &bodies, "body", &cancel).unwrap();
    assert_eq!(before, after);

    assert_eq!(
        decode_nav("not-a-nav-line").unwrap_err(),
        ChatNavError::BadEncoding
    );
}
