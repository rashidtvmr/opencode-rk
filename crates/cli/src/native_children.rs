//! Native child-task roster and typed history viewer. The caller owns HTTP;
//! this component emits explicit controls and never invents lifecycle state.
use serde_json::Value;
use opencode_rk_contracts::SessionId;
use std::hash::{Hash, Hasher};
use crate::native_view_chat::{ChatMessage, ChatView};
use crate::native_view_layout::fit;

const MAX_LIST_ROWS: usize = 5;

#[derive(Default)]
pub struct Panel {
    pub open: bool,
    pub parent: String,
    rows: Vec<Value>,
    selected: usize,
    pub detail: bool,
    pub composing: bool,
    input: String,
    pub error: Option<String>,
    pub action_error: Option<String>,
    history: Vec<ChatMessage>,
    chat: ChatView,
    roster_generation: u64,
    actions_generation: u64,
    lease_parent: Option<String>,
    lease_generation: Option<u64>,
    lease_auth: Option<u64>,
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    const IDS: [&str; 5] = [
        "11111111-1111-1111-1111-111111111111",
        "22222222-2222-2222-2222-222222222222",
        "33333333-3333-3333-3333-333333333333",
        "44444444-4444-4444-4444-444444444444",
        "55555555-5555-5555-5555-555555555555",
    ];
    fn panel() -> Panel {
        let mut panel = Panel::default();
        panel.show("parent");
        panel.update(&json!({"children":[{"session_id":"11111111-1111-1111-1111-111111111111","agent":"explore","state":"running","description":"inspect"}]})).unwrap();
        panel.bind_lease("parent", 1, Some("auth"));
        panel
    }
    fn roster(mut count: usize) -> Panel {
        count = count.min(IDS.len());
        let mut panel = Panel::default();
        panel.show("parent");
        let children: Vec<_> = IDS[..count].iter().enumerate().map(|(index, id)| json!({
            "session_id": id,
            "agent": format!("agent-{index}"),
            "state": "running",
            "description": format!("task {index}")
        })).collect();
        panel.update(&json!({ "children": children })).unwrap();
        panel.bind_lease("parent", 1, Some("auth"));
        panel
    }
    /// Clicks one footer region at the given geometry and reports the action.
    fn click_footer(panel: &mut Panel, hit: Hit, width: u16, height: usize) -> Option<Action> {
        let map = panel.hit_map(width, height);
        let region = map[height - 1].iter().find(|r| r.hit == hit).copied()?;
        panel.mouse(height as u16 - 1, (region.start + region.end) / 2, width, height)
    }
    #[test]
    fn successful_refresh_clears_stale_action_error_presentation() {
        let mut panel = panel();
        panel.action_error = Some("STALE_ACTION_FAILURE".into());
        panel.error = Some("STALE_ROSTER_FAILURE".into());
        assert!(panel.render(100, 24).join("\n").contains("STALE_ACTION_FAILURE"));
        panel.invalidate_actions();
        panel.update(&json!({"children":[{"session_id":IDS[0],"agent":"explore","state":"stopped","description":"inspect"}]})).unwrap();
        panel.bind_lease("parent", 1, Some("auth"));
        assert!(panel.action_error.is_none());
        assert!(panel.error.is_none());
        assert!(panel.lease_valid("parent", 1, Some("auth")));
        let rendered = panel.render(100, 24).join("\n");
        assert!(!rendered.contains("STALE_ACTION_FAILURE"));
        assert!(!rendered.contains("STALE_ROSTER_FAILURE"));
        assert!(rendered.contains("stopped"));
    }

    #[test]
    fn failed_refresh_invalidates_all_existing_child_actions() {
        let mut panel = panel();
        assert!(matches!(panel.key(b'o'), Some(Action::Open(_))));
        panel.invalidate_actions();
        assert!(panel.key(b'o').is_none());
        assert!(panel.key(b's').is_none());
        assert!(panel.key(b'r').is_none());
        panel.key(b'c');
        panel.text('x');
        assert!(panel.key(13).is_none());
    }

    #[test]
    fn lease_rejects_parent_generation_and_auth_context_changes() {
        let mut panel = panel();
        assert!(panel.lease_valid("parent", 1, Some("auth")));
        assert!(!panel.lease_valid("other", 1, Some("auth")));
        assert!(!panel.lease_valid("parent", 2, Some("auth")));
        assert!(!panel.lease_valid("parent", 1, Some("other")));
        panel.invalidate_actions();
        assert!(panel.key(b's').is_none());
    }

