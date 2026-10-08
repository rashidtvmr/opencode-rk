// SOURCE-PROPOSAL-UNINTEGRATED — REPAIR V2 (session-ownership P2 fixes)
//
// Standalone session-list view state for the native three-pane shell.
// Not GREEN: tests in this module are proposals and have NOT been executed.
// Input: parts/sessions/native_view_sessions.rs @ bbf94fe2e30c0072e72132ef16584b9d83ec19f673d7771a2818319a4c6e10c1
// Review disposition: component-review-v1 NEEDS_FIX (2 issues), repaired here:
//   1) parse_rows: title byte bound checked on RAW title before sanitize_title
//      (old post-sanitize check was unreachable -> silent truncation accept);
//   2) set_draft: id must be exactly known in the current rows before any
//      mutation (old code accepted any non-empty/valid-UUID id).
//
// Contract (exact public API):
//   #[derive(Clone, Debug, PartialEq, Eq)]
//   pub struct SessionRow { pub id: String, pub title: String }
//   pub struct SessionView (private bounded fields, Default impl)
//   fn set_rows / select_next / select_prev / acknowledge_active / render
//   pub fn parse_rows(value: &serde_json::Value) -> Result<Vec<SessionRow>, String>
//
// Bounds: rows 256, tabs 32, drafts 32, draft 32 KiB each, title 512 bytes.
// Selection is INTENT until acknowledge_active; render never fakes an active row.

use serde_json::Value;

const MAX_ROWS: usize = 256;
const MAX_TABS: usize = 32;
const MAX_DRAFTS: usize = 32;
const MAX_DRAFT_BYTES: usize = 32 * 1024;
const MAX_TITLE_BYTES: usize = 512;

/// One canonical session row as rendered in the sessions pane.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SessionRow {
    pub id: String,
    pub title: String,
}

/// Intent action proposed by the view; routed by the caller to real backend routes.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum SessionAction {
    Select { id: String },
    Create { title: String },
    Rename { id: String, title: String },
    Archive { id: String },
}

/// Optional pane-level lifecycle state (loading / empty / error are all valid).
#[derive(Clone, Debug, PartialEq, Eq, Default)]
pub enum SessionPaneState {
    #[default]
    Ready,
    Loading,
    Empty,
    Error(String),
}

/// Per-tab completion of a session-title request.
#[derive(Clone, Debug, PartialEq, Eq)]
enum Tab {
    Title(String),
    Fallback,
}

/// Bounded, side-effect-free session list state.
pub struct SessionView {
    rows: Vec<SessionRow>,
    selected: usize,
    active: Option<String>,
    pending: Option<String>,
    query: Option<String>,
    tabs: Vec<(String, Tab)>,
    drafts: Vec<(String, String)>,
    pane: SessionPaneState,
}

impl Default for SessionView {
    fn default() -> Self {
        Self {
            rows: Vec::new(),
            selected: 0,
            active: None,
            pending: None,
            query: None,
            tabs: Vec::new(),
            drafts: Vec::new(),
            pane: SessionPaneState::Ready,
        }
    }
}

impl SessionView {
    /// Replace the row set from a backend refresh.
    /// Keeps the selection by id; otherwise clamps. Active id is kept even if
    /// the backend no longer reports it so render can honestly show it missing.
    /// Pending intent is cleared when its target vanished. Unsent drafts for
    /// surviving ids are retained untouched.
    pub fn set_rows(&mut self, rows: Vec<SessionRow>, active: Option<String>) {
        let mut rows: Vec<SessionRow> = rows
            .into_iter()
            .take(MAX_ROWS)
            .map(|mut r| {
                r.id = sanitize_field(&r.id);
                r.title = sanitize_title(&r.title);
                r
            })
            .filter(|r| !r.id.is_empty())
            .collect();
        rows.dedup_by(|a, b| a.id == b.id);
        self.rows = rows;

        // Selection lives in filtered-window space: keep by id, else clamp.
        let prev_id = self
            .filtered_ids()
            .get(self.selected)
            .cloned()
            .or_else(|| self.rows.first().map(|r| r.id.clone()));
        let window = self.filtered_ids();
        self.selected = prev_id
            .and_then(|id| window.iter().position(|w| *w == id))
            .unwrap_or(0)
            .min(window.len().saturating_sub(1));

        if let Some(pending) = self.pending.take() {
            if !self.rows.iter().any(|r| r.id == pending) {
                // Pending intent target gone: drop it, never promote to active.
            } else {
                self.pending = Some(pending);
            }
        }

        self.active = active.filter(|a| !a.is_empty());
        if self.rows.is_empty() {
            self.pane = SessionPaneState::Empty;
        } else if matches!(self.pane, SessionPaneState::Empty) {
            self.pane = SessionPaneState::Ready;
        }
    }

