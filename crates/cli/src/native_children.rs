//! Native child-task roster and typed history viewer. The caller owns HTTP;
//! this component emits explicit controls and never invents lifecycle state.
use crate::native_view_chat::{ChatMessage, ChatView};
use crate::native_view_layout::fit;
use opencode_rk_contracts::SessionId;
use serde_json::Value;
use std::hash::{Hash, Hasher};

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
        let children: Vec<_> = IDS[..count]
            .iter()
            .enumerate()
            .map(|(index, id)| {
                json!({
                    "session_id": id,
                    "agent": format!("agent-{index}"),
                    "state": "running",
                    "description": format!("task {index}")
                })
            })
            .collect();
        panel.update(&json!({ "children": children })).unwrap();
        panel.bind_lease("parent", 1, Some("auth"));
        panel
    }
    /// Clicks one footer region at the given geometry and reports the action.
    fn click_footer(panel: &mut Panel, hit: Hit, width: u16, height: usize) -> Option<Action> {
        let map = panel.hit_map(width, height);
        let region = map[height - 1].iter().find(|r| r.hit == hit).copied()?;
        panel.mouse(
            height as u16 - 1,
            (region.start + region.end) / 2,
            width,
            height,
        )
    }
    #[test]
    fn successful_refresh_clears_stale_action_error_presentation() {
        let mut panel = panel();
        panel.action_error = Some("STALE_ACTION_FAILURE".into());
        panel.error = Some("STALE_ROSTER_FAILURE".into());
        assert!(panel
            .render(100, 24)
            .join("\n")
            .contains("STALE_ACTION_FAILURE"));
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
        assert!(
            !panel.context_valid("parent", 1, Some("auth")),
            "context cannot be rebound without reopen"
        );
    }

    #[test]
    fn failed_child_detail_shows_its_error_and_clears_it_after_recovery() {
        let mut panel = panel();
        panel
            .update(&json!({"children":[{
                "session_id": IDS[0], "agent":"explore", "model":"fixture/model",
                "state":"error", "output_tail":"", "error":"permanent_child_failure"
            }]}))
            .unwrap();
        panel.key(13);
        let rows = panel.render(100, 20);
        assert!(
            rows.iter()
                .any(|line| line.contains("permanent_child_failure")),
            "the selected child's failure must be visible before Retry"
        );
        assert!(rows.last().unwrap().contains("r retry"));
        panel
            .update(&json!({"children":[{
                "session_id": IDS[0], "agent":"explore", "model":"fixture/model",
                "state":"completed", "output_tail":"recovered child output", "error":null
            }]}))
            .unwrap();
        let recovered = panel.render(100, 20).join("\n");
        assert!(recovered.contains("recovered child output"));
        assert!(!recovered.contains("permanent_child_failure"));
    }
    #[test]
    fn detail_render_surfaces_snapshot_metrics_and_labels_missing_fields_unreported() {
        let mut panel = Panel::default();
        panel.show("parent");
        panel
            .update(&json!({"children":[{
                "session_id": IDS[0], "agent":"build", "state":"running", "description":"work",
                "generation": 3, "auto_retry_attempts": 1, "manual_retries": 2,
                "pending_followups": 2, "read_only": true
            }]}))
            .unwrap();
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
        panel
            .update(&json!({"children":[{
                "session_id": IDS[0], "agent":"build", "state":"running", "description":"work"
            }]}))
            .unwrap();
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
        assert!(
            matches!(panel.key(b's'), Some(Action::Stop(id)) if id == "11111111-1111-1111-1111-111111111111")
        );
        assert!(matches!(panel.key(b'r'), Some(Action::Retry(_))));
        assert!(panel.key(13).is_none());
        assert!(panel.detail);
        panel.key(b'c');
        for ch in "new instruction".chars() {
            panel.text(ch);
        }
        assert!(
            matches!(panel.key(13), Some(Action::Followup(_, text)) if text == "new instruction")
        );
        panel.key(27);
        assert!(!panel.detail && panel.open);
        panel.key(27);
        assert!(!panel.open);
    }
    #[test]
    fn daemon_text_is_sanitized_and_successful_refresh_clears_error() {
        let mut panel = panel();
        panel.error = Some("\x1b[31mfailure\nnew row".into());
        let rows = panel.render(60, 20);
        assert!(rows.iter().all(|line| !line.chars().any(char::is_control)));
        panel.update(&json!({"children":[]})).unwrap();
        assert!(panel.error.is_none());
        for height in 0..8 {
            let rows = panel.render(20, height);
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
        for ch in "ship it".chars() {
            panel.text(ch);
        }
        let action = click_footer(&mut panel, Hit::Send, width, height);
        assert!(
            matches!(action, Some(Action::Followup(id, text)) if id == IDS[0] && text == "ship it")
        );
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
                                assert!(
                                    row >= 2 && row < body_limit,
                                    "child hit at unexpected row {row}"
                                );
                                let action = panel.mouse(row as u16, region.start, 64, height);
                                if panel.detail {
                                    panel.key(27);
                                }
                                assert!(action.is_none(), "child click must not submit");
                                assert_eq!(panel.selected_id().as_deref(), Some(IDS[index]));
                            }
                            Hit::Stop
                            | Hit::Retry
                            | Hit::Open
                            | Hit::Followup
                            | Hit::Send
                            | Hit::Back
                            | Hit::Close => {
                                assert!(
                                    row == height - 1 && height > 1,
                                    "footer hit off the footer row {row}"
                                );
                                assert!(
                                    rendered[row].contains("stop")
                                        || rendered[row].contains("Esc")
                                        || rendered[row].contains("Enter")
                                );
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
                    assert!(
                        rendered[height - 1].contains("stop")
                            || rendered[height - 1].contains("Esc"),
                        "footer must be the last row at height {height}"
                    );
                    for (row, regions) in map.iter().enumerate() {
                        for region in regions {
                            match region.hit {
                                Hit::Child(index) => {
                                    assert!(row < height - 1, "child hit on the footer row");
                                    assert!(
                                        rendered[row].contains(&format!("task {index}")),
                                        "hit row {row} does not render child {index}"
                                    );
                                    let action = panel.mouse(row as u16, region.start, 64, height);
                                    if panel.detail {
                                        panel.key(27);
                                    }
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
                        assert!(panel
                            .mouse(row as u16, width.saturating_add(2), width, height)
                            .is_none());
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
        let stop = panel.hit_map(40, height)[height - 1]
            .iter()
            .find(|r| r.hit == Hit::Stop)
            .copied()
            .expect("visible stop");
        assert!(matches!(
            panel.mouse(height as u16 - 1, stop.start, 40, height),
            Some(Action::Stop(_))
        ));
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
        let child_hits: Vec<_> = map
            .iter()
            .flat_map(|row| row.iter())
            .filter_map(|hit| match hit.hit {
                Hit::Child(index) => Some(index),
                _ => None,
            })
            .collect();
        assert_eq!(child_hits, vec![0, 1, 2, 3, 4]);
        panel.key(14);
        panel.key(14);
        panel.key(14);
        panel.key(14);
        let rendered = panel.render(100, 8);
        let map = panel.hit_map(100, 8);
        let child_hits: Vec<_> = map
            .iter()
            .flat_map(|row| row.iter())
            .filter_map(|hit| match hit.hit {
                Hit::Child(index) => Some(index),
                _ => None,
            })
            .collect();
        assert_eq!(child_hits, vec![1, 2, 3, 4]);
        assert!(rendered.iter().any(|line| line.contains("task 4")));
    }

    #[test]
    fn metadata_fallback_does_not_create_child_identity() {
        let mut panel = Panel::default();
        panel.show("parent");
        panel
            .update(&json!({"children":[{"session_id":IDS[0],"state":"running",
            "metadata":{"agent":"reviewer","description":"metadata task"}}]}))
            .unwrap();
        let rendered = panel.render(100, 8).join("\n");
        assert!(rendered.contains("reviewer"));
        assert!(rendered.contains("metadata task"));
        assert_eq!(panel.selected_id().as_deref(), Some(IDS[0]));
    }

    #[test]
    fn untrusted_daemon_text_cannot_shift_child_hit_rows() {
        let mut panel = Panel::default();
        panel.show("parent");
        panel
            .update(&json!({"children":[{
                "session_id":"11111111-1111-1111-1111-111111111111",
                "agent":"evil\x1b[2;1H","state":"running","description":"click\nattack"
            }]}))
            .unwrap();
        panel.error = Some("\x1b[31mboom\nboom".into());
        let map = panel.hit_map(40, 20);
        // The error consumes header row 2; the child stays on row 3 no matter
        // what control sequences or newlines the daemon supplied.
        assert!(map[2].is_empty());
        assert!(map[3].iter().any(|r| r.hit == Hit::Child(0)));
        let rendered = panel.render(40, 20);
        assert!(rendered
            .iter()
            .all(|line| !line.chars().any(char::is_control)));
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
pub enum Hit {
    Child(usize),
    Stop,
    Retry,
    Open,
    Followup,
    Send,
    Back,
    Close,
}

/// One clickable column range on one rendered row (row = hit-map index).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct HitRegion {
    pub hit: Hit,
    pub start: u16,
    pub end: u16,
}

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
                end: start
                    .saturating_add(label.chars().count() as u16)
                    .saturating_add(1),
            });
        }
    }
}