    #[test]
    fn action_epoch_invalidation_preserves_context_but_context_invalidation_requires_reopen() {
        let mut panel = panel();
        assert!(panel.context_valid("parent", 1, Some("auth")));
        assert!(panel.lease_valid("parent", 1, Some("auth")));
        panel.invalidate_actions();
        assert!(panel.context_valid("parent", 1, Some("auth")));
        assert!(!panel.lease_valid("parent", 1, Some("auth")));
        panel.update(&json!({"children":[{"session_id":IDS[0],"agent":"explore","state":"running","description":"inspect"}]})).unwrap();
        panel.bind_lease("parent", 1, Some("auth"));
        assert!(panel.lease_valid("parent", 1, Some("auth")));
        panel.invalidate_context();
        assert!(!panel.context_valid("parent", 1, Some("auth")));
        assert!(!panel.lease_valid("parent", 1, Some("auth")));
        panel.update(&json!({"children":[{"session_id":IDS[0],"agent":"explore","state":"running","description":"inspect"}]})).unwrap();
        assert!(!panel.context_valid("parent", 1, Some("auth")), "context cannot be rebound without reopen");
    }

    #[test]
    fn failed_child_detail_shows_its_error_and_clears_it_after_recovery() {
        let mut panel = panel();
        panel.update(&json!({"children":[{
            "session_id": IDS[0], "agent":"explore", "model":"fixture/model",
            "state":"error", "output_tail":"", "error":"permanent_child_failure"
        }]})).unwrap();
        panel.key(13);
        let rows = panel.render(100, 20);
        assert!(rows.iter().any(|line| line.contains("permanent_child_failure")),
            "the selected child's failure must be visible before Retry");
        assert!(rows.last().unwrap().contains("r retry"));
        panel.update(&json!({"children":[{
            "session_id": IDS[0], "agent":"explore", "model":"fixture/model",
            "state":"completed", "output_tail":"recovered child output", "error":null
        }]})).unwrap();
        let recovered = panel.render(100, 20).join("\n");
        assert!(recovered.contains("recovered child output"));
        assert!(!recovered.contains("permanent_child_failure"));
    }
    #[test]
    fn detail_render_surfaces_snapshot_metrics_and_labels_missing_fields_unreported() {
        let mut panel = Panel::default();
        panel.show("parent");
        panel.update(&json!({"children":[{
            "session_id": IDS[0], "agent":"build", "state":"running", "description":"work",
            "generation": 3, "auto_retry_attempts": 1, "manual_retries": 2,
            "pending_followups": 2, "read_only": true
        }]})).unwrap();
        assert!(panel.key(13).is_none());
        assert!(panel.detail);
        let rendered = panel.render(120, 20);
        assert_eq!(rendered.len(), 20);
        let text = rendered.join("\n");
        assert!(text.contains("generation 3"));
        assert!(text.contains("auto-retry 1"));
        assert!(text.contains("manual retries 2"));
        assert!(text.contains("pending 2/4"));
        assert!(text.contains("read-only yes"));
        assert!(!text.contains("unreported"));
        // Older daemons omit the snapshot fields; render unreported, never 0.
        panel.update(&json!({"children":[{
            "session_id": IDS[0], "agent":"build", "state":"running", "description":"work"
        }]})).unwrap();
        let text = panel.render(120, 20).join("\n");
        assert!(text.contains("generation unreported"));
        assert!(text.contains("auto-retry unreported"));
        assert!(text.contains("manual retries unreported"));
        assert!(text.contains("pending unreported"));
        assert!(text.contains("read-only unreported"));
    }
    #[test]
    fn child_panel_navigation_never_submits_and_controls_use_selected_identity() {
        let mut panel = panel();
        assert!(panel.key(14).is_none());
        assert!(matches!(panel.key(b's'), Some(Action::Stop(id)) if id == "11111111-1111-1111-1111-111111111111"));
        assert!(matches!(panel.key(b'r'), Some(Action::Retry(_))));
        assert!(panel.key(13).is_none());
        assert!(panel.detail);
        panel.key(b'c');
        for ch in "new instruction".chars() { panel.text(ch); }
        assert!(matches!(panel.key(13), Some(Action::Followup(_, text)) if text == "new instruction"));
        panel.key(27);
        assert!(!panel.detail && panel.open);
        panel.key(27);
        assert!(!panel.open);
    }
    #[test]
    fn daemon_text_is_sanitized_and_successful_refresh_clears_error() {
        let mut panel = panel();
        panel.error = Some("\x1b[31mfailure\nnew row".into());
        let rows = panel.render(60,20);
        assert!(rows.iter().all(|line| !line.chars().any(char::is_control)));
        panel.update(&json!({"children":[]})).unwrap();
        assert!(panel.error.is_none());
        for height in 0..8 {
            let rows = panel.render(20,height);
            assert!(rows.len() <= height);
            assert!(rows.iter().all(|line| !line.chars().any(char::is_control)));
        }
    }
    #[test]
    fn numeric_keys_select_visible_children_without_submitting() {
        let mut panel = roster(2);
        assert!(panel.key(b'2').is_none());
        assert_eq!(panel.selected_id().as_deref(), Some(IDS[1]));
        assert!(!panel.detail);
        // Out-of-range digits keep the current selection; digits are inert
        // while the typed-history view is open.
        assert!(panel.key(b'9').is_none());
        assert_eq!(panel.selected_id().as_deref(), Some(IDS[1]));
        panel.key(13);
        assert!(panel.detail);
        assert!(panel.key(b'1').is_none());
        assert_eq!(panel.selected_id().as_deref(), Some(IDS[1]));
    }
    #[test]
    fn mouse_clicks_select_children_and_footer_buttons_reuse_key_actions() {
        let (width, height) = (64u16, 20usize);
        let mut panel = roster(2);
        // Header occupies rows 0-1; children render from row 2. Clicking the
        // already-selected row opens the detail view (Enter parity).
        assert!(panel.mouse(2, 0, width, height).is_none());
        assert!(panel.detail);
        assert_eq!(panel.selected_id().as_deref(), Some(IDS[0]));
        panel.key(27);
        assert!(!panel.detail);
        // Clicking another row selects it; clicking it again opens detail.
        assert!(panel.mouse(3, 0, width, height).is_none());
        assert_eq!(panel.selected_id().as_deref(), Some(IDS[1]));
        assert!(panel.mouse(3, 7, width, height).is_none());
        assert!(panel.detail);
        panel.key(27);
        // List footer buttons dispatch the same actions as their keys.
        let action = click_footer(&mut panel, Hit::Stop, width, height);
        assert!(matches!(action, Some(Action::Stop(id)) if id == IDS[1]));
        let action = click_footer(&mut panel, Hit::Retry, width, height);
        assert!(matches!(action, Some(Action::Retry(id)) if id == IDS[1]));
        assert!(click_footer(&mut panel, Hit::Close, width, height).is_none());
        assert!(!panel.open);
    }
    #[test]
    fn footer_followup_and_send_buttons_reuse_the_composer() {
        let (width, height) = (64u16, 20usize);
        let mut panel = roster(2);
        panel.key(13); // open detail for the selected child
        assert!(click_footer(&mut panel, Hit::Followup, width, height).is_none());
        assert!(panel.composing);
        for ch in "ship it".chars() { panel.text(ch); }
        let action = click_footer(&mut panel, Hit::Send, width, height);
        assert!(matches!(action, Some(Action::Followup(id, text)) if id == IDS[0] && text == "ship it"));
        assert!(!panel.composing);
    }
    #[test]
    fn hit_map_matches_rendered_rows_in_short_viewports() {
        for count in [5usize, 1] {
            for height in 0..=8usize {
                let mut panel = roster(count);
                let map = panel.hit_map(64, height);
                assert_eq!(map.len(), height);
                let rendered = panel.render(64, height);
                assert!(rendered.len() <= height);
                let body_limit = height.saturating_sub(2).max(1);
                for (row, regions) in map.iter().enumerate() {
                    for region in regions {
                        match region.hit {
                            Hit::Child(index) => {
                                assert!(index < count, "hit row {row} references absent child");
                                assert!(row >= 2 && row < body_limit, "child hit at unexpected row {row}");
                                let action = panel.mouse(row as u16, region.start, 64, height);
                                if panel.detail { panel.key(27); }
                                assert!(action.is_none(), "child click must not submit");
                                assert_eq!(panel.selected_id().as_deref(), Some(IDS[index]));
                            }
                            Hit::Stop | Hit::Retry | Hit::Open | Hit::Followup | Hit::Send | Hit::Back | Hit::Close => {
                                assert!(row == height - 1 && height > 1, "footer hit off the footer row {row}");
                                assert!(rendered[row].contains("stop")
                                    || rendered[row].contains("Esc")
                                    || rendered[row].contains("Enter"));
                            }
                        }
                    }
                }
            }
        }
    }
    /// The footer must occupy the viewport's last row even when the child list
    /// is shorter than the available space; blank rows pad the gap instead of
    /// letting the footer drift upward.
    #[test]
    fn short_lists_pad_to_the_bottom_and_footer_hits_follow_rendered_rows() {
        for count in [0usize, 1, 2, 5] {
            for height in 0..=8usize {
                let mut panel = roster(count);
                let rendered = panel.render(64, height);
                let map = panel.hit_map(64, height);
                assert_eq!(map.len(), height);
                assert_eq!(rendered.len(), height);
                if height == 1 {
                    assert!(rendered[0].contains("Subagents"));
                }
                if height > 1 {
                    assert!(rendered[height - 1].contains("stop") || rendered[height - 1].contains("Esc"),
                        "footer must be the last row at height {height}");
                    for (row, regions) in map.iter().enumerate() {
                        for region in regions {
                            match region.hit {
                                Hit::Child(index) => {
                                    assert!(row < height - 1, "child hit on the footer row");
                                    assert!(rendered[row].contains(&format!("task {index}")),
                                        "hit row {row} does not render child {index}");
                                    let action = panel.mouse(row as u16, region.start, 64, height);
                                    if panel.detail { panel.key(27); }
                                    assert!(action.is_none());
                                    assert_eq!(panel.selected_id().as_deref(), Some(IDS[index]));
                                }
                                _ => assert_eq!(row, height - 1, "footer hit off the footer row"),
                            }
                        }
                    }
                }
            }
        }
    }
    /// Hits never extend past the viewport and out-of-bounds columns are
    /// rejected, so a footer label clipped by a narrow terminal cannot fire.
    #[test]
    fn narrow_viewports_never_expose_or_dispatch_off_screen_hits() {
        for width in 0..=40u16 {
            for height in 0..=8usize {
                let mut panel = roster(5);
                let map = panel.hit_map(width, height);
                let rendered = panel.render(width, height);
                assert_eq!(map.len(), height);
                assert_eq!(rendered.len(), height);
                for (row, regions) in map.iter().enumerate() {
                    for region in regions {
                        assert!(region.start <= region.end);
                        assert!(region.end < width, "{region:?} escapes width {width}");
                        assert!(panel.mouse(row as u16, width.saturating_add(2), width, height).is_none());
                    }
                }
                assert!(panel.mouse(height as u16, 0, width, height).is_none());
            }
        }
    }
    /// A partially visible footer label is not a target; only fully visible
    /// labels dispatch, while earlier fully visible labels still work.
    #[test]
    fn clipped_footer_labels_cannot_execute() {
        let height = 8usize;
        let mut panel = roster(2);
        // Width 40 shows "s stop" but clips "r retry" and "Esc close".
        let footer = panel.hit_map(40, height);
        let footer = &footer[height - 1];
        assert!(footer.iter().any(|r| r.hit == Hit::Stop));
        assert!(!footer.iter().any(|r| r.hit == Hit::Retry));
        assert!(!footer.iter().any(|r| r.hit == Hit::Close));
        // Column 39 lies inside "r retry" as laid out, but that label is clipped.
        assert!(panel.mouse(height as u16 - 1, 39, 40, height).is_none());
        assert!(panel.open);
        assert_eq!(panel.selected_id().as_deref(), Some(IDS[0]));
        // Width 30 clips "s stop" too, so the footer offers no targets at all.
        assert!(panel.hit_map(30, height)[height - 1].is_empty());
        assert!(panel.mouse(height as u16 - 1, 27, 30, height).is_none());
        assert!(panel.open);
        // The fully visible Stop label still dispatches.
        let stop = panel.hit_map(40, height)[height - 1].iter()
            .find(|r| r.hit == Hit::Stop).copied().expect("visible stop");
        assert!(matches!(panel.mouse(height as u16 - 1, stop.start, 40, height), Some(Action::Stop(_))));
    }
    #[test]
    fn subagent_rows_cap_at_five_and_running_is_indeterminate_literal() {
        let mut panel = roster(5);
        let rendered = panel.render(100, 32).join("\n");
        assert_eq!(rendered.matches("[Running]").count(), 5);
        assert!(!rendered.contains("%"));
        assert!(!rendered.contains("progress"));
        assert!(rendered.contains("task 4"));
    }