    /// Move the highlight one row down. Returns the selected id as INTENT.
    /// Does not touch `active`. Empty list is inert.
    pub fn select_next(&mut self) -> Option<String> {
        let window = self.filter_window();
        if window.is_empty() {
            return None;
        }
        self.selected = (self.selected + 1).min(window.len() - 1);
        let id = self.rows[window[self.selected]].id.clone();
        self.pending = Some(id.clone());
        Some(id)
    }

    /// Move the highlight one row up. Returns the selected id as INTENT.
    /// Does not touch `active`. Empty list is inert.
    pub fn select_prev(&mut self) -> Option<String> {
        let window = self.filter_window();
        if window.is_empty() {
            return None;
        }
        self.selected = self.selected.saturating_sub(1);
        let id = self.rows[window[self.selected]].id.clone();
        self.pending = Some(id.clone());
        Some(id)
    }

    /// Commit the pending intent as the real active session (the caller has
    /// already issued the backend ACK). Only a known, valid id is accepted;
    /// opens a bounded tab; drafts untouched.
    pub fn acknowledge_active(&mut self, id: &str) {
        if !self.rows.iter().any(|r| r.id == id) {
            return;
        }
        if id.parse::<opencode_rk_contracts::SessionId>().is_err() {
            return;
        }
        self.active = Some(id.to_string());
        self.pending = None;
        if let Some(w) = self
            .filter_window()
            .iter()
            .position(|&i| self.rows[i].id == id)
        {
            self.selected = w;
        }
        self.open_tab(id);
    }

    /// Cancel the pending intent: restore the cursor to the active session and
    /// clear pending. Drafts are never modified here.
    pub fn cancel_pending(&mut self) {
        self.pending = None;
        if let Some(active) = self.active.clone() {
            if let Some(w) = self
                .filter_window()
                .iter()
                .position(|&i| self.rows[i].id == active)
            {
                self.selected = w;
            }
        }
    }

    /// Case-insensitive, Unicode-safe substring filter over id and title.
    /// Empty/whitespace query clears the filter. Selection follows the filter.
    pub fn search(&mut self, query: &str) -> Option<String> {
        let q: String = query.trim().to_lowercase();
        if q.is_empty() {
            self.query = None;
        } else {
            self.query = Some(q);
        }
        self.refilter();
        self.filtered_ids().first().cloned()
    }

    /// Set the current filter without reporting selection (None = clear).
    pub fn set_query(&mut self, query: Option<&str>) {
        self.query = query
            .map(|q| q.trim().to_lowercase())
            .filter(|q| !q.is_empty());
        self.refilter();
    }