impl Panel {
    pub fn show(&mut self, parent: &str) {
        *self = Self {
            open: true,
            parent: parent.into(),
            ..Self::default()
        };
    }
    /// Invalidate controls while a refresh is unavailable or malformed.
    /// Invalidate only the current roster/action epoch. The bound context remains
    /// usable so a failed action can be followed by a legitimate refresh.
    pub fn invalidate_actions(&mut self) {
        self.actions_generation = self.actions_generation.wrapping_add(1);
    }
    /// Invalidate the context lease itself; reopening the panel is required.
    pub fn invalidate_context(&mut self) {
        self.lease_parent = None;
        self.lease_generation = None;
        self.lease_auth = None;
        self.invalidate_actions();
    }
    fn auth_identity(auth: Option<&str>) -> Option<u64> {
        auth.map(|value| {
            let mut hasher = std::collections::hash_map::DefaultHasher::new();
            value.hash(&mut hasher);
            hasher.finish()
        })
    }
    pub fn bind_lease(&mut self, parent: &str, generation: u64, auth: Option<&str>) {
        self.lease_parent = Some(parent.to_owned());
        self.lease_generation = Some(generation);
        self.lease_auth = Self::auth_identity(auth);
        self.actions_generation = self.roster_generation;
    }
    pub fn lease_valid(&self, parent: &str, generation: u64, auth: Option<&str>) -> bool {
        self.actions_valid()
            && self.lease_parent.as_deref() == Some(parent)
            && self.lease_generation == Some(generation)
            && self.lease_auth == Self::auth_identity(auth)
    }
    pub fn context_valid(&self, parent: &str, generation: u64, auth: Option<&str>) -> bool {
        self.lease_parent.as_deref() == Some(parent)
            && self.lease_generation == Some(generation)
            && self.lease_auth == Self::auth_identity(auth)
    }
    pub fn actions_valid(&self) -> bool {
        self.actions_generation == self.roster_generation
    }
    fn action_id(&self) -> Option<String> {
        if !self.actions_valid() {
            return None;
        }
        self.selected_id()
    }
    pub fn update(&mut self, value: &Value) -> Result<(), String> {
        let rows = value["children"]
            .as_array()
            .ok_or("invalid child listing")?;
        if rows.len() > 128 {
            return Err("child listing exceeds limit".into());
        }
        let selected = self.selected_id();
        for row in rows {
            let id = row["session_id"].as_str().ok_or("child id missing")?;
            id.parse::<SessionId>()
                .map_err(|_| "invalid child id".to_owned())?;
        }
        self.rows = rows.clone();
        self.roster_generation = self.roster_generation.wrapping_add(1);
        self.actions_generation = self.roster_generation;
        self.selected = selected
            .and_then(|id| self.rows.iter().position(|row| row["session_id"] == id))
            .unwrap_or(self.selected)
            .min(self.rows.len().saturating_sub(1));
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
        self.rows
            .get(self.selected)?
            .get("session_id")
            .and_then(Value::as_str)
            .map(str::to_owned)
    }

    fn row_text<'a>(row: &'a Value, key: &str, fallback: &'a str) -> &'a str {
        row.get(key)
            .and_then(Value::as_str)
            .filter(|value| !value.trim().is_empty())
            .or_else(|| {
                row.get("metadata")
                    .and_then(|metadata| metadata.get(key))
                    .and_then(Value::as_str)
                    .filter(|value| !value.trim().is_empty())
            })
            .unwrap_or(fallback)
    }

    /// Returns the source Composer's family prefix for a daemon-provided row.
    /// The panel never discovers or creates sessions: `parent_id` and the row
    /// set are authoritative inputs from the existing children endpoint.
    fn family_prefix(&self, index: usize) -> String {
        let Some(row) = self.rows.get(index) else {
            return String::new();
        };
        let Some(parent) = row.get("parent_id").and_then(Value::as_str) else {
            return if index + 1 == self.rows.len() {
                "└─"
            } else {
                "├─"
            }
            .to_owned();
        };
        let mut depth = 0usize;
        let mut current = parent;
        let mut seen = std::collections::HashSet::new();
        while current != self.parent && depth < self.rows.len() && seen.insert(current) {
            let Some(parent_row) = self.rows.iter().find(|candidate| {
                candidate.get("session_id").and_then(Value::as_str) == Some(current)
            }) else {
                break;
            };
            current = parent_row
                .get("parent_id")
                .and_then(Value::as_str)
                .unwrap_or(self.parent.as_str());
            depth += 1;
        }
        let last = index + 1 == self.rows.len();
        let branch = if last { "└─" } else { "├─" };
        if depth == 0 {
            branch.to_owned()
        } else {
            format!("{}{}", "│ ".repeat(depth), branch)
        }
    }