    #[test]
    fn child_hierarchy_prefix_comes_only_from_daemon_parent_ids() {
        let mut panel = Panel::default();
        panel.show("parent");
        panel.update(&json!({"children":[
            {"session_id":IDS[0],"parent_id":"parent","agent":"build","state":"running","description":"root child"},
            {"session_id":IDS[1],"parent_id":IDS[0],"agent":"explore","state":"running","description":"nested child"}
        ]})).unwrap();
        let lines = panel.render(100, 12);
        assert!(lines.iter().any(|line| line.contains("├─ build")));
        assert!(lines.iter().any(|line| line.contains("│ └─ explore")));
        // Presentation never invents an id or issues a daemon call.
        assert_eq!(panel.selected_id().as_deref(), Some(IDS[0]));
    }

    #[test]
    fn five_row_window_and_hit_map_share_the_same_visible_indices() {
        let mut panel = roster(5);
        let map = panel.hit_map(100, 32);
        let child_hits: Vec<_> = map.iter().flat_map(|row| row.iter()).filter_map(|hit| match hit.hit {
            Hit::Child(index) => Some(index), _ => None,
        }).collect();
        assert_eq!(child_hits, vec![0, 1, 2, 3, 4]);
        panel.key(14);
        panel.key(14);
        panel.key(14);
        panel.key(14);
        let rendered = panel.render(100, 8);
        let map = panel.hit_map(100, 8);
        let child_hits: Vec<_> = map.iter().flat_map(|row| row.iter()).filter_map(|hit| match hit.hit {
            Hit::Child(index) => Some(index), _ => None,
        }).collect();
        assert_eq!(child_hits, vec![1, 2, 3, 4]);
        assert!(rendered.iter().any(|line| line.contains("task 4")));
    }

