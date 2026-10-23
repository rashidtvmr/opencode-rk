//! Keyboard/mouse tool-rule editor. All mutations require a daemon ACK.
#![forbid(unsafe_code)]
use crate::native_ui_style::{Line, Role};
const TOOLS: [&str; 10] = ["bash", "shell", "read", "write", "edit", "grep", "glob", "websearch", "webfetch", "task"];
const ACTIONS: [(&str, &str); 3] = [("allow", "1 Allow"), ("ask", "2 Ask"), ("deny", "3 Deny")];
const ACTION_GAP: &str = "    ";
pub struct Editor {
    pub open: bool,
    pub selected: usize,
    pub persistent: bool,
    pub error: Option<String>,
    rules: serde_json::Value,
}
impl Default for Editor {
    fn default() -> Self { Self {open:false,selected:0,persistent:false,error:None,rules:serde_json::json!({"rules":[]})} }
}
struct Layout {
    lines: Vec<Line>,
    hits: Vec<Option<usize>>,
    action_row: usize,
    action_spans: [(usize, usize); 3],
}
impl Editor {
    pub fn apply(&mut self, rules: serde_json::Value) { self.rules = rules; self.error = None; }
    pub fn navigate(&mut self, down: bool) {
        self.selected = if down { (self.selected + 1) % TOOLS.len() } else { (self.selected + TOOLS.len() - 1) % TOOLS.len() };
    }
    pub fn payload(&self, decision: &str, session: &str) -> serde_json::Value {
        serde_json::json!({"tool":TOOLS[self.selected],"decision":decision,"scope":if self.persistent {"always"} else {"session"},"session_id":session})
    }
    fn action_line() -> (String, [(usize, usize); 3]) {
        let mut text = String::new();
        let mut spans = [(0, 0); 3];
        for (index, (_, label)) in ACTIONS.iter().enumerate() {
            if index > 0 { text.push_str(ACTION_GAP); }
            spans[index].0 = text.len();
            text.push_str(label);
            spans[index].1 = text.len();
        }
        (text, spans)
    }
    fn layout(&self, height: usize) -> Layout {
        let mut lines = Vec::<Line>::new();
        let mut hits = Vec::new();
        {
            let mut push = |text:String,role,hit| { lines.push(Line {text,role,detail_from:None}); hits.push(hit); };
            push("Tool permissions".into(),Role::Heading,None);
            push(format!("Scope: {} · Tab change",if self.persistent {"Always (saved)"} else {"This session"}),Role::Description,None);
            let capacity = height.saturating_sub(5);
            let start = self.selected.saturating_sub(capacity.saturating_sub(1));
            for (index, tool) in TOOLS.iter().enumerate().skip(start).take(capacity) {
                let rule = self.rules.get("rules").and_then(serde_json::Value::as_array)
                    .and_then(|rules| rules.iter().rev().find(|rule| rule["tool"] == *tool
                        && rule["scope"].as_str().unwrap_or("always") == if self.persistent {"always"} else {"session"}));
                let decision = rule.and_then(|r| r["decision"].as_str()).unwrap_or("ask");
                push(format!("{} {:<10}  {}",if self.selected == index {">"} else {" "},tool,decision),
                    if self.selected == index {Role::Selected} else {Role::Item},Some(index));
            }
            push(self.error.clone().unwrap_or_else(|| "Rules never override protected operations.".into()),
                if self.error.is_some() {Role::Error} else {Role::Description},None);
        }
        let action_row = lines.len();
        let (action, action_spans) = Self::action_line();
        lines.push(Line {text:action,role:Role::Item,detail_from:None});
        hits.push(None);
        lines.push(Line {text:"↑↓/click tool · 1/2/3 set · Tab scope · Esc close".into(),role:Role::Hint,detail_from:None});
        hits.push(None);
        Layout {lines,hits,action_row,action_spans}
    }
    pub fn rows(&self, width:u16, height:usize) -> (Vec<String>, Vec<Role>, Vec<Option<usize>>) {
        let layout = self.layout(height);
        let limit = height.min(layout.lines.len());
        let rows = layout.lines.iter().take(limit).map(|line|crate::native_view_layout::fit(&line.text,width)).collect();
        let roles = layout.lines.iter().take(limit).map(|line|line.role).collect();
        let mut hits = layout.hits;
        hits.truncate(limit);
        (rows,roles,hits)
    }
    pub fn click_action(&mut self, x:u16, row:usize, width:u16,height:usize) -> Option<&'static str> {
        let layout = self.layout(height);
        if row >= height.min(layout.lines.len()) || x >= width { return None; }
        if let Some(index) = layout.hits.get(row).copied().flatten() { self.selected = index; return None; }
        if row == 1 {self.persistent = !self.persistent; return None;}
        if row != layout.action_row { return None; }
        let x = usize::from(x);
        ACTIONS.iter().enumerate().find_map(|(index,(action,_))| {
            let (start,end) = layout.action_spans[index];
            (x >= start && x < end && end <= usize::from(width)).then_some(*action)
        })
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn rule_edits_are_explicit_and_scope_is_bound_to_current_session() {
        let mut editor=Editor::default();
        assert_eq!(editor.payload("deny","s")["scope"],"session");
        editor.persistent=true;editor.navigate(true);
        assert_eq!(editor.payload("allow","s")["tool"],"shell");
        assert_eq!(editor.payload("allow","s")["session_id"],"s");
        assert_eq!(editor.payload("allow","s")["scope"],"always");
    }
    #[test]
    fn compact_editor_keeps_selection_and_mouse_actions_visible() {
        let mut editor=Editor::default();
        for _ in 0..9 {editor.navigate(true);}
        let (rows,roles,hits)=editor.rows(52,12);
        assert!(rows.len()<=12);
        let row=roles.iter().position(|role|*role==Role::Selected).unwrap();
        assert_eq!(hits[row],Some(9));
        assert_eq!(editor.click_action(1,rows.len()-2,52,12),Some("allow"));
        assert_eq!(editor.click_action(13,rows.len()-2,52,12),Some("ask"));
        assert_eq!(editor.click_action(23,rows.len()-2,52,12),Some("deny"));
    }
    #[test]
    fn rendered_action_hit_map_matches_numeric_keyboard_labels() {
        let mut editor=Editor::default();
        let (rows,_,_)=editor.rows(52,12);
        let action_row=rows.iter().position(|row|row.contains(ACTIONS[0].1)).unwrap();
        let labels=ACTIONS.iter().map(|(_,label)|*label).collect::<Vec<_>>().join(ACTION_GAP);
        assert_eq!(rows[action_row].trim_end(),labels.as_str());
        for (index,(decision,label)) in ACTIONS.iter().enumerate() {
            assert_eq!(label.chars().next(),Some(char::from(b'1'+index as u8)));
            assert_eq!(editor.payload(decision,"s")["decision"],*decision);
            let start=rows[action_row].find(*label).unwrap();
            for x in start..start+label.len() {
                assert_eq!(editor.click_action(x as u16,action_row,52,12),Some(*decision),"x={x}");
            }
        }
        assert_eq!(editor.click_action(7,action_row,52,12),None);
        assert_eq!(editor.click_action(8,action_row,52,12),None);
        assert_eq!(editor.click_action(16,action_row,52,12),None);
        assert_eq!(editor.click_action(19,action_row,52,12),None);
        assert_eq!(editor.click_action(52,action_row,52,12),None);
        assert_eq!(editor.selected,0);
    }
    #[test]
    fn truncated_or_hidden_action_rows_never_apply_rules() {
        let mut editor=Editor::default();
        let (rows,_,_)=editor.rows(13,6);
        let action_row=rows.iter().position(|row|row.contains("1 Allow")).unwrap();
        assert!(!rows[action_row].contains("2 Ask"));
        assert_eq!(editor.click_action(0,action_row,13,6),Some("allow"));
        assert_eq!(editor.click_action(11,action_row,13,6),None);
        assert_eq!(editor.click_action(12,action_row,13,6),None);

        let (narrow,_,_)=editor.rows(4,6);
        let action_row=narrow.iter().position(|row|row.starts_with("1 Al")).unwrap();
        assert_eq!(editor.click_action(0,action_row,4,6),None);

        let (short,_,_)=editor.rows(52,3);
        assert_eq!(short.len(),3);
        assert_eq!(editor.selected,0);
        assert_eq!(editor.click_action(0,2,52,3),None);
        assert_eq!(editor.selected,0);
        assert_eq!(editor.click_action(0,3,52,3),None);

        let (tiny,_,_)=editor.rows(52,1);
        assert_eq!(tiny.len(),1);
        assert!(!editor.persistent);
        assert_eq!(editor.click_action(0,1,52,1),None);
        assert!(!editor.persistent);

        for height in 0..8 {
            let (bounded,_,_)=editor.rows(52,height);
            assert!(bounded.len()<=height);
        }
    }
    #[test]
    fn scope_row_click_matches_tab_scope_toggle() {
        let mut editor=Editor::default();
        let (rows,_,_)=editor.rows(52,12);
        assert!(rows[1].starts_with("Scope: This session"));
        assert_eq!(editor.click_action(0,1,52,12),None);
        assert!(editor.persistent);
        assert!(editor.rows(52,12).0[1].starts_with("Scope: Always (saved)"));
        assert_eq!(editor.click_action(0,1,52,12),None);
        assert!(!editor.persistent);
        assert_eq!(editor.click_action(60,1,52,12),None);
        assert!(!editor.persistent);
    }
}