    /// Store an unsent draft for one session id. Refuses to silently drop or
    /// truncate: oversize or unknown-id inserts are `Err`. The id must be
    /// exactly known in the current row set before any mutation; a non-empty
    /// or even canonical-UUID-but-unlisted id is `Err` and leaves all existing
    /// draft records unchanged. Detached drafts (session removed by a later
    /// refresh) stay readable via `draft()` but accept no new updates until
    /// the session is listed again; they are never silently deleted.
    pub fn set_draft(&mut self, id: &str, text: &str) -> Result<(), String> {
        if id.is_empty() {
            return Err("draft id must not be empty".into());
        }
        // P2 fix: ownership check before ANY mutation of the draft store.
        if !self.rows.iter().any(|r| r.id == id) {
            return Err(format!(
                "unknown session {id}: not in current session list; draft unchanged"
            ));
        }
        if text.len() > MAX_DRAFT_BYTES {
            return Err(format!(
                "draft for {id} exceeds {MAX_DRAFT_BYTES} bytes ({} bytes)",
                text.len()
            ));
        }
        if let Some(slot) = self.drafts.iter_mut().find(|(k, _)| k == id) {
            slot.1 = text.to_string();
            return Ok(());
        }
        if self.drafts.len() >= MAX_DRAFTS {
            return Err(format!(
                "draft capacity {MAX_DRAFTS} exceeded; discard a draft first"
            ));
        }
        self.drafts.push((id.to_string(), text.to_string()));
        Ok(())
    }

    /// Unsent draft for a session, if any. Never synthesized.
    pub fn draft(&self, id: &str) -> Option<&str> {
        self.drafts
            .iter()
            .find(|(k, _)| k == id)
            .map(|(_, v)| v.as_str())
    }

    /// Explicit user command: drop the draft. Drafts are otherwise preserved
    /// across selection changes, new rows and refreshes until the user discards
    /// them (backend-side removal also drops them via prune).
    pub fn discard_draft(&mut self, id: &str) {
        self.drafts.retain(|(k, _)| k != id);
    }

    /// Drop drafts whose session no longer exists (backend removed the session).
    /// Only called when a refresh proves the id is gone.
    pub fn prune_drafts(&mut self) {
        self.drafts
            .retain(|(k, _)| self.rows.iter().any(|r| &r.id == k));
    }

    /// Close the leftmost tab (bounded to MAX_TABS). Closing a tab never
    /// changes drafts or the active session.
    pub fn close_tab(&mut self) {
        if !self.tabs.is_empty() {
            self.tabs.remove(0);
        }
    }

    pub fn active_id(&self) -> Option<&str> {
        self.active.as_deref()
    }

    pub fn pending_id(&self) -> Option<&str> {
        self.pending.as_deref()
    }

    pub fn selected_id(&self) -> Option<&str> {
        let window = self.filter_window();
        let idx = *window.get(self.selected)?;
        self.rows.get(idx).map(|r| r.id.as_str())
    }

    pub fn row_count(&self) -> usize {
        self.rows.len()
    }

    pub fn pane_state(&self) -> &SessionPaneState {
        &self.pane
    }

    pub fn set_pane_state(&mut self, state: SessionPaneState) {
        self.pane = state;
    }

    fn filtered_ids(&self) -> Vec<String> {
        match self.query.as_ref() {
            None => self.rows.iter().map(|r| r.id.clone()).collect(),
            Some(q) => self
                .rows
                .iter()
                .filter(|r| r.id.to_lowercase().contains(q) || r.title.to_lowercase().contains(q))
                .map(|r| r.id.clone())
                .collect(),
        }
    }

    fn refilter(&mut self) {
        let ids = self.filtered_ids();
        let keep = self
            .rows
            .get(self.selected)
            .map(|r| r.id.clone())
            .unwrap_or_default();
        self.selected = match ids.iter().position(|i| *i == keep) {
            Some(i) => i,
            None => 0,
        };
    }

    fn open_tab(&mut self, id: &str) {
        if self.tabs.iter().any(|(k, _)| k == id) {
            return;
        }
        if self.tabs.len() >= MAX_TABS {
            self.tabs.remove(0);
        }
        let title = self
            .rows
            .iter()
            .find(|r| r.id == id)
            .map(|r| Tab::Title(r.title.clone()))
            .unwrap_or(Tab::Fallback);
        self.tabs.push((id.to_string(), title));
    }

    fn filter_window(&self) -> Vec<usize> {
        let ids = self.filtered_ids();
        ids.iter()
            .filter_map(|id| self.rows.iter().position(|r| &r.id == id))
            .collect()
    }