    #[test]
    fn metadata_fallback_does_not_create_child_identity() {
        let mut panel = Panel::default();
        panel.show("parent");
        panel.update(&json!({"children":[{"session_id":IDS[0],"state":"running",
            "metadata":{"agent":"reviewer","description":"metadata task"}}]})).unwrap();
        let rendered = panel.render(100, 8).join("\n");
        assert!(rendered.contains("reviewer"));
        assert!(rendered.contains("metadata task"));
        assert_eq!(panel.selected_id().as_deref(), Some(IDS[0]));
    }

    #[test]
    fn untrusted_daemon_text_cannot_shift_child_hit_rows() {
        let mut panel = Panel::default();
        panel.show("parent");
        panel.update(&json!({"children":[{
            "session_id":"11111111-1111-1111-1111-111111111111",
            "agent":"evil\x1b[2;1H","state":"running","description":"click\nattack"
        }]})).unwrap();
        panel.error = Some("\x1b[31mboom\nboom".into());
        let map = panel.hit_map(40, 20);
        // The error consumes header row 2; the child stays on row 3 no matter
        // what control sequences or newlines the daemon supplied.
        assert!(map[2].is_empty());
        assert!(map[3].iter().any(|r| r.hit == Hit::Child(0)));
        let rendered = panel.render(40, 20);
        assert!(rendered.iter().all(|line| !line.chars().any(char::is_control)));
    }
}

