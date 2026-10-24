#![forbid(unsafe_code)]
//! Bounded display model for authenticated session instruction sources.

const MAX_SOURCES: usize = 64;
const MAX_PATH_BYTES: usize = 4096;
const MAX_SOURCE_BYTES: usize = 64 * 1024;
const MAX_TOTAL_BYTES: usize = 512 * 1024;

#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct Source {
    pub(crate) path: String,
    pub(crate) content: String,
    pub(crate) included: bool,
    pub(crate) reason: Option<String>,
}

#[derive(Debug, Default)]
pub(crate) struct Memory {
    sources: Vec<Source>,
    selected: Option<String>,
    content_offset: usize,
    error: Option<String>,
}

impl Memory {
    pub(crate) fn new() -> Self {
        Self::default()
    }

    pub(crate) fn sources(&self) -> &[Source] {
        &self.sources
    }

    pub(crate) fn selected(&self) -> Option<&Source> {
        self.sources.iter().find(|source| Some(source.path.as_str()) == self.selected.as_deref())
    }

    pub(crate) fn selected_index(&self) -> Option<usize> {
        self.sources.iter().position(|source| Some(source.path.as_str()) == self.selected.as_deref())
    }

    pub(crate) fn error(&self) -> Option<&str> {
        self.error.as_deref()
    }

    pub(crate) fn set_error(&mut self, error: &str) {
        self.error = Some(error.chars().take(512).collect());
    }

    /// Refuse malformed or oversized snapshots atomically; selection uses the
    /// exact backend path and is retained only when that source still exists.
    pub(crate) fn set_sources(&mut self, sources: Vec<Source>) -> Result<(), String> {
        if sources.len() > MAX_SOURCES {
            return Err("memory source listing exceeds 64 entries".to_owned());
        }
        let mut total = 0usize;
        let mut seen = std::collections::HashSet::with_capacity(sources.len());
        for source in &sources {
            if source.path.is_empty() || source.path.len() > MAX_PATH_BYTES
                || source.path.chars().any(char::is_control)
            {
                return Err("memory source path is invalid".to_owned());
            }
            if !seen.insert(source.path.as_str()) {
                return Err("memory source listing contains duplicate paths".to_owned());
            }
            if source.content.len() > MAX_SOURCE_BYTES {
                return Err("memory source content exceeds 65536 bytes".to_owned());
            }
            total = total.saturating_add(source.path.len()).saturating_add(source.content.len());
            if total > MAX_TOTAL_BYTES {
                return Err("memory source content exceeds 524288 bytes".to_owned());
            }
            if source.reason.as_ref().is_some_and(|reason| reason.chars().any(char::is_control)) {
                return Err("memory source reason is invalid".to_owned());
            }
        }
        let selected = self.selected.clone().filter(|path| seen.contains(path.as_str()));
        self.sources = sources;
        self.selected = selected.or_else(|| self.sources.first().map(|source| source.path.clone()));
        self.content_offset = 0;
        self.error = None;
        Ok(())
    }

    pub(crate) fn load_response(&mut self, body: &str) -> Result<(), String> {
        let value: serde_json::Value = serde_json::from_str(body)
            .map_err(|_| "memory response is not valid JSON".to_owned())?;
        let entries = value.get("sources").and_then(serde_json::Value::as_array)
            .ok_or_else(|| "memory response is missing sources".to_owned())?;
        let mut sources = Vec::with_capacity(entries.len().min(MAX_SOURCES));
        for entry in entries {
            let path = entry.get("path").and_then(serde_json::Value::as_str)
                .ok_or_else(|| "memory source is missing its path".to_owned())?;
            let content = entry.get("content").and_then(serde_json::Value::as_str)
                .ok_or_else(|| "memory source is missing its content".to_owned())?;
            let included = entry.get("included").and_then(serde_json::Value::as_bool)
                .ok_or_else(|| "memory source is missing its inclusion state".to_owned())?;
            let reason = match entry.get("reason") {
                None | Some(serde_json::Value::Null) => None,
                Some(serde_json::Value::String(reason)) => Some(reason.clone()),
                Some(_) => return Err("memory source reason is invalid".to_owned()),
            };
            sources.push(Source {
                path: path.to_owned(),
                content: content.to_owned(),
                included,
                reason,
            });
        }
        self.set_sources(sources)
    }