    /// Render the sessions pane: row 0 is the heading, rows 1..height-1 are
    /// rows from the filtered window (bounded by MAX_ROWS). Final cell clipping
    /// is delegated to the layout worker; this only bounds by `width`/`height`.
    pub fn render(&self, width: usize, height: usize) -> Vec<String> {
        if height == 0 {
            return Vec::new();
        }
        let width = width.max(1);
        let clip = |s: &str| -> String {
            let mut out = String::new();
            for c in s.chars() {
                if out.chars().count() + 1 > width {
                    break;
                }
                out.push(c);
            }
            out
        };

        let mut out = Vec::with_capacity(height);
        out.push(clip("Sessions"));
        if height == 1 {
            return out;
        }

        match &self.pane {
            SessionPaneState::Loading => {
                out.push(clip("...loading"));
                return out;
            }
            SessionPaneState::Error(e) => {
                out.push(clip(&format!("error: {e}")));
                return out;
            }
            SessionPaneState::Empty => {
                out.push(clip("(empty)"));
                return out;
            }
            SessionPaneState::Ready => {}
        }

        let window = self.filter_window();
        if window.is_empty() {
            out.push(clip("(no matches)"));
            return out;
        }

        let capacity = height - 1;
        let start = if window.len() <= capacity {
            0
        } else {
            self.selected
                .saturating_sub(capacity / 2)
                .min(window.len() - capacity)
        };
        for (offset, &idx) in window.iter().skip(start).take(capacity).enumerate() {
            let row = &self.rows[idx];
            let sel_pos = start + offset;
            let is_selected = sel_pos == self.selected;
            let is_active = self.active.as_deref() == Some(row.id.as_str());
            let is_pending = self.pending.as_deref() == Some(row.id.as_str());
            let marker = if is_active {
                "*"
            } else if is_selected {
                ">"
            } else {
                " "
            };
            let suffix = if is_pending && !is_active {
                " (pending)"
            } else {
                ""
            };
            out.push(clip(&format!("{marker}{}{suffix}", row.title)));
        }
        out
    }

    /// Render the open-tab strip from actual backend titles (or safe ids when
    /// the title is unavailable). Never invents session names.
    pub fn render_tabs(&self, width: usize, height: usize) -> Vec<String> {
        if height == 0 || width == 0 || self.tabs.is_empty() {
            return Vec::new();
        }
        let width = width.max(1);
        let mut line = String::new();
        for (i, (id, tab)) in self.tabs.iter().enumerate() {
            let label = match tab {
                Tab::Title(t) if !t.is_empty() => t.clone(),
                _ => id.clone(),
            };
            let piece = if i == 0 { label } else { format!(" | {label}") };
            if line.chars().count() + piece.chars().count() > width {
                break;
            }
            line.push_str(&piece);
        }
        let mut out = vec![line];
        out.resize(height.min(1), String::new());
        out
    }
}

/// Truncate to MAX_TITLE_BYTES at a UTF-8 boundary, strip control chars.
fn sanitize_title(raw: &str) -> String {
    let cleaned: String = raw
        .chars()
        .filter(|c| !c.is_control() || *c == ' ')
        .collect();
    truncate_utf8(&cleaned, MAX_TITLE_BYTES)
}

/// Minimal field sanitation: strip control characters, cap id length.
fn sanitize_field(raw: &str) -> String {
    let cleaned: String = raw.chars().filter(|c| !c.is_control()).collect();
    truncate_utf8(&cleaned, 128)
}

fn truncate_utf8(s: &str, max: usize) -> String {
    if s.len() <= max {
        return s.to_string();
    }
    let mut end = max;
    while end > 0 && !s.is_char_boundary(end) {
        end -= 1;
    }
    s[..end].to_string()
}