#[derive(Debug)]
pub enum Action {
    /// Open the selected child as the active authenticated session route.
    Open(String),
    Stop(String),
    Retry(String),
    Followup(String, String),
}

/// Visible screen targets the pointer can land on. Row clicks are addressed
/// by rendered child index; footer labels reuse their keyboard equivalents.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Hit { Child(usize), Stop, Retry, Open, Followup, Send, Back, Close }

/// One clickable column range on one rendered row (row = hit-map index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitRegion { pub hit: Hit, pub start: u16, pub end: u16 }

/// Adds column ranges for each `label` found in the footer text. Footer
/// segments are ASCII and every prefix glyph (arrows, dots) is one cell wide,
/// so the display column of a segment equals its char count.
fn footer_buttons(map: &mut Vec<HitRegion>, footer: &str, labels: &[(&str, Hit)]) {
    for (label, hit) in labels {
        if let Some(byte) = footer.find(label) {
            let start = footer[..byte].chars().count() as u16;
            map.push(HitRegion {
                hit: *hit,
                start: start.saturating_sub(1),
                end: start.saturating_add(label.chars().count() as u16).saturating_add(1),
            });
        }
    }
}

impl Panel {
    pub fn show(&mut self, parent: &str) {
        *self = Self { open:true, parent:parent.into(), ..Self::default() };
    }
    /// Invalidate controls while a refresh is unavailable or malformed.
    /// Invalidate only the current roster/action epoch. The bound context remains
    /// usable so a failed action can be followed by a legitimate refresh.
    pub fn invalidate_actions(&mut self) { self.actions_generation = self.actions_generation.wrapping_add(1); }
    /// Invalidate the context lease itself; reopening the panel is required.
    pub fn invalidate_context(&mut self) { self.lease_parent = None; self.lease_generation = None; self.lease_auth = None; self.invalidate_actions(); }
    fn auth_identity(auth: Option<&str>) -> Option<u64> {
        auth.map(|value| { let mut hasher = std::collections::hash_map::DefaultHasher::new(); value.hash(&mut hasher); hasher.finish() })
    }
    pub fn bind_lease(&mut self, parent: &str, generation: u64, auth: Option<&str>) { self.lease_parent = Some(parent.to_owned()); self.lease_generation = Some(generation); self.lease_auth = Self::auth_identity(auth); self.actions_generation = self.roster_generation; }
    pub fn lease_valid(&self, parent: &str, generation: u64, auth: Option<&str>) -> bool { self.actions_valid() && self.lease_parent.as_deref() == Some(parent) && self.lease_generation == Some(generation) && self.lease_auth == Self::auth_identity(auth) }
    pub fn context_valid(&self, parent: &str, generation: u64, auth: Option<&str>) -> bool { self.lease_parent.as_deref() == Some(parent) && self.lease_generation == Some(generation) && self.lease_auth == Self::auth_identity(auth) }
    pub fn actions_valid(&self) -> bool { self.actions_generation == self.roster_generation }
    fn action_id(&self) -> Option<String> {
        if !self.actions_valid() { return None; }
        self.selected_id()
    }
    pub fn update(&mut self, value: &Value) -> Result<(), String> {
        let rows = value["children"].as_array().ok_or("invalid child listing")?;
        if rows.len() > 128 { return Err("child listing exceeds limit".into()); }
        let selected = self.selected_id();
        for row in rows {
            let id = row["session_id"].as_str().ok_or("child id missing")?;
            id.parse::<SessionId>().map_err(|_| "invalid child id".to_owned())?;
        }
        self.rows = rows.clone();
        self.roster_generation = self.roster_generation.wrapping_add(1);
        self.actions_generation = self.roster_generation;
        self.selected = selected.and_then(|id| self.rows.iter().position(|row| row["session_id"] == id)).unwrap_or(self.selected).min(self.rows.len().saturating_sub(1));
        self.error = None;
        self.action_error = None;
        Ok(())
    }
    pub fn history(&mut self, value: &Value) {
        if let Some(messages) = value["messages"].as_array() {
            self.history = crate::native_chat_projection::project_history(messages);
        }
    }
    pub fn selected_id(&self) -> Option<String> {
        self.rows.get(self.selected)?.get("session_id").and_then(Value::as_str).map(str::to_owned)
    }

