//! Native child-task roster and typed history viewer. The caller owns HTTP;
//! this component emits explicit controls and never invents lifecycle state.
use serde_json::Value;
use crate::native_view_chat::{ChatMessage, ChatView};
use crate::native_view_layout::fit;

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
}

#[cfg(test)]
mod tests {
    use super::*;
    use serde_json::json;
    fn panel() -> Panel {
        let mut panel = Panel::default();
        panel.show("parent");
        panel.update(&json!({"children":[{"session_id":"11111111-1111-1111-1111-111111111111","agent":"explore","state":"running","description":"inspect"}]})).unwrap();
        panel
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
}
pub enum Action { Stop(String), Retry(String), Followup(String, String) }

impl Panel {
    pub fn show(&mut self, parent: &str) {
        *self = Self { open:true, parent:parent.into(), ..Self::default() };
    }
    pub fn update(&mut self, value: &Value) -> Result<(), String> {
        let rows = value["children"].as_array().ok_or("invalid child listing")?;
        if rows.len() > 128 { return Err("child listing exceeds limit".into()); }
        let selected = self.selected_id();
        for row in rows {
            let id = row["session_id"].as_str().ok_or("child id missing")?;
            if id.len() != 36 || !id.bytes().all(|b| b.is_ascii_hexdigit() || b == b'-') { return Err("invalid child id".into()); }
        }
        self.rows = rows.clone();
        self.selected = selected.and_then(|id| self.rows.iter().position(|row| row["session_id"] == id)).unwrap_or(self.selected).min(self.rows.len().saturating_sub(1));
        self.error = None;
        Ok(())
    }
    pub fn history(&mut self, value: &Value) {
        if let Some(messages) = value["messages"].as_array() {
            self.history = crate::native_chat_projection::project_history(messages);
        }
    }
    pub fn selected_id(&self) -> Option<String> {
        self.rows.get(self.selected)?.get("session_id")?.as_str().map(str::to_owned)
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
                    let id = self.selected_id()?;
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
            b's' => return self.selected_id().map(Action::Stop),
            b'r' => return self.selected_id().map(Action::Retry),
            b'c' if !self.rows.is_empty() => { self.detail = true; self.composing = true; }
            _ => {}
        }
        None
    }
    pub fn render(&self, width: u16, height: usize) -> Vec<String> {
        if height == 0 { return Vec::new(); }
        let mut lines = vec!["Subagents".into(), String::new()];
        let footer = if self.composing { "Enter send follow-up · Esc cancel" }
            else if self.detail { "↑/↓ scroll · s stop · r retry · c follow-up · Esc back" }
            else { "↑/↓ select · Enter detail · s stop · r retry · Esc close" };
        if let Some(error) = self.action_error.as_ref().or(self.error.as_ref()) { lines.push(format!("Error: {error}")); }
        if self.detail {
            if let Some(row) = self.rows.get(self.selected) {
                lines.push(format!("{} · {} · {}", row["agent"].as_str().unwrap_or("?"), row["state"].as_str().unwrap_or("unknown"), row["model"].as_str().unwrap_or("?")));
                lines.push(format!("Session: {}", row["session_id"].as_str().unwrap_or("?")));
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
            let capacity = height.saturating_sub(lines.len() + 2);
            let start = self.selected.saturating_sub(capacity.saturating_sub(1));
            for (index, row) in self.rows.iter().enumerate().skip(start).take(capacity) {
                let state = if row["awaiting_permission"] == true { "approval needed" } else { row["state"].as_str().unwrap_or("unknown") };
                lines.push(format!("{} [{}] {} — {}", if index == self.selected { ">" } else { " " }, state,
                    row["agent"].as_str().unwrap_or("?"), row["description"].as_str().unwrap_or("?")));
            }
        }
        lines.truncate(height.saturating_sub(2).max(1));
        if height > 2 { lines.push(String::new()); }
        if height > 1 { lines.push(footer.into()); }
        // fit strips ANSI/control sequences with a bounded scan and clips by
        // terminal cells; daemon text never goes to the renderer unfiltered.
        lines.into_iter().take(height).map(|line| fit(&line, width)).collect()
    }
}