    pub(crate) fn select_next(&mut self, forward: bool) {
        if self.sources.is_empty() { return; }
        let current = self.selected_index().unwrap_or(0);
        let next = if forward {
            (current + 1) % self.sources.len()
        } else {
            current.checked_sub(1).unwrap_or(self.sources.len() - 1)
        };
        self.selected = Some(self.sources[next].path.clone());
        self.content_offset = 0;
    }

    pub(crate) fn scroll_content(&mut self, lines: isize, width: usize, height: usize) {
        let Some(source) = self.selected() else { return };
        let content_height = height.saturating_sub(4);
        let total = wrap_content(&source.content, width).len();
        let maximum = total.saturating_sub(content_height);
        self.content_offset = self.content_offset.saturating_add_signed(lines).min(maximum);
    }

    /// Toggle intent. The returned endpoint/body are sent by the authenticated
    /// HTTP caller; this model does not change state until the response parses.
    pub(crate) fn toggle_request(&self) -> Option<(&'static str, String)> {
        let source = self.selected()?;
        let endpoint = if source.included { "exclude" } else { "include" };
        let body = serde_json::json!({"path": source.path, "scope": "session"}).to_string();
        Some((endpoint, body))
    }

    pub(crate) fn render(&self, width: usize, height: usize) -> Vec<String> {
        if width == 0 || height == 0 { return Vec::new(); }
        let mut lines = Vec::with_capacity(height);
        lines.push(format!("Memory - {} source(s)", self.sources.len()));
        if let Some(error) = &self.error {
            lines.push(format!("error: {error}"));
            lines.push("No changes applied without a backend acknowledgement".to_owned());
        } else if let Some(source) = self.selected() {
            let index = self.selected_index().unwrap_or(0) + 1;
            let state = if source.included { "included" } else { "excluded" };
            let reason = source.reason.as_deref().map(|reason| format!(" ({reason})")).unwrap_or_default();
            lines.push(format!("Source {index}/{} - {state}{reason}", self.sources.len()));
            lines.push(display_safe(&source.path));
            let content_height = height.saturating_sub(4);
            let content = wrap_content(&source.content, width);
            let max_offset = content.len().saturating_sub(content_height);
            let offset = self.content_offset.min(max_offset);
            for line in content.iter().skip(offset).take(content_height) {
                lines.push(line.clone());
            }
            if content.is_empty() {
                lines.push("(empty source)".to_owned());
            }
        } else {
            lines.push("No instruction sources found".to_owned());
            lines.push(String::new());
        }
        while lines.len() + 1 < height { lines.push(String::new()); }
        lines.push("Up/Down source · Ctrl-F/B scroll · Enter toggle · Esc close".to_owned());
        lines.truncate(height);
        lines.into_iter().map(|line|
            crate::native_view_layout::fit(&display_safe(&line), width.min(u16::MAX as usize) as u16)
        ).collect()
    }
}

/// Display-only, control-safe view of the exact backend content. CRLF is
/// normalized, tabs become spaces, terminal escapes/control codes never pass.
fn display_safe(text: &str) -> String {
    text.chars().filter_map(|ch| match ch {
        '\r' => None,
        '\t' => Some("    ".to_owned()),
        '\n' => Some("\n".to_owned()),
        c if c.is_control() => Some("�".to_owned()),
        c => Some(c.to_string()),
    }).collect()
}