    fn row_text<'a>(row: &'a Value, key: &str, fallback: &'a str) -> &'a str {
        row.get(key).and_then(Value::as_str).filter(|value| !value.trim().is_empty())
            .or_else(|| row.get("metadata").and_then(|metadata| metadata.get(key)).and_then(Value::as_str).filter(|value| !value.trim().is_empty()))
            .unwrap_or(fallback)
    }

    /// Returns the source Composer's family prefix for a daemon-provided row.
    /// The panel never discovers or creates sessions: `parent_id` and the row
    /// set are authoritative inputs from the existing children endpoint.
    fn family_prefix(&self, index: usize) -> String {
        let Some(row) = self.rows.get(index) else { return String::new(); };
        let Some(parent) = row.get("parent_id").and_then(Value::as_str) else {
            return if index + 1 == self.rows.len() { "└─" } else { "├─" }.to_owned();
        };
        let mut depth = 0usize;
        let mut current = parent;
        let mut seen = std::collections::HashSet::new();
        while current != self.parent && depth < self.rows.len() && seen.insert(current) {
            let Some(parent_row) = self.rows.iter().find(|candidate|
                candidate.get("session_id").and_then(Value::as_str) == Some(current)) else { break; };
            current = parent_row.get("parent_id").and_then(Value::as_str).unwrap_or(self.parent.as_str());
            depth += 1;
        }
        let last = index + 1 == self.rows.len();
        let branch = if last { "└─" } else { "├─" };
        if depth == 0 { branch.to_owned() } else { format!("{}{}", "│ ".repeat(depth), branch) }
    }