/// Parse the actual backend payload:
/// `{"sessions":[{"id":"<canonicalUUID>","title":"...","state":"active"}, ...]}`
/// A bare `[...]` array is also accepted. Entries with invalid ids, control
/// titles or over-long titles are rejected as `Err` so the caller keeps its
/// last good list. Arrays over MAX_ROWS are `Err` rather than a truncated list.
/// Unknown `state` values such as `archived` are skipped (not listed).
pub fn parse_rows(value: &Value) -> Result<Vec<SessionRow>, String> {
    let raw: Vec<&Value> = match value {
        Value::Array(items) => items.iter().collect(),
        Value::Object(map) => match map.get("sessions") {
            Some(Value::Array(items)) => items.iter().collect(),
            Some(other) => return Err(format!("expected sessions array, got {other}")),
            None => return Err("missing `sessions` array".into()),
        },
        other => return Err(format!("unexpected payload: {other}")),
    };

    if raw.len() > MAX_ROWS {
        return Err(format!(
            "server sent {} sessions; refusing to present a truncated {MAX_ROWS}-row list",
            raw.len()
        ));
    }

    let mut rows = Vec::with_capacity(raw.len());
    for entry in raw {
        let obj = entry
            .as_object()
            .ok_or_else(|| format!("session entry not an object: {entry}"))?;

        let state = obj
            .get("state")
            .map(|s| s.as_str().unwrap_or_default().to_string())
            .unwrap_or_default();
        if state == "archived" || state == "deleted" {
            continue;
        }

        let id = obj
            .get("id")
            .and_then(|v| v.as_str())
            .ok_or_else(|| "session entry missing string `id`".to_string())?;
        id.parse::<opencode_rk_contracts::SessionId>()
            .map_err(|e| format!("invalid session id {id:?}: {e}"))?;

        let title_raw = obj.get("title").and_then(|v| v.as_str()).unwrap_or("");
        // P2 fix: enforce the byte bound on the RAW backend title BEFORE any
        // sanitation. sanitize_title truncates at a UTF-8 boundary, so a
        // length check afterwards could never fire (unreachable) and an
        // oversized backend title would be silently accepted truncated.
        if title_raw.len() > MAX_TITLE_BYTES {
            return Err(format!(
                "session {id} title exceeds {MAX_TITLE_BYTES} bytes ({} bytes)",
                title_raw.len()
            ));
        }
        if title_raw.chars().any(|c| c.is_control() && c != ' ') {
            return Err(format!("session {id} title contains control characters"));
        }
        // Sanitation still strips controls and stays UTF-8-boundary safe; the
        // raw value already fits MAX_TITLE_BYTES so nothing is cut.
        let title = sanitize_title(title_raw);
        debug_assert!(title.len() <= MAX_TITLE_BYTES);

        rows.push(SessionRow {
            id: id.to_string(),
            title,
        });
    }
    Ok(rows)
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;

    const VALID_ID: &str = "550e8400-e29b-41d4-a716-446655440000";
    const OTHER_ID: &str = "6ba7b810-9dad-11d1-80b4-00c04fd430c8";

    fn view() -> SessionView {
        let mut v = SessionView::default();
        v.set_rows(
            vec![
                SessionRow {
                    id: VALID_ID.to_string(),
                    title: "alpha".into(),
                },
                SessionRow {
                    id: OTHER_ID.to_string(),
                    title: "beta".into(),
                },
            ],
            None,
        );
        v
    }

    #[test]
    fn selection_intent_requires_ack() {
        let mut v = view();
        let got = v.select_next();
        assert_eq!(got.as_deref(), Some(OTHER_ID));
        // INTENT must not become active before ACK.
        assert_eq!(v.active_id(), None);
        assert_eq!(v.pending_id(), Some(OTHER_ID));
        v.acknowledge_active(OTHER_ID);
        assert_eq!(v.active_id(), Some(OTHER_ID));
        assert_eq!(v.pending_id(), None);
    }

    #[test]
    fn ack_unknown_id_is_inert() {
        let mut v = view();
        let _ = v.select_next();
        v.acknowledge_active("00000000-0000-0000-0000-000000000000");
        assert_eq!(v.active_id(), None);
        assert_eq!(v.pending_id(), Some(OTHER_ID));
    }

    #[test]
    fn cancel_pending_restores_cursor_without_draft_change() {
        let mut v = view();
        v.set_draft(VALID_ID, "unsent").unwrap();
        v.acknowledge_active(VALID_ID);
        let _ = v.select_next();
        assert_eq!(v.pending_id(), Some(OTHER_ID));
        v.cancel_pending();
        assert_eq!(v.pending_id(), None);
        assert_eq!(v.selected_id(), Some(VALID_ID));
        assert_eq!(v.draft(VALID_ID), Some("unsent"));
    }

    #[test]
    fn drafts_are_isolated_per_session() {
        let mut v = view();
        v.set_draft(VALID_ID, "one").unwrap();
        v.set_draft(OTHER_ID, "two").unwrap();
        assert_eq!(v.draft(VALID_ID), Some("one"));
        assert_eq!(v.draft(OTHER_ID), Some("two"));
        v.discard_draft(VALID_ID);
        assert_eq!(v.draft(VALID_ID), None);
        assert_eq!(v.draft(OTHER_ID), Some("two"));
    }

    #[test]
    fn oversize_draft_refused_not_truncated() {
        let mut v = view(); // positive control: sessions registered first
        assert!(v.draft(VALID_ID).is_none());
        let big = "x".repeat(MAX_DRAFT_BYTES + 1);
        assert!(v.set_draft(VALID_ID, &big).is_err());
        assert_eq!(v.draft(VALID_ID), None);
        // exactly at the bound is accepted
        let ok = "x".repeat(MAX_DRAFT_BYTES);
        assert!(v.set_draft(VALID_ID, &ok).is_ok());
        assert_eq!(v.draft(VALID_ID).map(|d| d.len()), Some(MAX_DRAFT_BYTES));
    }

    #[test]
    fn oversize_title_bytes_rejected_source_json_unchanged() {
        const UNLISTED: &str = "00000000-0000-4000-8000-000000000001";
        // ASCII: 513 bytes
        let ascii = "a".repeat(MAX_TITLE_BYTES + 1);
        let ascii_json = json!({"sessions":[{"id":VALID_ID,"title":ascii,"state":"active"}]});
        let ascii_snapshot = ascii_json.clone();
        assert!(parse_rows(&ascii_json).is_err());
        assert_eq!(
            ascii_json, ascii_snapshot,
            "source json must not be mutated"
        );

        // UTF-8: 300 * 2-byte chars = 600 bytes (> 512), legal boundary chars
        let utf8 = "é".repeat(300);
        assert!(utf8.len() > MAX_TITLE_BYTES);
        let utf8_json = json!({"sessions":[{"id":VALID_ID,"title":utf8,"state":"active"}]});
        let utf8_snapshot = utf8_json.clone();
        assert!(parse_rows(&utf8_json).is_err());
        assert_eq!(utf8_json, utf8_snapshot);

        // control characters still rejected
        let ctrl_json = json!({"sessions":[{"id":VALID_ID,"title":"a\u{0007}b","state":"active"}]});
        assert!(parse_rows(&ctrl_json).is_err());

        // exact bound passes and is not truncated
        let exact = "b".repeat(MAX_TITLE_BYTES);
        let rows = parse_rows(&json!({"sessions":[
            {"id":VALID_ID,"title":exact,"state":"active"},
            {"id":UNLISTED,"title":"other","state":"active"}
        ]}))
        .unwrap();
        assert_eq!(rows[0].title.len(), MAX_TITLE_BYTES);
        assert_eq!(rows.len(), 2);
    }

    #[test]
    fn set_draft_unknown_id_refused_side_effect_free() {
        let mut v = view();
        v.set_draft(VALID_ID, "mine").unwrap();
        let before = v.drafts.len();
        let snapshot: Vec<(String, String)> = v.drafts.clone();

        // empty id
        assert!(v.set_draft("", "x").is_err());
        // non-empty junk id (not listed)
        assert!(v.set_draft("garbage-id", "x").is_err());
        // canonical-UUID id that is NOT in the current list
        assert!(v
            .set_draft("00000000-0000-4000-8000-000000000001", "x")
            .is_err());

        assert_eq!(v.drafts.len(), before, "draft count must be unchanged");
        assert_eq!(v.drafts, snapshot, "no draft record may be altered");
        assert_eq!(v.draft(VALID_ID), Some("mine"));

        // known id still works after the refusals
        assert!(v.set_draft(OTHER_ID, "two").is_ok());
        assert_eq!(v.draft(OTHER_ID), Some("two"));
        assert_eq!(v.draft(VALID_ID), Some("mine"));
    }

    #[test]
    fn removed_session_keeps_draft_but_new_updates_refused() {
        let mut v = view();
        v.set_draft(VALID_ID, "orphan").unwrap();
        // refresh no longer reports VALID_ID
        v.set_rows(
            vec![SessionRow {
                id: OTHER_ID.to_string(),
                title: "beta".into(),
            }],
            None,
        );
        // detached draft is preserved, never silently deleted
        assert_eq!(v.draft(VALID_ID), Some("orphan"));
        // but no NEW updates for the currently-unknown session
        let before = v.drafts.len();
        assert!(v.set_draft(VALID_ID, "update").is_err());
        assert_eq!(v.draft(VALID_ID), Some("orphan"));
        assert_eq!(v.drafts.len(), before);
        // still listed session accepts updates
        assert!(v.set_draft(OTHER_ID, "fresh").is_ok());
        assert_eq!(v.draft(OTHER_ID), Some("fresh"));
    }

    #[test]
    fn drafts_survive_refresh_and_selection() {
        let mut v = view();
        v.set_draft(VALID_ID, "keep").unwrap();
        let _ = v.select_next();
        v.set_rows(
            vec![
                SessionRow {
                    id: VALID_ID.to_string(),
                    title: "renamed".into(),
                },
                SessionRow {
                    id: OTHER_ID.to_string(),
                    title: "beta".into(),
                },
            ],
            Some(VALID_ID.to_owned()),
        );
        assert_eq!(v.draft(VALID_ID), Some("keep"));
    }

    #[test]
    fn malformed_and_archived_entries_rejected_or_skipped() {
        // non-canonical id -> Err, caller keeps last good list
        let bad = json!({"sessions":[{"id":"not-a-uuid","title":"x","state":"active"}]});
        assert!(parse_rows(&bad).is_err());

        // archived entries are skipped, not listed
        let arch = json!({"sessions":[
            {"id":VALID_ID,"title":"live","state":"active"},
            {"id":OTHER_ID,"title":"old","state":"archived"}
        ]});
        let rows = parse_rows(&arch).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].id, VALID_ID);

        // entry without id -> Err
        let noid = json!({"sessions":[{"title":"x"}]});
        assert!(parse_rows(&noid).is_err());
    }

    #[test]
    fn oversized_payload_rejected_not_truncated() {
        let entries: Vec<Value> = (0..(MAX_ROWS + 1))
            .map(|_| json!({"id": VALID_ID, "title": "t", "state": "active"}))
            .collect();
        let over = json!({"sessions": entries});
        assert!(parse_rows(&over).is_err());
    }

    #[test]
    fn window_bounded_at_5k_and_render_clips_to_height() {
        let entries: Vec<Value> = (0..MAX_ROWS)
            .map(|_| json!({"id": VALID_ID, "title": "t", "state": "active"}))
            .collect();
        let payload = json!({"sessions": entries});
        // 5k sent: only MAX_ROWS+1 tests; here 5000 > 256 -> Err
        let big: Vec<Value> = (0..5000)
            .map(|_| json!({"id": VALID_ID, "title": "t"}))
            .collect();
        assert!(parse_rows(&json!({"sessions": big})).is_err());
        assert_eq!(parse_rows(&payload).unwrap().len(), MAX_ROWS);

        let mut v = SessionView::default();
        // Fixture fix: distinct canonical UUIDs per row so the 256 entries are
        // not de-duplicated by id inside set_rows (reviewer NEEDS_FIX).
        let rows: Vec<SessionRow> = (0..MAX_ROWS)
            .map(|i| SessionRow {
                id: format!("00000000-0000-4000-8000-{:012x}", i),
                title: format!("row {i}"),
            })
            .collect();
        v.set_rows(rows, None);
        let lines = v.render(20, 10);
        assert_eq!(lines.len(), 10);
        for line in &lines {
            assert!(line.chars().count() <= 20);
        }
        // empty viewport is empty
        assert!(v.render(20, 0).is_empty());
    }

    #[test]
    fn empty_list_never_fakes_active() {
        let mut v = SessionView::default();
        assert_eq!(v.select_next(), None);
        assert_eq!(v.select_prev(), None);
        assert_eq!(v.active_id(), None);
        assert_eq!(v.pending_id(), None);
        let lines = v.render(40, 4);
        assert_eq!(lines[0], "Sessions");
        assert!(!lines.iter().any(|l| l.contains('*')));
        // refresh that loses the active id: active follows backend (None)
        v.set_rows(vec![], None);
        assert_eq!(v.active_id(), None);
    }

    #[test]
    fn selection_kept_by_id_across_refresh_else_clamped() {
        let mut v = view();
        let _ = v.select_next(); // highlight OTHER_ID
        v.set_rows(
            vec![SessionRow {
                id: VALID_ID.to_string(),
                title: "a".into(),
            }],
            None,
        );
        assert_eq!(v.selected_id(), Some(VALID_ID));
        v.set_rows(
            vec![
                SessionRow {
                    id: VALID_ID.to_string(),
                    title: "a".into(),
                },
                SessionRow {
                    id: OTHER_ID.to_string(),
                    title: "b".into(),
                },
            ],
            None,
        );
        assert_eq!(v.selected_id(), Some(VALID_ID));
    }

    #[test]
    fn search_is_case_insensitive_and_bounded() {
        let mut v = view();
        assert_eq!(v.search("BETA"), Some(OTHER_ID.to_string()));
        assert_eq!(v.search("zzz"), None);
        v.search("");
        assert_eq!(v.filtered_ids().len(), 2);
    }

    #[test]
    fn render_marks_active_selected_pending() {
        let mut v = view();
        v.acknowledge_active(VALID_ID);
        let _ = v.select_next(); // OTHER_ID pending
        let lines = v.render(40, 8);
        assert!(lines.iter().any(|l| l.starts_with('*')));
        assert!(lines.iter().any(|l| l.starts_with('>')));
        // pending id is never shown as active
        let pending_line = lines.iter().find(|l| l.contains("(pending)")).unwrap();
        assert!(!pending_line.starts_with('*'));
    }

    #[test]
    fn tabs_use_backend_titles_and_dedup() {
        let mut v = SessionView::default();
        v.set_rows(
            vec![SessionRow {
                id: VALID_ID.to_string(),
                title: "alpha".into(),
            }],
            None,
        );
        v.acknowledge_active(VALID_ID);
        let tabs = v.render_tabs(80, 3);
        assert_eq!(tabs.len(), 1);
        assert!(tabs[0].contains("alpha"));
        // re-acknowledging the same id does not duplicate a tab
        v.acknowledge_active(VALID_ID);
        v.acknowledge_active(VALID_ID);
        assert_eq!(v.render_tabs(80, 3)[0], "alpha");
        // unknown-title fallback uses the real id, never a fictional name
        let mut v2 = SessionView::default();
        v2.set_rows(
            vec![SessionRow {
                id: VALID_ID.to_string(),
                title: String::new(),
            }],
            None,
        );
        v2.acknowledge_active(VALID_ID);
        assert!(v2.render_tabs(80, 1)[0].contains(VALID_ID));
    }
}