    fn list_window(&self, height: usize, error_row: bool) -> (usize, usize) {
        let top = 2 + usize::from(error_row);
        let capacity = height.saturating_sub(top + 2).min(MAX_LIST_ROWS);
        let start = self.selected.saturating_sub(capacity.saturating_sub(1));
        (start, capacity)
    }
    pub fn text(&mut self, character: char) {
        if self.composing
            && !character.is_control()
            && self.input.len() + character.len_utf8() <= 32768
        {
            self.input.push(character);
        }
    }
    pub fn key(&mut self, byte: u8) -> Option<Action> {
        if self.composing {
            match byte {
                27 => {
                    self.composing = false;
                    self.input.clear();
                }
                8 | 127 => {
                    self.input.pop();
                }
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
            27 if self.detail => {
                self.detail = false;
                self.history.clear();
            }
            27 => self.open = false,
            14 if self.detail => self.chat.scroll_down(3),
            2 if self.detail => self.chat.scroll_up(3),
            14 => {
                self.selected = self
                    .selected
                    .saturating_add(1)
                    .min(self.rows.len().saturating_sub(1))
            }
            2 => self.selected = self.selected.saturating_sub(1),
            13 | 10 if !self.rows.is_empty() => {
                self.detail = true;
                self.chat = ChatView::default();
            }
            b'o' => return self.action_id().map(Action::Open),
            b's' => return self.action_id().map(Action::Stop),
            b'r' => return self.action_id().map(Action::Retry),
            b'c' if !self.rows.is_empty() => {
                self.detail = true;
                self.composing = true;
            }
            b'1'..=b'9' => {
                // Numeric child selection mirrors the pointer row click; the
                // detail view keeps its own scroll keys, so digits are inert there.
                if !self.detail {
                    let index = usize::from(byte - b'1');
                    if index < self.rows.len() {
                        self.selected = index;
                    }
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
        if width == 0 || height == 0 {
            return map;
        }
        if height > 1 {
            let footer = if self.composing {
                "Enter send follow-up · Esc cancel"
            } else if self.detail {
                "↑/↓ scroll · s stop · r retry · c follow-up · Esc back"
            } else {
                "↑/↓ select · Enter detail · s stop · r retry · Esc close · o open"
            };
            let labels: &[(&str, Hit)] = if self.composing {
                &[
                    ("Enter send follow-up", Hit::Send),
                    ("Esc cancel", Hit::Back),
                ]
            } else if self.detail {
                &[
                    ("s stop", Hit::Stop),
                    ("r retry", Hit::Retry),
                    ("c follow-up", Hit::Followup),
                    ("Esc back", Hit::Back),
                ]
            } else {
                &[
                    ("s stop", Hit::Stop),
                    ("r retry", Hit::Retry),
                    ("Esc close", Hit::Close),
                    ("o open", Hit::Open),
                ]
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
            let (start, capacity) =
                self.list_window(height, self.error.is_some() || self.action_error.is_some());
            if capacity > 0 {
                let limit = height.saturating_sub(2).max(1);
                for (index, _) in self.rows.iter().enumerate().skip(start).take(capacity) {
                    let line = top + index - start;
                    if line < limit {
                        if let Some(row) = map.get_mut(line) {
                            row.push(HitRegion {
                                hit: Hit::Child(index),
                                start: 0,
                                end: width.saturating_sub(1),
                            });
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
        if column >= width {
            return None;
        }
        let hit_map = self.hit_map(width, height);
        let regions = hit_map.get(usize::from(row))?;
        let hit = regions
            .iter()
            .find(|r| column >= r.start && column <= r.end)
            .map(|r| r.hit)?;
        self.mouse_from_hit(hit)
    }

    /// Hovering a child list row updates only the highlighted row; buttons and
    /// footer controls remain press-only (`subagents-tab.tsx:224`).
    pub fn hover(&mut self, row: u16, column: u16, width: u16, height: usize) -> bool {
        if column >= width { return false; }
        let hit_map = self.hit_map(width, height);
        let Some(regions) = hit_map.get(usize::from(row)) else { return false };
        let Some(Hit::Child(index)) = regions.iter().find(|r| column >= r.start && column <= r.end).map(|r| r.hit) else { return false };
        if index >= self.rows.len() || self.selected == index { return false }
        self.selected = index;
        true
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
            Hit::Followup => {
                self.key(b'c');
                None
            }
            Hit::Send => self.key(13),
            Hit::Back | Hit::Close => self.key(27),
        }
    }
    pub fn render(&self, width: u16, height: usize) -> Vec<String> {
        if height == 0 {
            return Vec::new();
        }
        let mut lines = vec!["Subagents".into(), String::new()];
        let footer = if self.composing {
            "Enter send follow-up · Esc cancel"
        } else if self.detail {
            "↑/↓ scroll · s stop · r retry · c follow-up · Esc back"
        } else {
            "↑/↓ select · Enter detail · s stop · r retry · Esc close · o open"
        };
        if let Some(error) = self.action_error.as_ref().or(self.error.as_ref()) {
            lines.push(format!("Error: {error}"));
        }
        if self.detail {
            if let Some(row) = self.rows.get(self.selected) {
                lines.push(format!(
                    "{} · {} · {}",
                    row["agent"].as_str().unwrap_or("?"),
                    row["state"].as_str().unwrap_or("unknown"),
                    row["model"].as_str().unwrap_or("?")
                ));
                lines.push(format!(
                    "Session: {}",
                    row["session_id"].as_str().unwrap_or("?")
                ));
                if let Some(error) = row["error"]
                    .as_str()
                    .filter(|error| !error.trim().is_empty())
                {
                    lines.push(format!("Error: {error}"));
                }
                let metric = |key: &str| {
                    row.get(key)
                        .and_then(Value::as_u64)
                        .map_or_else(|| "unreported".to_owned(), |value| value.to_string())
                };
                let pending = row
                    .get("pending_followups")
                    .and_then(Value::as_u64)
                    .map_or_else(|| "unreported".to_owned(), |value| format!("{value}/4"));
                let read_only = row
                    .get("read_only")
                    .and_then(Value::as_bool)
                    .map_or("unreported", |value| if value { "yes" } else { "no" });
                lines.push(format!(
                    "generation {} · auto-retry {} · manual retries {} · pending {} · read-only {}",
                    metric("generation"),
                    metric("auto_retry_attempts"),
                    metric("manual_retries"),
                    pending,
                    read_only
                ));
                lines.push(String::new());
                let budget = height.saturating_sub(lines.len() + 3);
                if self.history.is_empty() {
                    lines.extend(
                        row["output_tail"]
                            .as_str()
                            .unwrap_or("Waiting for child output…")
                            .lines()
                            .take(budget)
                            .map(str::to_owned),
                    );
                } else {
                    lines.extend(
                        self.chat
                            .render_typed(
                                usize::from(width),
                                budget + 2,
                                "Child history",
                                &self.history,
                                "",
                                false,
                            )
                            .into_iter()
                            .skip(1)
                            .take(budget),
                    );
                }
            }
            if self.composing {
                lines.push(format!("Follow-up: {}▏", self.input));
            }
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
                lines.push(format!(
                    "{} [{}] {} {} — {}",
                    if index == self.selected { ">" } else { " " },
                    state,
                    self.family_prefix(index),
                    agent,
                    description
                ));
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
        lines
            .into_iter()
            .take(height)
            .map(|line| fit(&line, width))
            .collect()
    }
}

/// Subagents tab of the session composer auxiliary dock
/// (`packages/tui/src/routes/session/composer/subagents-tab.tsx:26-259`).
///
/// Upstream facts encoded here:
/// - rows come from `sessionFamily(data.session.list(), current.id)`, i.e. the
///   whole descendant tree of the *root* ancestor of the composer's session,
///   depth-first with `sessionFamily`'s exact prefix glyphs
///   (`util/session.ts:9-45`, `subagents-tab.tsx:41-65`);
/// - the agent label is `session.agent` titlecased, else the title's
///   `@(\w+) subagent` capture titlecased, else `"Subagent"`, and that capture
///   is stripped from the displayed title (`:48-58`);
/// - the activity filter starts active, is held in TUI memory so a route
///   remount does not change it, and only `ctrl+a` toggles it
///   (`:38-39,64,174-184`; `test/cli/tui/composer-keymap.test.tsx` "the
///   inactive subagent filter only changes with its toggle");
/// - on activation the selected row is the current session's row when it is in
///   the filtered list, else row 0, and it is centred
///   (`:73-93,103-116`);
/// - deactivation resets the selection and the remembered session
///   (`:75-82`); a shrunk list clamps the selection (`:95`);
/// - `up` on row 0 closes the composer, `down` wraps (`:142-164`);
/// - `select` navigates to the row's session (`:165-173`), `interrupt` is only
///   available for a running row (`:122-133,185-194`);
/// - the exact empty text is ` No {active|inactive} subagents`
///   (`:201-204`), and the list sits in a `maxHeight={5}` scrollbox (`:200`);
/// - the row is `{prefix}{agent}: {title}` with `Running` right-aligned as a
///   literal, never a percentage or spinner (`:205-250`).
#[cfg(feature = "native")]
pub use composer_subagents::{
    SubagentAction, SubagentEntry, SubagentRow, SubagentSession, SubagentShortcuts, SubagentsTab,
    SUBAGENT_DOWN_COMMAND, SUBAGENT_INTERRUPT_COMMAND, SUBAGENT_SELECT_COMMAND,
    SUBAGENT_TOGGLE_COMMAND, SUBAGENT_UP_COMMAND,
};

#[cfg(feature = "native")]
mod composer_subagents {
    use crate::native_dock::{
        DockHint, DockStyle, MAX_DOCK_ROWS, SUBAGENTS_TAB_ID, SUBAGENTS_TAB_LABEL,
    };

    pub const SUBAGENT_UP_COMMAND: &str = "composer.subagent.up";
    pub const SUBAGENT_DOWN_COMMAND: &str = "composer.subagent.down";
    pub const SUBAGENT_SELECT_COMMAND: &str = "composer.subagent.select";
    pub const SUBAGENT_TOGGLE_COMMAND: &str = "composer.subagent.toggle-activity";
    pub const SUBAGENT_INTERRUPT_COMMAND: &str = "composer.subagent.interrupt";
    /// Upstream literal status label for a running child (`:208-211`).
    pub const RUNNING_LABEL: &str = "Running";
    /// Upstream fallback agent when neither the session nor the title names one
    /// (`:50-55`).
    pub const SUBAGENT_FALLBACK_AGENT: &str = "Subagent";
    /// Bounds for the family walk. Upstream relies on the server's acyclic
    /// `parentID` graph; the port refuses cycles instead of recursing forever.
    const MAX_FAMILY_DEPTH: usize = 64;
    const MAX_FAMILY_ROWS: usize = 256;

    /// One session from `data.session.list()`.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SubagentSession {
        pub id: String,
        pub parent_id: Option<String>,
        pub title: Option<String>,
        pub agent: Option<String>,
        /// `time.created` in milliseconds; upstream requires it for the
        /// timestamped title fallback (`util/session-title-fallback.ts:1-29`).
        pub created_millis: Option<i64>,
        pub status: String,
    }

    impl SubagentSession {
        #[must_use]
        pub fn new(id: impl Into<String>, status: impl Into<String>) -> Self {
            Self {
                id: id.into(),
                parent_id: None,
                title: None,
                agent: None,
                created_millis: None,
                status: status.into(),
            }
        }

        #[must_use]
        pub fn with_parent(mut self, parent: impl Into<String>) -> Self {
            self.parent_id = Some(parent.into());
            self
        }

        #[must_use]
        pub fn with_title(mut self, title: impl Into<String>) -> Self {
            self.title = Some(title.into());
            self
        }

        #[must_use]
        pub fn with_agent(mut self, agent: impl Into<String>) -> Self {
            self.agent = Some(agent.into());
            self
        }

        #[must_use]
        pub fn with_created(mut self, millis: i64) -> Self {
            self.created_millis = Some(millis);
            self
        }

        #[must_use]
        pub fn is_running(&self) -> bool {
            self.status == "running"
        }
    }

    /// One projected family row (`sessionFamily` result).
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SubagentEntry {
        pub session_id: String,
        /// `Locale.titlecase(session.agent)` or the title capture.
        pub agent: String,
        /// Title with the `@agent subagent` capture removed; `None` when the
        /// session has neither a title nor a creation timestamp (upstream would
        /// throw on `new Date(undefined).toISOString()`, so the port drops the
        /// invented text instead of guessing).
        pub title: Option<String>,
        pub status: String,
        /// `session.id === route.sessionID` (`:58`).
        pub current: bool,
        /// `sessionFamily` connector prefix (`:242`).
        pub prefix: String,
        pub depth: usize,
    }

    impl SubagentEntry {
        #[must_use]
        pub fn is_running(&self) -> bool {
            self.status == "running"
        }

        /// `{status()}` right-aligned cell (`:208-211,246-250`).
        #[must_use]
        pub fn status_label(&self) -> &'static str {
            if self.is_running() {
                RUNNING_LABEL
            } else {
                ""
            }
        }

        /// `{prefix}{agent}: {title}` (`:242-243`).
        #[must_use]
        pub fn row_label(&self) -> String {
            match self.title.as_deref() {
                Some(title) => format!("{}{}: {}", self.prefix, self.agent, title),
                None => format!("{}{}", self.prefix, self.agent),
            }
        }
    }

    /// One painted row.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SubagentRow {
        pub session_id: String,
        pub label: String,
        pub status: &'static str,
        pub active: bool,
        pub current: bool,
        pub status_style: DockStyle,
    }

    /// Resolved `composer.subagent.*` shortcut labels. `toggle_activity` has no
    /// config keybind id (the layer binds `ctrl+a` literally), so upstream's
    /// `shortcuts.get(...) ?? ""` renders an empty shortcut.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct SubagentShortcuts {
        pub interrupt: Option<String>,
        pub toggle_activity: Option<String>,
    }

    impl SubagentShortcuts {
        /// Upstream defaults: `composer.subagent.interrupt` is `ctrl+d`
        /// (`config/keybind.ts:252`); the toggle has no keybind entry.
        #[must_use]
        pub fn upstream_defaults() -> Self {
            Self {
                interrupt: Some("ctrl+d".into()),
                toggle_activity: None,
            }
        }

        #[must_use]
        pub fn disabled() -> Self {
            Self::default()
        }
    }

    /// What the caller must do after a subagents-tab command.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub enum SubagentAction {
        None,
        Selected(usize),
        /// `composer.close()` (`:148-151`).
        CloseComposer,
        /// `navigate({ type: "session", sessionID })` (`:170-172`).
        Navigate(String),
        /// `client.api.session.interrupt({ sessionID })` (`:189-193`).
        Interrupt(String),
    }

    /// Family projection used by the tab: the exact `sessionFamily` recursion
    /// from `util/session.ts:9-45`.
    #[must_use]
    pub fn subagent_family(sessions: &[SubagentSession], session_id: &str) -> Vec<SubagentEntry> {
        let Some(_) = sessions.iter().find(|session| session.id == session_id) else {
            return Vec::new();
        };
        let mut children: Vec<(&str, Vec<usize>)> = Vec::new();
        for (index, session) in sessions.iter().enumerate() {
            let Some(parent) = session.parent_id.as_deref() else {
                continue;
            };
            match children.iter_mut().find(|(id, _)| *id == parent) {
                Some((_, group)) => group.push(index),
                None => children.push((parent, vec![index])),
            }
        }
        // Find the root ancestor, refusing a cyclic parent chain.
        let mut root = session_id;
        let mut seen = std::collections::HashSet::new();
        loop {
            seen.insert(root);
            let Some(current) = sessions.iter().find(|session| session.id == root) else {
                return Vec::new();
            };
            match current.parent_id.as_deref().and_then(|parent| {
                sessions
                    .iter()
                    .find(|candidate| candidate.id == parent)
                    .map(|_| parent)
            }) {
                Some(parent) if !seen.contains(parent) => root = parent,
                Some(_) => return Vec::new(),
                None => break,
            }
        }
        let mut rows = Vec::new();
        walk_family(sessions, &children, root, &[], 0, &mut rows);
        rows
    }

    fn walk_family(
        sessions: &[SubagentSession],
        children: &[(&str, Vec<usize>)],
        parent_id: &str,
        ancestors: &[bool],
        depth: usize,
        rows: &mut Vec<SubagentEntry>,
    ) {
        if depth >= MAX_FAMILY_DEPTH || rows.len() >= MAX_FAMILY_ROWS {
            return;
        }
        let Some((_, group)) = children.iter().find(|(id, _)| *id == parent_id) else {
            return;
        };
        let len = group.len();
        for (position, index) in group.iter().copied().enumerate() {
            let session = &sessions[index];
            let last = position == len - 1;
            let prefix = if ancestors.is_empty() {
                String::new()
            } else {
                let mut prefix: String = ancestors[1..]
                    .iter()
                    .map(|ancestor| if *ancestor { "   " } else { "│  " })
                    .collect();
                prefix.push_str(if last { "└─ " } else { "├─ " });
                prefix
            };
            let fallback = timestamped_title(session);
            let (agent, title) = agent_and_title(session, fallback.as_deref());
            rows.push(SubagentEntry {
                session_id: session.id.clone(),
                agent,
                title,
                status: session.status.clone(),
                current: false,
                prefix,
                depth,
            });
            let mut next = ancestors.to_vec();
            next.push(last);
            walk_family(sessions, children, &session.id, &next, depth + 1, rows);
            if rows.len() >= MAX_FAMILY_ROWS {
                return;
            }
        }
    }

    /// `withTimestampedFallback` (`util/session-title-fallback.ts:1-29`):
    /// `title ?? "{New|Child} session - {ISO}"`.
    fn timestamped_title(session: &SubagentSession) -> Option<String> {
        if let Some(title) = session.title.as_deref() {
            return Some(title.to_owned());
        }
        let created = session.created_millis?;
        Some(format!(
            "{} session - {}",
            if session.parent_id.is_some() {
                "Child"
            } else {
                "New"
            },
            iso_from_millis(created)
        ))
    }

    /// `new Date(ms).toISOString()` for the title fallback
    /// (`YYYY-MM-DDTHH:mm:ss.sssZ`).
    fn iso_from_millis(millis: i64) -> String {
        const MS_PER_DAY: i64 = 86_400_000;
        let days = millis.div_euclid(MS_PER_DAY);
        let rem = millis.rem_euclid(MS_PER_DAY);
        let (year, month, day) = civil_from_days(days);
        let hour = rem / 3_600_000;
        let minute = (rem % 3_600_000) / 60_000;
        let second = (rem % 60_000) / 1_000;
        let milli = rem % 1_000;
        format!("{year:04}-{month:02}-{day:02}T{hour:02}:{minute:02}:{second:02}.{milli:03}Z")
    }

    /// Howard Hinnant's civil-from-days algorithm (days since 1970-01-01).
    fn civil_from_days(days: i64) -> (i64, i64, i64) {
        let z = days + 719_468;
        let era = if z >= 0 { z } else { z - 146_096 } / 146_097;
        let doe = z - era * 146_097;
        let yoe = (doe - doe / 1_460 + doe / 36_524 - doe / 146_096) / 365;
        let y = yoe + era * 400;
        let doy = doe - (365 * yoe + yoe / 4 - yoe / 100);
        let mp = (5 * doy + 2) / 153;
        let d = doy - (153 * mp + 2) / 5 + 1;
        let m = if mp < 10 { mp + 3 } else { mp - 9 };
        (if m <= 2 { y + 1 } else { y }, m, d)
    }

    /// `subagents-tab.tsx:45-58`: agent and displayed title.
    fn agent_and_title(
        session: &SubagentSession,
        fallback_title: Option<&str>,
    ) -> (String, Option<String>) {
        let title = fallback_title;
        let capture = title.and_then(subagent_capture);
        let agent = session
            .agent
            .as_deref()
            .map(titlecase)
            .or_else(|| capture.map(|(name, _)| titlecase(name)))
            .unwrap_or_else(|| SUBAGENT_FALLBACK_AGENT.to_owned());
        let display = match (title, capture) {
            (Some(title), Some((_, matched))) => {
                // JS `String.replace` replaces the first occurrence only.
                let stripped = title.replacen(matched, "", 1).trim().to_owned();
                Some(if stripped.is_empty() {
                    title.to_owned()
                } else {
                    stripped
                })
            }
            (title, _) => title.map(str::to_owned),
        };
        (agent, display)
    }

    /// `title.match(/@(\w+) subagent/)`: capture group 1 (JS `\w` is ASCII)
    /// plus the whole matched text, which `String.replace` strips.
    fn subagent_capture(title: &str) -> Option<(&str, &str)> {
        let mut search_from = 0;
        while let Some(offset) = title[search_from..].find('@') {
            let at = search_from + offset;
            let after = &title[at + 1..];
            let name_len = after
                .chars()
                .take_while(|ch| ch.is_ascii_alphanumeric() || *ch == '_')
                .map(char::len_utf8)
                .sum::<usize>();
            if name_len > 0 && after[name_len..].starts_with(" subagent") {
                let name = &after[..name_len];
                let matched = &title[at..at + 1 + name_len + " subagent".len()];
                return Some((name, matched));
            }
            search_from = at + 1;
            if search_from >= title.len() {
                break;
            }
        }
        None
    }

    /// `Locale.titlecase` (`util/locale.ts:3-5`): uppercase the first `\w` of
    /// each word, ASCII-only.
    fn titlecase(text: &str) -> String {
        let mut output = String::with_capacity(text.len());
        let mut at_boundary = true;
        for ch in text.chars() {
            let is_word = ch.is_ascii_alphanumeric() || ch == '_';
            if is_word && at_boundary {
                output.push(ch.to_ascii_uppercase());
            } else {
                output.push(ch);
            }
            at_boundary = !is_word;
        }
        output
    }

    /// State of the subagents tab.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct SubagentsTab {
        session_id: String,
        route_session_id: String,
        sessions: Vec<SubagentSession>,
        /// `memory("subagents-filter", { initial: { active: true } })`
        /// (`:39`).
        filter_active: bool,
        selected: usize,
        selected_session_id: String,
        was_active: bool,
        scroll_top: usize,
        viewport: usize,
    }

    impl Default for SubagentsTab {
        fn default() -> Self {
            Self::new()
        }
    }

    impl SubagentsTab {
        #[must_use]
        pub fn new() -> Self {
            Self {
                session_id: String::new(),
                route_session_id: String::new(),
                sessions: Vec::new(),
                filter_active: true,
                selected: 0,
                selected_session_id: String::new(),
                was_active: false,
                scroll_top: 0,
                viewport: MAX_DOCK_ROWS,
            }
        }

        #[must_use]
        pub const fn tab_id() -> &'static str {
            SUBAGENTS_TAB_ID
        }

        #[must_use]
        pub const fn tab_label() -> &'static str {
            SUBAGENTS_TAB_LABEL
        }

        /// Composer session and current route session (`props.sessionID` vs
        /// `route.sessionID`).
        pub fn set_sessions(
            &mut self,
            session_id: &str,
            route_session_id: &str,
            sessions: Vec<SubagentSession>,
        ) {
            self.session_id = session_id.to_owned();
            self.route_session_id = route_session_id.to_owned();
            self.sessions = sessions;
            self.clamp_selection();
        }

        /// The live filter state (`ctrl+a` only changes it).
        #[must_use]
        pub fn filter_active(&self) -> bool {
            self.filter_active
        }

        #[must_use]
        pub fn selected(&self) -> usize {
            self.selected
        }

        #[must_use]
        pub fn scroll_top(&self) -> usize {
            self.scroll_top
        }

        /// Scroll viewport rows; capped at the dock maximum.
        pub fn set_viewport(&mut self, viewport: usize) {
            self.viewport = viewport.min(MAX_DOCK_ROWS);
            self.clamp_scroll();
        }

        /// All family rows before the activity filter.
        #[must_use]
        pub fn family(&self) -> Vec<SubagentEntry> {
            let mut rows = subagent_family(&self.sessions, &self.session_id);
            for row in &mut rows {
                row.current = row.session_id == self.route_session_id;
            }
            rows
        }

        /// Filtered entries (`:64`): running rows while the filter is active,
        /// non-running rows otherwise.
        #[must_use]
        pub fn entries(&self) -> Vec<SubagentEntry> {
            self.family()
                .into_iter()
                .filter(|entry| {
                    if self.filter_active {
                        entry.is_running()
                    } else {
                        !entry.is_running()
                    }
                })
                .collect()
        }

        #[must_use]
        pub fn selected_entry(&self) -> Option<SubagentEntry> {
            self.entries().get(self.selected).cloned()
        }

        fn clamp_selection(&mut self) {
            let len = self.entries().len();
            if self.selected >= len {
                self.selected = len.saturating_sub(1);
            }
        }

        fn max_scroll(&self) -> usize {
            self.entries().len().saturating_sub(self.viewport)
        }

        fn clamp_scroll(&mut self) {
            self.scroll_top = self.scroll_top.min(self.max_scroll());
        }

        /// The `composer.active("subagents")` effect (`:73-96`).
        pub fn set_active(&mut self, active: bool) {
            if !active {
                if self.was_active {
                    self.selected_session_id.clear();
                    self.selected = 0;
                }
                self.was_active = false;
                return;
            }
            let entries = self.entries();
            if self.selected_session_id != self.route_session_id && !entries.is_empty() {
                let current = entries.iter().position(|entry| entry.current);
                let next = current.unwrap_or(0);
                self.selected_session_id = self.route_session_id.clone();
                self.selected = next;
                self.scroll_to_index(next, true);
            }
            self.was_active = true;
            let len = self.entries().len();
            if self.selected >= len {
                self.move_to(len.saturating_sub(1), false);
            }
        }

        fn move_to(&mut self, index: usize, center: bool) {
            self.selected = index;
            self.scroll_to_index(index, center);
        }

        /// `scrollToIndex` (`:103-116`) with the scrollbox clamps applied.
        fn scroll_to_index(&mut self, index: usize, center: bool) {
            if center {
                let half = self.viewport / 2;
                self.scroll_top = index.saturating_sub(half);
            } else {
                if index >= self.scroll_top + self.viewport {
                    self.scroll_top = index + 1 - self.viewport;
                }
                if index < self.scroll_top {
                    self.scroll_top = index;
                }
            }
            self.clamp_scroll();
        }

        /// Hints (`:122-134`): `interrupt` only for a running selection, then
        /// the activity toggle with the *inverse* action label.
        #[must_use]
        pub fn hints(&self, shortcuts: &SubagentShortcuts) -> Vec<DockHint> {
            let mut hints = Vec::new();
            if self
                .selected_entry()
                .is_some_and(|entry| entry.is_running())
            {
                hints.push(DockHint::new("interrupt", shortcuts.interrupt.clone()));
            }
            hints.push(DockHint::new(
                format!(
                    "show {}",
                    if self.filter_active {
                        "inactive"
                    } else {
                        "active"
                    }
                ),
                shortcuts.toggle_activity.clone(),
            ));
            hints
        }

        /// Dispatch one upstream command id (`:142-195`). Unknown ids are inert:
        /// the component owns no key fallbacks.
        pub fn command(&mut self, id: &str) -> SubagentAction {
            match id {
                SUBAGENT_UP_COMMAND => self.up(),
                SUBAGENT_DOWN_COMMAND => self.down(),
                SUBAGENT_SELECT_COMMAND => self.select(),
                SUBAGENT_TOGGLE_COMMAND => {
                    self.toggle_activity();
                    SubagentAction::None
                }
                SUBAGENT_INTERRUPT_COMMAND => self.interrupt(),
                _ => SubagentAction::None,
            }
        }

        /// `composer.subagent.up` (`:142-154`).
        pub fn up(&mut self) -> SubagentAction {
            if self.selected == 0 {
                return SubagentAction::CloseComposer;
            }
            let next = self.selected.saturating_sub(1);
            self.move_to(next, true);
            SubagentAction::Selected(next)
        }

        /// `composer.subagent.down` (`:155-164`).
        pub fn down(&mut self) -> SubagentAction {
            let len = self.entries().len();
            if len == 0 {
                return SubagentAction::None;
            }
            let next = (self.selected + 1) % len;
            self.move_to(next, true);
            SubagentAction::Selected(next)
        }

        /// `composer.subagent.select` (`:165-173`).
        pub fn select(&mut self) -> SubagentAction {
            self.selected_entry()
                .map(|entry| SubagentAction::Navigate(entry.session_id))
                .unwrap_or(SubagentAction::None)
        }

        /// `composer.subagent.toggle-activity` (`:174-184`): resets the
        /// selection and the scroll position.
        pub fn toggle_activity(&mut self) {
            self.filter_active = !self.filter_active;
            self.selected = 0;
            self.scroll_top = 0;
        }

        /// `composer.subagent.interrupt` (`:185-194`): running rows only.
        pub fn interrupt(&mut self) -> SubagentAction {
            match self.selected_entry() {
                Some(entry) if entry.is_running() => SubagentAction::Interrupt(entry.session_id),
                _ => SubagentAction::None,
            }
        }

        /// Pointer move selects (`:224`).
        pub fn hover(&mut self, index: usize) -> SubagentAction {
            if index < self.entries().len() && index != self.selected {
                self.selected = index;
                SubagentAction::Selected(index)
            } else {
                SubagentAction::None
            }
        }

        /// Pointer up selects then navigates (`:225-228`).
        pub fn click(&mut self, index: usize) -> SubagentAction {
            if index < self.entries().len() {
                self.selected = index;
                return self.select();
            }
            SubagentAction::None
        }

        /// Painted rows with upstream row paint: focused beats current
        /// (`:217-249`).
        #[must_use]
        pub fn rows(&self) -> Vec<SubagentRow> {
            self.entries()
                .into_iter()
                .enumerate()
                .map(|(index, entry)| {
                    let active = index == self.selected;
                    SubagentRow {
                        label: entry.row_label(),
                        status: entry.status_label(),
                        active,
                        current: entry.current,
                        status_style: if active {
                            DockStyle::SubagentStatusActive
                        } else {
                            DockStyle::SubagentStatusMuted
                        },
                        session_id: entry.session_id,
                    }
                })
                .collect()
        }

        /// Painted window: `viewport` rows from `scroll_top`
        /// (`scrollbox maxHeight={5}`, `:200`).
        #[must_use]
        pub fn visible_rows(&self) -> Vec<SubagentRow> {
            self.rows()
                .into_iter()
                .skip(self.scroll_top)
                .take(self.viewport.max(1))
                .collect()
        }

        /// Row background/foreground class (`:217-239`). The bool is the
        /// BOLD attribute of the row *label* (the status cell is never bold).
        #[must_use]
        pub fn row_style(row: &SubagentRow) -> (DockStyle, bool) {
            if row.active {
                (DockStyle::SubagentActive, true)
            } else if row.current {
                (DockStyle::SubagentCurrent, false)
            } else {
                (DockStyle::SubagentBase, false)
            }
        }

        /// Exact empty text (`:203`).
        #[must_use]
        pub fn empty_text(&self) -> String {
            format!(
                " No {} subagents",
                if self.filter_active {
                    "active"
                } else {
                    "inactive"
                }
            )
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;

        const IDS: [&str; 7] = ["a0", "a1", "a2", "a3", "a4", "a5", "a6"];

        fn sessions() -> Vec<SubagentSession> {
            vec![
                SubagentSession::new("parent", "idle").with_title("Parent"),
                SubagentSession::new("child-a", "running")
                    .with_parent("parent")
                    .with_title("First")
                    .with_agent("build"),
                SubagentSession::new("child-b", "running")
                    .with_parent("parent")
                    .with_title("Second")
                    .with_agent("plan"),
                SubagentSession::new("child-c", "idle")
                    .with_parent("parent")
                    .with_title("Third")
                    .with_agent("build"),
            ]
        }

        fn tab() -> SubagentsTab {
            let mut tab = SubagentsTab::new();
            tab.set_sessions("parent", "parent", sessions());
            tab
        }

        // CP-24: nested family, depth-first order and upstream prefixes.
        #[test]
        fn cp24_nested_family_prefixes_and_order() {
            let mut tab = SubagentsTab::new();
            tab.set_sessions(
                "parent",
                "parent",
                vec![
                    SubagentSession::new("parent", "idle"),
                    SubagentSession::new("a", "running")
                        .with_parent("parent")
                        .with_title("A"),
                    SubagentSession::new("a1", "running")
                        .with_parent("a")
                        .with_title("A1"),
                    SubagentSession::new("a1x", "running")
                        .with_parent("a1")
                        .with_title("A1X"),
                    SubagentSession::new("a2", "running")
                        .with_parent("a")
                        .with_title("A2"),
                    SubagentSession::new("b", "running")
                        .with_parent("parent")
                        .with_title("B"),
                ],
            );
            let rows = tab.entries();
            let seen: Vec<(&str, &str)> = rows
                .iter()
                .map(|row| (row.prefix.as_str(), row.session_id.as_str()))
                .collect();
            // Root children carry no prefix; nested rows use sessionFamily's
            // exact glyphs (`util/session.ts:27-43`).
            assert_eq!(
                seen,
                vec![
                    ("", "a"),
                    ("├─ ", "a1"),
                    ("│  └─ ", "a1x"),
                    ("└─ ", "a2"),
                    ("", "b"),
                ]
            );
            // Four levels keep upstream's shifted ancestor column: every
            // ancestor in this single-child chain is a last sibling, so
            // `ancestors.slice(1)` yields two blank columns.
            let mut deep = SubagentsTab::new();
            deep.set_sessions(
                "parent",
                "parent",
                vec![
                    SubagentSession::new("parent", "idle"),
                    SubagentSession::new("a", "running").with_parent("parent"),
                    SubagentSession::new("a1", "running").with_parent("a"),
                    SubagentSession::new("a1x", "running").with_parent("a1"),
                    SubagentSession::new("a1x1", "running").with_parent("a1x"),
                ],
            );
            let last = deep.entries().pop().unwrap();
            assert_eq!(last.prefix, "      └─ ");
            assert_eq!(last.depth, 3);
            // A child session's dock shows the whole root family.
            let family = tab.sessions.clone();
            tab.set_sessions("a1x", "a1x", family);
            let ids: Vec<String> = tab
                .entries()
                .into_iter()
                .map(|row| row.session_id)
                .collect();
            assert_eq!(ids, vec!["a", "a1", "a1x", "a2", "b"]);
            // Unknown session and cyclic parents project nothing.
            assert!(subagent_family(&sessions(), "missing").is_empty());
            let cyclic = vec![
                SubagentSession::new("x", "running").with_parent("y"),
                SubagentSession::new("y", "running").with_parent("x"),
            ];
            assert!(subagent_family(&cyclic, "x").is_empty());
        }

        // CP-24: filter defaults to active, only the toggle changes it, and the
        // current session can be hidden by the filter.
        #[test]
        fn cp24_activity_filter_default_and_toggle() {
            let mut tab = tab();
            assert!(tab.filter_active());
            let ids: Vec<String> = tab
                .entries()
                .into_iter()
                .map(|row| row.session_id)
                .collect();
            assert_eq!(ids, vec!["child-a", "child-b"]);
            // The focused row is running, so `interrupt` leads the hints
            // (`subagents-tab.tsx:154-166`).
            let hints = tab.hints(&SubagentShortcuts::upstream_defaults());
            let labels: Vec<&str> = hints.iter().map(|hint| hint.label.as_str()).collect();
            assert_eq!(labels, vec!["interrupt", "show inactive"]);
            tab.command(SUBAGENT_TOGGLE_COMMAND);
            assert!(!tab.filter_active());
            let ids: Vec<String> = tab
                .entries()
                .into_iter()
                .map(|row| row.session_id)
                .collect();
            assert_eq!(ids, vec!["child-c"]);
            assert_eq!(tab.selected(), 0, "the toggle resets the selection");
            assert_eq!(tab.empty_text(), " No inactive subagents");
            assert_eq!(
                tab.hints(&SubagentShortcuts::upstream_defaults())
                    .first()
                    .map(|hint| hint.label.clone()),
                Some("show active".into()),
                "no interrupt hint for an idle row"
            );
            // The current session is hidden by the active filter.
            tab.set_sessions("parent", "child-c", sessions());
            tab.toggle_activity();
            assert!(tab.filter_active());
            let rows = tab.entries();
            assert!(!rows.iter().any(|row| row.current));
            tab.set_active(false);
            tab.set_active(true);
            assert_eq!(
                tab.selected(),
                0,
                "findIndex(-1) selects the first filtered row"
            );
            assert_eq!(tab.selected_entry().unwrap().session_id, "child-a");
        }

        // CP-24: route remount reselects the current session's row.
        #[test]
        fn cp24_route_remount_reselects_current() {
            let mut tab = tab();
            tab.set_active(true);
            assert_eq!(tab.selected(), 0);
            // Navigating to child-b remounts the route; the effect re-runs.
            tab.set_sessions("parent", "child-b", sessions());
            tab.set_active(false);
            tab.set_active(true);
            assert_eq!(tab.selected(), 1);
            assert_eq!(tab.selected_entry().unwrap().session_id, "child-b");
            // Selecting the running row interrupts it.
            assert_eq!(
                tab.command(SUBAGENT_INTERRUPT_COMMAND),
                SubagentAction::Interrupt("child-b".into())
            );
            assert_eq!(
                tab.command(SUBAGENT_SELECT_COMMAND),
                SubagentAction::Navigate("child-b".into())
            );
        }

        // CP-24: deletion clamps the selection; up on row 0 closes.
        #[test]
        fn cp24_deletion_clamps_and_up_closes() {
            let mut tab = tab();
            tab.set_active(true);
            tab.down();
            assert_eq!(tab.selected(), 1);
            let mut shrunk = sessions();
            shrunk.retain(|session| session.id != "child-b" && session.id != "child-c");
            tab.set_sessions("parent", "parent", shrunk);
            assert_eq!(tab.selected(), 0);
            assert_eq!(
                tab.command(SUBAGENT_UP_COMMAND),
                SubagentAction::CloseComposer
            );
            assert_eq!(tab.selected(), 0);
            // Down wraps over the filtered list.
            assert_eq!(
                tab.command(SUBAGENT_DOWN_COMMAND),
                SubagentAction::Selected(0)
            );
            assert_eq!(tab.command("none"), SubagentAction::None);
        }

        // CP-24: more than five rows scroll inside the maxHeight-5 window.
        #[test]
        fn cp24_more_than_five_rows_scrolls_selection_into_view() {
            let mut tab = SubagentsTab::new();
            let mut sessions = vec![SubagentSession::new("parent", "idle")];
            for id in IDS {
                sessions.push(
                    SubagentSession::new(id, "running")
                        .with_parent("parent")
                        .with_title(id),
                );
            }
            tab.set_sessions("parent", "parent", sessions);
            tab.set_active(true);
            assert_eq!(tab.entries().len(), 7);
            assert_eq!(tab.visible_rows().len(), MAX_DOCK_ROWS);
            for _ in 0..6 {
                tab.down();
            }
            assert_eq!(tab.selected(), 6);
            // Centred: 6 - floor(5 / 2) = 4, clamped to content - viewport = 2.
            assert_eq!(tab.scroll_top(), 2);
            let visible: Vec<String> = tab
                .visible_rows()
                .into_iter()
                .map(|row| row.session_id)
                .collect();
            assert_eq!(visible, vec!["a2", "a3", "a4", "a5", "a6"]);
            // Moving back up recentres.
            tab.up();
            assert_eq!(tab.selected(), 5);
            assert_eq!(tab.scroll_top(), 2);
            for _ in 0..5 {
                tab.up();
            }
            assert_eq!(tab.scroll_top(), 0);
            assert_eq!(tab.visible_rows()[0].session_id, "a0");
        }

        // CP-24: agent/title derivation from session facts and the title
        // capture.
        #[test]
        fn cp24_agent_title_derivation() {
            let mut tab = SubagentsTab::new();
            tab.set_sessions(
                "parent",
                "parent",
                vec![
                    SubagentSession::new("parent", "idle"),
                    SubagentSession::new("plain", "running")
                        .with_parent("parent")
                        .with_title("Plain child"),
                    SubagentSession::new("captured", "running")
                        .with_parent("parent")
                        .with_title("@explore subagent inspect the tree"),
                    SubagentSession::new("captured-only", "running")
                        .with_parent("parent")
                        .with_title("@review subagent"),
                    SubagentSession::new("timestamped", "running")
                        .with_parent("parent")
                        .with_created(0),
                ],
            );
            let rows = tab.entries();
            let find = |id: &str| {
                rows.iter()
                    .find(|row| row.session_id == id)
                    .cloned()
                    .unwrap()
            };
            assert_eq!(find("plain").agent, "Subagent");
            assert_eq!(find("plain").title.as_deref(), Some("Plain child"));
            assert_eq!(find("captured").agent, "Explore");
            assert_eq!(find("captured").title.as_deref(), Some("inspect the tree"));
            assert_eq!(find("captured-only").agent, "Review");
            // The stripped title is empty, so upstream keeps the full title.
            assert_eq!(
                find("captured-only").title.as_deref(),
                Some("@review subagent")
            );
            // `withTimestampedFallback` for a child session without a title.
            assert_eq!(
                find("timestamped").title.as_deref(),
                Some("Child session - 1970-01-01T00:00:00.000Z")
            );
            // A session agent wins over the title capture and is titlecased.
            let mut agent_tab = SubagentsTab::new();
            agent_tab.set_sessions(
                "parent",
                "parent",
                vec![
                    SubagentSession::new("parent", "idle"),
                    SubagentSession::new("both", "running")
                        .with_parent("parent")
                        .with_title("@explore subagent work")
                        .with_agent("build agent"),
                ],
            );
            let row = agent_tab.entries().pop().unwrap();
            assert_eq!(row.agent, "Build Agent");
            assert_eq!(row.title.as_deref(), Some("work"));
        }

        // CP-25: Running is a literal label with no progress or spinner, and
        // idle rows render no status cell at all.
        #[test]
        fn cp25_running_label_is_literal_without_progress() {
            let mut tab = tab();
            let rows = tab.rows();
            assert_eq!(rows[0].status, "Running");
            // The first row is focused by default (`store.selected` starts at
            // 0), so it paints the focused status colour; unfocused running
            // rows use `text.muted` (`subagents-tab.tsx:244-249`).
            assert_eq!(rows[0].status_style, DockStyle::SubagentStatusActive);
            assert_eq!(rows[1].status_style, DockStyle::SubagentStatusMuted);
            let rendered: String = rows
                .iter()
                .map(|row| format!("{}{}", row.label, row.status))
                .collect();
            assert_eq!(rendered.matches("Running").count(), 2);
            assert!(!rendered.contains('%'));
            assert!(!rendered.contains("spinner"));
            assert!(!rendered.contains("progress"));
            // Idle (inactive-filter) rows have an empty status label.
            tab.toggle_activity();
            let idle = tab.rows();
            assert_eq!(idle.len(), 1);
            assert_eq!(idle[0].status, "");
            // Row paint: the focused label is bold; the status cell never is.
            let mut focused = self::tab();
            focused.set_active(true);
            focused.down();
            let running = focused.rows();
            assert!(running[1].active);
            let (style, bold) = SubagentsTab::row_style(&running[1]);
            assert_eq!(style, DockStyle::SubagentActive);
            assert!(bold, "the focused row label is BOLD");
            let (style, bold) = SubagentsTab::row_style(&running[0]);
            assert_eq!(style, DockStyle::SubagentBase);
            assert!(!bold);
            assert_eq!(running[0].status_style, DockStyle::SubagentStatusMuted);
            assert_eq!(running[1].status_style, DockStyle::SubagentStatusActive);
        }

        // CP-25 (and CP-24): pointer actions plus the exact empty label.
        #[test]
        fn cp25_pointer_actions_and_rows() {
            let mut tab = tab();
            assert_eq!(tab.hover(1), SubagentAction::Selected(1));
            assert_eq!(tab.hover(9), SubagentAction::None);
            assert_eq!(tab.click(0), SubagentAction::Navigate("child-a".into()));
            assert_eq!(tab.empty_text(), " No active subagents");
            let mut empty = SubagentsTab::new();
            empty.set_sessions(
                "parent",
                "parent",
                vec![SubagentSession::new("parent", "idle")],
            );
            assert!(empty.entries().is_empty());
            assert_eq!(empty.rows().len(), 0);
        }
    }
}

/// Running-child facts for the prompt footer live label
/// (`packages/tui/src/feature-plugins/prompt/footer.tsx:22-28`): family
/// members other than this session whose status is `running`.
pub mod live {
    use serde_json::Value;

    /// One retained `/children` row. Unlike the footer count this keeps the
    /// structural facts (parent, agent, description, state) the composer dock
    /// needs; nothing is synthesized when the server did not report a field.
    #[derive(Clone, Debug, PartialEq, Eq)]
    pub struct ChildRow {
        pub session_id: String,
        pub parent_id: Option<String>,
        pub agent: Option<String>,
        pub description: Option<String>,
        pub state: String,
    }

    impl ChildRow {
        fn from_value(row: &Value) -> Result<Self, String> {
            let session_id = row["session_id"].as_str().ok_or("child id missing")?;
            session_id
                .parse::<opencode_rk_contracts::SessionId>()
                .map_err(|_| "invalid child id".to_owned())?;
            let string = |key: &str| {
                row[key]
                    .as_str()
                    .filter(|value| !value.is_empty())
                    .map(str::to_owned)
            };
            Ok(Self {
                session_id: session_id.to_owned(),
                parent_id: string("parent_id"),
                agent: string("agent"),
                description: string("description"),
                // Missing state is reported as "unknown", exactly the literal
                // the server uses when the child state is unavailable.
                state: string("state").unwrap_or_else(|| "unknown".to_owned()),
            })
        }

        #[must_use]
        pub fn is_running(&self) -> bool {
            self.state == "running"
        }
    }

    /// Last observed listing. An unavailable or malformed listing is cleared
    /// rather than left stale, so the painted count is always observed data.
    #[derive(Clone, Debug, Default, PartialEq, Eq)]
    pub struct Status {
        session: Option<String>,
        running: usize,
        first: Option<String>,
        rows: Vec<ChildRow>,
    }

    impl Status {
        /// Bind a listing to `session`. Returns `Ok(true)` when the painted
        /// facts changed.
        pub fn apply(&mut self, session: &str, value: &Value) -> Result<bool, String> {
            let values = value["children"]
                .as_array()
                .ok_or("invalid child listing")?;
            if values.len() > 128 {
                return Err("child listing exceeds limit".into());
            }
            let mut rows = Vec::with_capacity(values.len());
            let mut running = 0usize;
            let mut first: Option<String> = None;
            for value in values {
                let row = ChildRow::from_value(value)?;
                if row.session_id == session || !row.is_running() {
                    rows.push(row);
                    continue;
                }
                running += 1;
                if first.is_none() {
                    first = Some(row.session_id.clone());
                }
                rows.push(row);
            }
            let next = Self {
                session: Some(session.to_owned()),
                running,
                first,
                rows,
            };
            let changed = *self != next;
            *self = next;
            Ok(changed)
        }

        /// Drop the facts (daemon unavailable, malformed listing or no active
        /// session). Returns whether anything was painted before.
        pub fn clear(&mut self) -> bool {
            let changed = *self != Self::default();
            *self = Self::default();
            changed
        }

        #[must_use]
        pub fn running(&self) -> usize {
            self.running
        }

        #[must_use]
        pub fn first(&self) -> Option<&str> {
            self.first.as_deref()
        }

        /// Session the listing is bound to, if any.
        #[must_use]
        pub fn session(&self) -> Option<&str> {
            self.session.as_deref()
        }

        /// Retained listing rows in server order (running and inactive).
        #[must_use]
        pub fn rows(&self) -> &[ChildRow] {
            &self.rows
        }
    }

    #[cfg(test)]
    mod tests {
        use super::*;
        use serde_json::json;

        const PARENT: &str = "11111111-1111-1111-1111-111111111111";
        const A: &str = "22222222-2222-2222-2222-222222222222";
        const B: &str = "33333333-3333-3333-3333-333333333333";
        const C: &str = "44444444-4444-4444-4444-444444444444";
        const A1: &str = "55555555-5555-5555-5555-555555555555";

        #[test]
        fn counts_only_running_family_members_other_than_the_session() {
            let mut status = Status::default();
            let listing = json!({"children": [
                {"session_id": PARENT, "state": "running"},
                {"session_id": A, "state": "running"},
                {"session_id": B, "state": "idle"},
                {"session_id": C, "state": "running"},
            ]});
            assert_eq!(status.apply(PARENT, &listing), Ok(true));
            assert_eq!(status.running(), 2);
            assert_eq!(status.first(), Some(A));
            // Unchanged listings do not request a repaint.
            assert_eq!(status.apply(PARENT, &listing), Ok(false));
        }

        #[test]
        fn nesting_does_not_double_count_and_empty_listing_clears() {
            let mut status = Status::default();
            let listing = json!({"children": [
                {"session_id": A, "state": "running"},
                {"session_id": A1, "parent_id": A, "state": "running"},
            ]});
            assert_eq!(status.apply(PARENT, &listing), Ok(true));
            assert_eq!(status.running(), 2);
            assert_eq!(status.first(), Some(A));

            let empty = json!({"children": []});
            assert_eq!(status.apply(PARENT, &empty), Ok(true));
            assert_eq!(status.running(), 0);
            assert_eq!(status.first(), None);
            // The listing is still bound to its session, so clearing the
            // binding is a change; a second clear is not.
            assert!(status.clear());
            assert!(!status.clear());
        }

        #[test]
        fn malformed_listing_is_rejected_without_changing_facts() {
            let mut status = Status::default();
            let listing = json!({"children": [{"session_id": A, "state": "running"}]});
            assert_eq!(status.apply(PARENT, &listing), Ok(true));
            assert!(status.apply(PARENT, &json!({"rows": []})).is_err());
            assert!(status.apply(PARENT, &json!({"children": [{"state": "running"}]})).is_err());
            assert!(status.apply(PARENT, &json!({"children": [{"session_id": "bad id"}]})).is_err());
            assert_eq!(status.running(), 1, "rejections never publish new facts");
            assert!(status.clear());
            assert_eq!(status, Status::default());
        }
    }
}