    fn list_window(&self, height: usize, error_row: bool) -> (usize, usize) {
        let top = 2 + usize::from(error_row);
        let capacity = height.saturating_sub(top + 2).min(MAX_LIST_ROWS);
        let start = self.selected.saturating_sub(capacity.saturating_sub(1));
        (start, capacity)
    }
    pub fn text(&mut self, character: char) {
        if self.composing && !character.is_control() && self.input.len() + character.len_utf8() <= 32768 { self.input.push(character); }
    }
    pub fn key(&mut self, byte: u8) -> Option<Action> {
        if self.composing {
            match byte {
                27 => { self.composing = false; self.input.clear(); }
                8 | 127 => { self.input.pop(); }
                13 | 10 if !self.input.trim().is_empty() => {
                    let id = self.action_id()?;
                    self.composing = false;
                    return Some(Action::Followup(id, std::mem::take(&mut self.input)));
                }
                _ => {}
            }
            return None;
        }
        match byte {
            27 if self.detail => { self.detail = false; self.history.clear(); }
            27 => self.open = false,
            14 if self.detail => self.chat.scroll_down(3),
            2 if self.detail => self.chat.scroll_up(3),
            14 => self.selected = self.selected.saturating_add(1).min(self.rows.len().saturating_sub(1)),
            2 => self.selected = self.selected.saturating_sub(1),
            13 | 10 if !self.rows.is_empty() => { self.detail = true; self.chat = ChatView::default(); }
            b'o' => return self.action_id().map(Action::Open),
            b's' => return self.action_id().map(Action::Stop),
            b'r' => return self.action_id().map(Action::Retry),
            b'c' if !self.rows.is_empty() => { self.detail = true; self.composing = true; }
            b'1'..=b'9' => {
                // Numeric child selection mirrors the pointer row click; the
                // detail view keeps its own scroll keys, so digits are inert there.
                if !self.detail {
                    let index = usize::from(byte - b'1');
                    if index < self.rows.len() { self.selected = index; }
                }
            }
            _ => {}
        }
        None
    }
    /// Interactive target per rendered row, indexed by the local offset from
    /// the modal content origin. Computed with the same layout arithmetic as
    /// [`Self::render`] so hits always follow the visible rows, including
    /// short viewports where the list window is clipped.
    pub fn hit_map(&self, width: u16, height: usize) -> Vec<Vec<HitRegion>> {
        let mut map: Vec<Vec<HitRegion>> = vec![Vec::new(); height];
        if width == 0 || height == 0 { return map; }
        if height > 1 {
            let footer = if self.composing { "Enter send follow-up · Esc cancel" }
                else if self.detail { "↑/↓ scroll · s stop · r retry · c follow-up · Esc back" }
                else { "↑/↓ select · Enter detail · s stop · r retry · Esc close · o open" };
            let labels: &[(&str, Hit)] = if self.composing {
                &[("Enter send follow-up", Hit::Send), ("Esc cancel", Hit::Back)]
            } else if self.detail {
                &[("s stop", Hit::Stop), ("r retry", Hit::Retry), ("c follow-up", Hit::Followup), ("Esc back", Hit::Back)]
            } else {
                &[("s stop", Hit::Stop), ("r retry", Hit::Retry), ("Esc close", Hit::Close), ("o open", Hit::Open)]
            };
            let mut buttons = Vec::new();
            footer_buttons(&mut buttons, footer, labels);
            // Only fully visible labels are clickable; a label clipped by the
            // right edge must not fire from its visible prefix.
            buttons.retain(|region| region.end < width);
            map[height - 1] = buttons;
        }
        if !self.detail && !self.composing && !self.rows.is_empty() {
            // Mirror the list branch of render: two header rows, an optional
            // error row, and the bottom footer row stay unclickable.
            let top = 2 + usize::from(self.error.is_some() || self.action_error.is_some());
            let (start, capacity) = self.list_window(height, self.error.is_some() || self.action_error.is_some());
            if capacity > 0 {
                let limit = height.saturating_sub(2).max(1);
                for (index, _) in self.rows.iter().enumerate().skip(start).take(capacity) {
                    let line = top + index - start;
                    if line < limit {
                        if let Some(row) = map.get_mut(line) {
                            row.push(HitRegion { hit: Hit::Child(index), start: 0, end: width.saturating_sub(1) });
                        }
                    }
                }
            }
        }
        map
    }
    /// Mouse press inside the children modal. `row` is the local row offset
    /// from the modal content origin and `column` the local column offset.
    /// Row clicks mirror arrow selection (a second click on the selected row
    /// opens the detail view); footer clicks mirror their keyboard actions.
    pub fn mouse(&mut self, row: u16, column: u16, width: u16, height: usize) -> Option<Action> {
        // Reject coordinates outside the viewport before consulting the map.
        if column >= width { return None; }
        let hit_map = self.hit_map(width, height);
        let regions = hit_map.get(usize::from(row))?;
        let hit = regions.iter().find(|r| column >= r.start && column <= r.end).map(|r| r.hit)?;
        self.mouse_from_hit(hit)
    }
    fn mouse_from_hit(&mut self, hit: Hit) -> Option<Action> {
        match hit {
            Hit::Child(index) => {
                if self.selected == index {
                    if !self.detail {
                        self.detail = true;
                        self.chat = ChatView::default();
                    }
                } else if index < self.rows.len() {
                    self.selected = index;
                }
                None
            }
            Hit::Stop => self.key(b's'),
            Hit::Open => self.key(b'o'),
            Hit::Retry => self.key(b'r'),
            Hit::Followup => { self.key(b'c'); None }
            Hit::Send => self.key(13),
            Hit::Back | Hit::Close => self.key(27),
        }
    }
    pub fn render(&self, width: u16, height: usize) -> Vec<String> {
        if height == 0 { return Vec::new(); }
        let mut lines = vec!["Subagents".into(), String::new()];
        let footer = if self.composing { "Enter send follow-up · Esc cancel" }
            else if self.detail { "↑/↓ scroll · s stop · r retry · c follow-up · Esc back" }
            else { "↑/↓ select · Enter detail · s stop · r retry · Esc close · o open" };
        if let Some(error) = self.action_error.as_ref().or(self.error.as_ref()) { lines.push(format!("Error: {error}")); }
        if self.detail {
            if let Some(row) = self.rows.get(self.selected) {
                lines.push(format!("{} · {} · {}", row["agent"].as_str().unwrap_or("?"), row["state"].as_str().unwrap_or("unknown"), row["model"].as_str().unwrap_or("?")));
                lines.push(format!("Session: {}", row["session_id"].as_str().unwrap_or("?")));
                if let Some(error) = row["error"].as_str().filter(|error| !error.trim().is_empty()) {
                    lines.push(format!("Error: {error}"));
                }
                let metric = |key: &str| row.get(key).and_then(Value::as_u64)
                    .map_or_else(|| "unreported".to_owned(), |value| value.to_string());
                let pending = row.get("pending_followups").and_then(Value::as_u64)
                    .map_or_else(|| "unreported".to_owned(), |value| format!("{value}/4"));
                let read_only = row.get("read_only").and_then(Value::as_bool)
                    .map_or("unreported", |value| if value { "yes" } else { "no" });
                lines.push(format!("generation {} · auto-retry {} · manual retries {} · pending {} · read-only {}",
                    metric("generation"), metric("auto_retry_attempts"), metric("manual_retries"), pending, read_only));
                lines.push(String::new());
                let budget = height.saturating_sub(lines.len() + 3);
                if self.history.is_empty() {
                    lines.extend(row["output_tail"].as_str().unwrap_or("Waiting for child output…").lines().take(budget).map(str::to_owned));
                } else {
                    lines.extend(self.chat.render_typed(usize::from(width), budget + 2, "Child history", &self.history, "", false).into_iter().skip(1).take(budget));
                }
            }
            if self.composing { lines.push(format!("Follow-up: {}▏", self.input)); }
        } else if self.rows.is_empty() {
            lines.push("No child tasks in this daemon session".into());
            lines.push("Ask the agent to delegate using the task tool.".into());
        } else {
            let error_row = self.error.is_some() || self.action_error.is_some();
            let (start, capacity) = self.list_window(height, error_row);
            for (index, row) in self.rows.iter().enumerate().skip(start).take(capacity) {
                let state = if row["awaiting_permission"] == true {
                    "approval needed"
                } else if row["state"].as_str() == Some("running") {
                    // Upstream Composer C08 uses a literal Running label: no
                    // percentage or determinate/fake progress is available.
                    "Running"
                } else {
                    row["state"].as_str().unwrap_or("unknown")
                };
                let agent = Self::row_text(row, "agent", "Subagent");
                let description = Self::row_text(row, "description", "?");
                lines.push(format!("{} [{}] {} {} — {}", if index == self.selected { ">" } else { " " },
                    state, self.family_prefix(index), agent, description));
            }
        }
        lines.truncate(height.saturating_sub(2).max(1));
        if height > 1 {
            // Reserve the viewport's last row for the footer. Short lists pad
            // with blank rows instead of letting the footer float upward.
            lines.resize(height - 1, String::new());
            lines.push(footer.into());
        }
        // fit strips ANSI/control sequences with a bounded scan and clips by
        // terminal cells; daemon text never goes to the renderer unfiltered.
        lines.into_iter().take(height).map(|line| fit(&line, width)).collect()
    }
}