/// Wrap source text into viewport-width display rows before applying scroll.
fn wrap_content(text: &str, width: usize) -> Vec<String> {
    if width == 0 { return Vec::new(); }
    let safe = display_safe(text);
    let mut lines = Vec::new();
    for input in safe.split('\n') {
        let mut line = String::new();
        let mut used = 0usize;
        crate::native_view_layout::visit_unpadded_cells(input, MAX_SOURCE_BYTES * 4, |glyph, cells| {
            let (glyph, cells) = if cells > width { ("�", 1) } else { (glyph, cells) };
            if used > 0 && used.saturating_add(cells) > width {
                lines.push(std::mem::take(&mut line));
                used = 0;
            }
            line.push_str(glyph);
            used = used.saturating_add(cells);
        });
        lines.push(line);
    }
    lines
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn scrolling_reaches_the_end_of_a_single_long_instruction_line() {
        let mut memory = Memory::new();
        let content = format!("{}TAIL_SENTINEL", "x".repeat(640));
        memory.load_response(&response(&content, true)).unwrap();
        memory.scroll_content(isize::MAX, 32, 10);
        assert!(memory.render(32, 10).join("\n").contains("TAIL_SENTINEL"),
            "wrapping must not clip the source before the content viewport scrolls");
    }

    #[test]
    fn source_labels_and_content_stay_inside_a_wide_character_viewport() {
        let mut memory = Memory::new();
        memory.load_response(&response(&"界".repeat(40), true)).unwrap();
        for row in memory.render(17, 12) {
            let cells = crate::native_view_layout::visit_unpadded_cells(&row, 4096, |_, _| {});
            assert!(cells <= 17, "memory row exceeded its cell budget: {row:?}");
        }
    }

    fn response(content: &str, included: bool) -> String {
        serde_json::json!({"sources":[{
            "path":"/workspace/AGENTS.md", "content":content,
            "included":included, "reason":if included { serde_json::Value::Null } else { serde_json::json!("session_excluded") }
        }]}).to_string()
    }

    #[test]
    fn snapshots_render_real_content_and_scroll_without_losing_selection() {
        let mut memory = Memory::new();
        memory.load_response(&response("first line\nsecond line", true)).unwrap();
        assert_eq!(memory.selected().unwrap().content, "first line\nsecond line");
        let before = memory.render(40, 8).join("\n");
        assert!(before.contains("/workspace/AGENTS.md"));
        assert!(before.contains("first line"));
        assert!(before.contains("second line"));
        memory.scroll_content(1, 40, 5);
        let after = memory.render(40, 5).join("\n");
        assert!(after.contains("second line"));
        assert_eq!(memory.selected_index(), Some(0));
    }

    #[test]
    fn toggle_waits_for_valid_backend_snapshot_and_uses_session_scope() {
        let mut memory = Memory::new();
        memory.load_response(&response("read-only source", true)).unwrap();
        let (endpoint, body) = memory.toggle_request().unwrap();
        assert_eq!(endpoint, "exclude");
        let payload: serde_json::Value = serde_json::from_str(&body).unwrap();
        assert_eq!(payload["path"], "/workspace/AGENTS.md");
        assert_eq!(payload["scope"], "session");
        assert!(memory.selected().unwrap().included, "intent is not an ACK");
        assert!(memory.load_response("not json").is_err());
        assert!(memory.selected().unwrap().included, "failed ACK retains state");
        memory.load_response(&response("read-only source", false)).unwrap();
        assert!(!memory.selected().unwrap().included, "only returned state applies");
        assert_eq!(memory.toggle_request().unwrap().0, "include");
    }

    #[test]
    fn malformed_duplicate_oversized_and_terminal_control_sources_refuse_atomically() {
        let mut memory = Memory::new();
        memory.load_response(&response("safe", true)).unwrap();
        for body in [
            r#"{"sources":[{"path":"/p","content":"x"}]}"#,
            r#"{"sources":[{"path":"/p","content":"x","included":true},{"path":"/p","content":"y","included":false}]}"#,
            &serde_json::json!({"sources": vec![serde_json::json!({"path":"/p","content":"x"}); MAX_SOURCES + 1]}).to_string(),
        ] {
            assert!(memory.load_response(body).is_err());
            assert_eq!(memory.selected().unwrap().content, "safe");
        }
        let controls = display_safe("ok\u{1b}[31m red\tline\r\nnext");
        assert!(!controls.contains('\u{1b}'));
        assert!(controls.contains("�[31m red    line\nnext"));
    }
}
