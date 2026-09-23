#![forbid(unsafe_code)]
//! Keybind definition + command tables verbatim from TS.
//!
//! TS sources (local checkout /home/rashid/projects/opencode @ a0d9b6c,
//! diverges from pinned 95daf90; line numbers below are a0d9b6c):
//! - `packages/tui/src/config/keybind.ts:17-25` BindingObject: key
//!   (string | KeyStroke), event `press | release` (default press),
//!   preventDefault, fallthrough.
//! - `keybind.ts:27-33` BindingItem forms: string | KeyStroke |
//!   BindingObject; value = `false | "none" | item | item[]`.
//! - `keybind.ts:41` LeaderDefault `"ctrl+x"` (see [`crate::keymap`]).
//! - `keybind.ts:45-240` Definitions table (184 `keybind()` entries).
//! - `keybind.ts:162` only object-form default:
//!   `input_paste` = `{ key: "ctrl+v", preventDefault: false }`.
//! - `keybind.ts:256-420` CommandMap (163 name -> command entries;
//!   `leader` + 20 `dialog.*` / `prompt.autocomplete.*` /
//!   `permission.*` / `plugins.toggle` / `dialog.plugins.install`
//!   names have no command).
//! - `keybind.ts:421-426` CommandDescriptions (command -> description).
//! - `keybind.ts:449-458` `parse`: defaults + overrides.
//! - `keybind.ts:462-464` `unknownKeys` reject.
//!
//! Str-only parsing/mode stack live in [`crate::keymap`] (`BindingValue`,
//! `KeymapConfig`); single-stroke parsing in [`crate::key_event`]. This
//! module adds the object form, the literal tables, and lookups. It does
//! NOT redefine `BindingValue` / `KeymapConfig`.

use core::fmt;

use crate::key_event::{parse_stroke, KeyEvent, KeyPress};
use crate::keymap::{BindingValue, KeymapConfig};

/// Max chars for a [`BindingObject`] key. Mirrors `MAX_STROKE` in
/// `crate::key_event`. Fail-closed: longer rejected.
pub const MAX_KEY_LEN: usize = 64;
/// Max strokes merged by [`parse_binding_array`]. Mirrors `MAX_STROKES`
/// in `crate::keymap` (private there; duplicated, not redefined).
pub const MAX_TABLE_STROKES: usize = 8;

/// Press vs release. TS: `BindingObject.event` (keybind.ts:20).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum KeyEvent2 {
    #[default]
    Press,
    Release,
}

impl From<KeyPress> for KeyEvent2 {
    fn from(e: KeyPress) -> Self {
        match e {
            KeyPress::Press => Self::Press,
            KeyPress::Release => Self::Release,
        }
    }
}

impl From<KeyEvent2> for KeyPress {
    fn from(e: KeyEvent2) -> Self {
        match e {
            KeyEvent2::Press => Self::Press,
            KeyEvent2::Release => Self::Release,
        }
    }
}

/// Object binding form. TS: `BindingObject` (keybind.ts:17-25).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct BindingObject {
    /// Key string, 1..=[`MAX_KEY_LEN`] chars.
    pub key: String,
    /// Defaults to press when unspecified (keybind.ts:20).
    pub event: KeyEvent2,
    /// TS `preventDefault`; defaults true (only `input_paste` sets false,
    /// keybind.ts:162).
    pub prevent_default: bool,
    /// TS `fallthrough`; defaults false.
    pub fallthrough: bool,
}

/// Fail-closed table errors.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum TableError {
    EmptyKey,
    KeyTooLong,
    UnknownEvent,
    BadStroke(crate::key_event::KeyEventError),
    BadBinding(crate::keymap::KeymapError),
    UnknownKey(String),
    TooManyStrokes,
    InvalidItem,
}

impl fmt::Display for TableError {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        match self {
            Self::EmptyKey => f.write_str("empty binding key"),
            Self::KeyTooLong => f.write_str("binding key too long"),
            Self::UnknownEvent => f.write_str("unknown event (want press|release)"),
            Self::BadStroke(e) => write!(f, "bad stroke: {e}"),
            Self::BadBinding(e) => write!(f, "bad binding: {e}"),
            Self::UnknownKey(k) => write!(f, "unrecognized keybind: {k}"),
            Self::TooManyStrokes => f.write_str("too many strokes in binding"),
            Self::InvalidItem => f.write_str("invalid binding array item"),
        }
    }
}

impl std::error::Error for TableError {}

impl From<crate::key_event::KeyEventError> for TableError {
    fn from(e: crate::key_event::KeyEventError) -> Self {
        Self::BadStroke(e)
    }
}

impl From<crate::keymap::KeymapError> for TableError {
    fn from(e: crate::keymap::KeymapError) -> Self {
        Self::BadBinding(e)
    }
}

/// Parse the object form (keybind.ts:17-25). `event` accepts
/// `press | release` (case-insensitive, default press).
/// Fail-closed on empty/over-long key or unknown event.
pub fn parse_binding_object(
    key: &str,
    event: Option<&str>,
    prevent_default: Option<bool>,
    fallthrough: Option<bool>,
) -> Result<BindingObject, TableError> {
    let key = key.trim();
    if key.is_empty() {
        return Err(TableError::EmptyKey);
    }
    if key.len() > MAX_KEY_LEN {
        return Err(TableError::KeyTooLong);
    }
    let event = match event.map(|e| e.trim().to_ascii_lowercase()) {
        None | Some(_) if event.is_none_or(|e| e.trim().is_empty()) => KeyEvent2::Press,
        Some(e) if e == "press" => KeyEvent2::Press,
        Some(e) if e == "release" => KeyEvent2::Release,
        _ => return Err(TableError::UnknownEvent),
    };
    Ok(BindingObject {
        key: key.to_string(),
        event,
        prevent_default: prevent_default.unwrap_or(true),
        fallthrough: fallthrough.unwrap_or(false),
    })
}

/// Validate an object form's key via [`parse_stroke`] and return the
/// single [`KeyEvent`] with the object's event applied.
pub fn object_key_event(obj: &BindingObject) -> Result<KeyEvent, TableError> {
    let mut ev = parse_stroke(&obj.key)?;
    ev.event = KeyPress::from(obj.event);
    Ok(ev)
}

/// Object form -> [`BindingValue`]. Key must be one valid stroke.
pub fn binding_value_of_object(obj: &BindingObject) -> Result<BindingValue, TableError> {
    Ok(BindingValue::Strokes(vec![object_key_event(obj)?]))
}

/// One array element (keybind.ts:32 `Schema.Array(BindingItem)`).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum BindingArrayItem {
    Str(String),
    Object(BindingObject),
}

/// Array form -> [`BindingValue`]. Str elements may hold comma sequences
/// (merged); object elements contribute one stroke each. `false`/`"none"`
/// inside an array is rejected (TS types array items as `BindingItem`,
/// which excludes them). Fail-closed past [`MAX_TABLE_STROKES`].
pub fn parse_binding_array(items: &[BindingArrayItem]) -> Result<BindingValue, TableError> {
    let mut out = Vec::new();
    for item in items {
        match item {
            BindingArrayItem::Str(s) => match BindingValue::parse(s)? {
                BindingValue::Strokes(strokes) => out.extend(strokes),
                BindingValue::Disabled | BindingValue::Unbound => {
                    return Err(TableError::InvalidItem);
                }
            },
            BindingArrayItem::Object(obj) => out.push(object_key_event(obj)?),
        }
        if out.len() > MAX_TABLE_STROKES {
            return Err(TableError::TooManyStrokes);
        }
    }
    if out.is_empty() {
        return Err(TableError::InvalidItem);
    }
    Ok(BindingValue::Strokes(out))
}

/// One Definitions row: name, default key string, TS `preventDefault`
/// for that default (false only for `input_paste`, keybind.ts:162),
/// description. Verbatim from keybind.ts:45-240.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Definition {
    pub name: &'static str,
    pub default: &'static str,
    pub prevent_default: bool,
    pub description: &'static str,
}

/// Definitions table: 184 entries (keybind.ts:45-240).
pub static DEFINITIONS: &[Definition] = &[
    Definition { name: "leader", default: "ctrl+x", prevent_default: true, description: "Leader key for keybind combinations" },
    Definition { name: "app_exit", default: "ctrl+c,ctrl+d,<leader>q", prevent_default: true, description: "Exit the application" },
    Definition { name: "app_debug", default: "none", prevent_default: true, description: "Toggle debug panel" },
    Definition { name: "app_console", default: "none", prevent_default: true, description: "Toggle console" },
    Definition { name: "app_heap_snapshot", default: "none", prevent_default: true, description: "Write heap snapshot" },
    Definition { name: "app_toggle_animations", default: "none", prevent_default: true, description: "Toggle animations" },
    Definition { name: "app_toggle_file_context", default: "none", prevent_default: true, description: "Toggle file context" },
    Definition { name: "app_toggle_diffwrap", default: "none", prevent_default: true, description: "Toggle diff wrapping" },
    Definition { name: "app_toggle_paste_summary", default: "none", prevent_default: true, description: "Toggle paste summary" },
    Definition { name: "app_toggle_session_directory_filter", default: "none", prevent_default: true, description: "Toggle session directory filtering" },
    Definition { name: "command_list", default: "ctrl+p", prevent_default: true, description: "List available commands" },
    Definition { name: "help_show", default: "none", prevent_default: true, description: "Open help dialog" },
    Definition { name: "docs_open", default: "none", prevent_default: true, description: "Open documentation" },
    Definition { name: "diff_open", default: "none", prevent_default: true, description: "Open diff viewer" },
    Definition { name: "diff_close", default: "escape,q", prevent_default: true, description: "Close diff viewer" },
    Definition { name: "diff_toggle", default: "enter,space", prevent_default: true, description: "Toggle diff viewer item" },
    Definition { name: "diff_expand", default: "right", prevent_default: true, description: "Expand diff viewer item" },
    Definition { name: "diff_expand_all", default: "E", prevent_default: true, description: "Expand all diff viewer folders" },
    Definition { name: "diff_collapse", default: "left", prevent_default: true, description: "Collapse diff viewer item" },
    Definition { name: "diff_switch_focus", default: "tab", prevent_default: true, description: "Switch diff viewer focus" },
    Definition { name: "diff_next_hunk", default: "]", prevent_default: true, description: "Jump to next diff hunk" },
    Definition { name: "diff_previous_hunk", default: "[", prevent_default: true, description: "Jump to previous diff hunk" },
    Definition { name: "diff_next_file", default: "n", prevent_default: true, description: "Jump to next diff file" },
    Definition { name: "diff_previous_file", default: "p", prevent_default: true, description: "Jump to previous diff file" },
    Definition { name: "diff_toggle_file_tree", default: "b", prevent_default: true, description: "Toggle diff viewer file tree" },
    Definition { name: "diff_single_patch", default: "s", prevent_default: true, description: "Toggle single patch view" },
    Definition { name: "diff_switch_source", default: "d", prevent_default: true, description: "Switch diff viewer source" },
    Definition { name: "diff_toggle_view", default: "v", prevent_default: true, description: "Toggle diff viewer split or unified view" },
    Definition { name: "diff_help", default: "?", prevent_default: true, description: "Show more diff viewer shortcuts" },
    Definition { name: "editor_open", default: "<leader>e", prevent_default: true, description: "Open external editor" },
    Definition { name: "theme_list", default: "<leader>t", prevent_default: true, description: "List available themes" },
    Definition { name: "theme_switch_mode", default: "none", prevent_default: true, description: "Switch between light and dark theme mode" },
    Definition { name: "theme_mode_lock", default: "none", prevent_default: true, description: "Lock or unlock theme mode" },
    Definition { name: "sidebar_toggle", default: "<leader>b", prevent_default: true, description: "Toggle sidebar" },
    Definition { name: "scrollbar_toggle", default: "none", prevent_default: true, description: "Toggle session scrollbar" },
    Definition { name: "status_view", default: "<leader>s", prevent_default: true, description: "View status" },
    Definition { name: "debug_view", default: "none", prevent_default: true, description: "View debug info" },
    Definition { name: "session_export", default: "<leader>x", prevent_default: true, description: "Export session to editor" },
    Definition { name: "session_copy", default: "none", prevent_default: true, description: "Copy session transcript" },
    Definition { name: "session_move", default: "none", prevent_default: true, description: "Move session" },
    Definition { name: "session_new", default: "<leader>n", prevent_default: true, description: "Create a new session" },
    Definition { name: "session_list", default: "<leader>l", prevent_default: true, description: "List all sessions" },
    Definition { name: "session_timeline", default: "<leader>g", prevent_default: true, description: "Show session timeline" },
    Definition { name: "session_fork", default: "none", prevent_default: true, description: "Fork session from message" },
    Definition { name: "session_rename", default: "ctrl+r", prevent_default: true, description: "Rename session" },
    Definition { name: "session_delete", default: "ctrl+d", prevent_default: true, description: "Delete session" },
    Definition { name: "session_share", default: "none", prevent_default: true, description: "Share current session" },
    Definition { name: "session_unshare", default: "none", prevent_default: true, description: "Unshare current session" },
    Definition { name: "session_interrupt", default: "escape", prevent_default: true, description: "Interrupt current session" },
    Definition { name: "session_background", default: "ctrl+b", prevent_default: true, description: "Background synchronous subagents" },
    Definition { name: "session_compact", default: "<leader>c", prevent_default: true, description: "Compact the session" },
    Definition { name: "session_toggle_timestamps", default: "none", prevent_default: true, description: "Toggle message timestamps" },
    Definition { name: "session_toggle_generic_tool_output", default: "none", prevent_default: true, description: "Toggle generic tool output" },
    Definition { name: "session_queued_prompts", default: "<leader>q", prevent_default: true, description: "Manage queued prompts" },
    Definition { name: "session_child_first", default: "<leader>down", prevent_default: true, description: "Go to first child session" },
    Definition { name: "session_child_cycle", default: "right", prevent_default: true, description: "Go to next child session" },
    Definition { name: "session_child_cycle_reverse", default: "left", prevent_default: true, description: "Go to previous child session" },
    Definition { name: "session_parent", default: "up", prevent_default: true, description: "Go to parent session" },
    Definition { name: "session_pin_toggle", default: "ctrl+f", prevent_default: true, description: "Pin or unpin session in the session list" },
    Definition { name: "session_quick_switch_1", default: "<leader>1", prevent_default: true, description: "Switch to session in quick slot 1" },
    Definition { name: "session_quick_switch_2", default: "<leader>2", prevent_default: true, description: "Switch to session in quick slot 2" },
    Definition { name: "session_quick_switch_3", default: "<leader>3", prevent_default: true, description: "Switch to session in quick slot 3" },
    Definition { name: "session_quick_switch_4", default: "<leader>4", prevent_default: true, description: "Switch to session in quick slot 4" },
    Definition { name: "session_quick_switch_5", default: "<leader>5", prevent_default: true, description: "Switch to session in quick slot 5" },
    Definition { name: "session_quick_switch_6", default: "<leader>6", prevent_default: true, description: "Switch to session in quick slot 6" },
    Definition { name: "session_quick_switch_7", default: "<leader>7", prevent_default: true, description: "Switch to session in quick slot 7" },
    Definition { name: "session_quick_switch_8", default: "<leader>8", prevent_default: true, description: "Switch to session in quick slot 8" },
    Definition { name: "session_quick_switch_9", default: "<leader>9", prevent_default: true, description: "Switch to session in quick slot 9" },
    Definition { name: "stash_delete", default: "ctrl+d", prevent_default: true, description: "Delete stash entry" },
    Definition { name: "model_provider_list", default: "ctrl+a", prevent_default: true, description: "Open provider list from model dialog" },
    Definition { name: "model_favorite_toggle", default: "ctrl+f", prevent_default: true, description: "Toggle model favorite status" },
    Definition { name: "model_list", default: "<leader>m", prevent_default: true, description: "List available models" },
    Definition { name: "model_cycle_recent", default: "f2", prevent_default: true, description: "Next recently used model" },
    Definition { name: "model_cycle_recent_reverse", default: "shift+f2", prevent_default: true, description: "Previous recently used model" },
    Definition { name: "model_cycle_favorite", default: "none", prevent_default: true, description: "Next favorite model" },
    Definition { name: "model_cycle_favorite_reverse", default: "none", prevent_default: true, description: "Previous favorite model" },
    Definition { name: "mcp_list", default: "none", prevent_default: true, description: "List MCP servers" },
    Definition { name: "provider_connect", default: "none", prevent_default: true, description: "Connect provider" },
    Definition { name: "console_org_switch", default: "none", prevent_default: true, description: "Switch console organization" },
    Definition { name: "agent_list", default: "<leader>a", prevent_default: true, description: "List agents" },
    Definition { name: "agent_cycle", default: "tab", prevent_default: true, description: "Next agent" },
    Definition { name: "agent_cycle_reverse", default: "shift+tab", prevent_default: true, description: "Previous agent" },
    Definition { name: "variant_cycle", default: "ctrl+t", prevent_default: true, description: "Cycle model variants" },
    Definition { name: "variant_list", default: "none", prevent_default: true, description: "List model variants" },
    Definition { name: "messages_page_up", default: "pageup,ctrl+alt+b", prevent_default: true, description: "Scroll messages up by one page" },
    Definition { name: "messages_page_down", default: "pagedown,ctrl+alt+f", prevent_default: true, description: "Scroll messages down by one page" },
    Definition { name: "messages_line_up", default: "ctrl+alt+y", prevent_default: true, description: "Scroll messages up by one line" },
    Definition { name: "messages_line_down", default: "ctrl+alt+e", prevent_default: true, description: "Scroll messages down by one line" },
    Definition { name: "messages_half_page_up", default: "ctrl+alt+u", prevent_default: true, description: "Scroll messages up by half page" },
    Definition { name: "messages_half_page_down", default: "ctrl+alt+d", prevent_default: true, description: "Scroll messages down by half page" },
    Definition { name: "messages_first", default: "ctrl+g,home", prevent_default: true, description: "Navigate to first message" },
    Definition { name: "messages_last", default: "ctrl+alt+g,end", prevent_default: true, description: "Navigate to last message" },
    Definition { name: "messages_next", default: "none", prevent_default: true, description: "Navigate to next message" },
    Definition { name: "messages_previous", default: "none", prevent_default: true, description: "Navigate to previous message" },
    Definition { name: "messages_last_user", default: "none", prevent_default: true, description: "Navigate to last user message" },
    Definition { name: "messages_copy", default: "<leader>y", prevent_default: true, description: "Copy message" },
    Definition { name: "messages_undo", default: "<leader>u", prevent_default: true, description: "Undo message" },
    Definition { name: "messages_redo", default: "<leader>r", prevent_default: true, description: "Redo message" },
    Definition { name: "messages_toggle_conceal", default: "<leader>h", prevent_default: true, description: "Toggle code block concealment in messages" },
    Definition { name: "tool_details", default: "none", prevent_default: true, description: "Toggle tool details visibility" },
    Definition { name: "display_thinking", default: "none", prevent_default: true, description: "Toggle thinking blocks visibility" },
    Definition { name: "prompt_submit", default: "none", prevent_default: true, description: "Submit prompt" },
    Definition { name: "prompt_editor_context_clear", default: "none", prevent_default: true, description: "Clear editor context" },
    Definition { name: "prompt_skills", default: "none", prevent_default: true, description: "Open skill selector" },
    Definition { name: "prompt_stash", default: "none", prevent_default: true, description: "Stash prompt" },
    Definition { name: "prompt_stash_pop", default: "none", prevent_default: true, description: "Pop stashed prompt" },
    Definition { name: "prompt_stash_list", default: "none", prevent_default: true, description: "List stashed prompts" },
    Definition { name: "workspace_set", default: "none", prevent_default: true, description: "Set workspace" },
    Definition { name: "input_clear", default: "ctrl+c", prevent_default: true, description: "Clear input field" },
    Definition { name: "input_paste", default: "ctrl+v", prevent_default: false, description: "Paste from clipboard" },
    Definition { name: "input_submit", default: "return", prevent_default: true, description: "Submit input" },
    Definition { name: "input_newline", default: "shift+return,ctrl+return,alt+return,ctrl+j", prevent_default: true, description: "Insert newline in input" },
    Definition { name: "input_move_left", default: "left,ctrl+b", prevent_default: true, description: "Move cursor left in input" },
    Definition { name: "input_move_right", default: "right,ctrl+f", prevent_default: true, description: "Move cursor right in input" },
    Definition { name: "input_move_up", default: "up", prevent_default: true, description: "Move cursor up in input" },
    Definition { name: "input_move_down", default: "down", prevent_default: true, description: "Move cursor down in input" },
    Definition { name: "input_select_left", default: "shift+left", prevent_default: true, description: "Select left in input" },
    Definition { name: "input_select_right", default: "shift+right", prevent_default: true, description: "Select right in input" },
    Definition { name: "input_select_up", default: "shift+up", prevent_default: true, description: "Select up in input" },
    Definition { name: "input_select_down", default: "shift+down", prevent_default: true, description: "Select down in input" },
    Definition { name: "input_line_home", default: "ctrl+a", prevent_default: true, description: "Move to start of line in input" },
    Definition { name: "input_line_end", default: "ctrl+e", prevent_default: true, description: "Move to end of line in input" },
    Definition { name: "input_select_line_home", default: "ctrl+shift+a", prevent_default: true, description: "Select to start of line in input" },
    Definition { name: "input_select_line_end", default: "ctrl+shift+e", prevent_default: true, description: "Select to end of line in input" },
    Definition { name: "input_visual_line_home", default: "alt+a", prevent_default: true, description: "Move to start of visual line in input" },
    Definition { name: "input_visual_line_end", default: "alt+e", prevent_default: true, description: "Move to end of visual line in input" },
    Definition { name: "input_select_visual_line_home", default: "alt+shift+a", prevent_default: true, description: "Select to start of visual line in input" },
    Definition { name: "input_select_visual_line_end", default: "alt+shift+e", prevent_default: true, description: "Select to end of visual line in input" },
    Definition { name: "input_buffer_home", default: "home", prevent_default: true, description: "Move to start of buffer in input" },
    Definition { name: "input_buffer_end", default: "end", prevent_default: true, description: "Move to end of buffer in input" },
    Definition { name: "input_select_buffer_home", default: "shift+home", prevent_default: true, description: "Select to start of buffer in input" },
    Definition { name: "input_select_buffer_end", default: "shift+end", prevent_default: true, description: "Select to end of buffer in input" },
    Definition { name: "input_delete_line", default: "ctrl+shift+d", prevent_default: true, description: "Delete line in input" },
    Definition { name: "input_delete_to_line_end", default: "ctrl+k", prevent_default: true, description: "Delete to end of line in input" },
    Definition { name: "input_delete_to_line_start", default: "ctrl+u", prevent_default: true, description: "Delete to start of line in input" },
    Definition { name: "input_backspace", default: "backspace,shift+backspace", prevent_default: true, description: "Backspace in input" },
    Definition { name: "input_delete", default: "ctrl+d,delete,shift+delete", prevent_default: true, description: "Delete character in input" },
    Definition { name: "input_undo", default: "ctrl+-,super+z", prevent_default: true, description: "Undo in input" },
    Definition { name: "input_redo", default: "ctrl+.,super+shift+z", prevent_default: true, description: "Redo in input" },
    Definition { name: "input_word_forward", default: "alt+f,alt+right,ctrl+right", prevent_default: true, description: "Move word forward in input" },
    Definition { name: "input_word_backward", default: "alt+b,alt+left,ctrl+left", prevent_default: true, description: "Move word backward in input" },
    Definition { name: "input_select_word_forward", default: "alt+shift+f,alt+shift+right", prevent_default: true, description: "Select word forward in input" },
    Definition { name: "input_select_word_backward", default: "alt+shift+b,alt+shift+left", prevent_default: true, description: "Select word backward in input" },
    Definition { name: "input_delete_word_forward", default: "alt+d,alt+delete,ctrl+delete", prevent_default: true, description: "Delete word forward in input" },
    Definition { name: "input_delete_word_backward", default: "ctrl+w,ctrl+backspace,alt+backspace", prevent_default: true, description: "Delete word backward in input" },
    Definition { name: "input_select_all", default: "super+a", prevent_default: true, description: "Select all in input" },
    Definition { name: "history_previous", default: "up", prevent_default: true, description: "Previous history item" },
    Definition { name: "history_next", default: "down", prevent_default: true, description: "Next history item" },
    Definition { name: "dialog.select.prev", default: "up,ctrl+p", prevent_default: true, description: "Move to previous dialog item" },
    Definition { name: "dialog.select.next", default: "down,ctrl+n", prevent_default: true, description: "Move to next dialog item" },
    Definition { name: "dialog.select.page_up", default: "pageup", prevent_default: true, description: "Move up one page in dialog" },
    Definition { name: "dialog.select.page_down", default: "pagedown", prevent_default: true, description: "Move down one page in dialog" },
    Definition { name: "dialog.select.home", default: "home", prevent_default: true, description: "Move to first dialog item" },
    Definition { name: "dialog.select.end", default: "end", prevent_default: true, description: "Move to last dialog item" },
    Definition { name: "dialog.select.submit", default: "return", prevent_default: true, description: "Submit selected dialog item" },
    Definition { name: "dialog.prompt.submit", default: "return", prevent_default: true, description: "Submit dialog prompt" },
    Definition { name: "dialog.mcp.toggle", default: "space", prevent_default: true, description: "Toggle MCP in MCP dialog" },
    Definition { name: "dialog.move_session.new", default: "ctrl+m", prevent_default: true, description: "New project copy" },
    Definition { name: "dialog.move_session.delete", default: "ctrl+d", prevent_default: true, description: "Delete project copy" },
    Definition { name: "dialog.move_session.refresh", default: "ctrl+r", prevent_default: true, description: "Refresh project copies" },
    Definition { name: "prompt.autocomplete.prev", default: "up,ctrl+p", prevent_default: true, description: "Move to previous autocomplete item" },
    Definition { name: "prompt.autocomplete.next", default: "down,ctrl+n", prevent_default: true, description: "Move to next autocomplete item" },
    Definition { name: "prompt.autocomplete.hide", default: "escape", prevent_default: true, description: "Hide autocomplete" },
    Definition { name: "prompt.autocomplete.select", default: "return", prevent_default: true, description: "Select autocomplete item" },
    Definition { name: "prompt.autocomplete.complete", default: "tab", prevent_default: true, description: "Complete autocomplete item" },
    Definition { name: "permission.prompt.fullscreen", default: "ctrl+f", prevent_default: true, description: "Toggle permission prompt fullscreen" },
    Definition { name: "plugins.toggle", default: "space", prevent_default: true, description: "Toggle plugin" },
    Definition { name: "dialog.plugins.install", default: "shift+i", prevent_default: true, description: "Install plugin from plugin dialog" },
    Definition { name: "terminal_suspend", default: "ctrl+z", prevent_default: true, description: "Suspend terminal" },
    Definition { name: "terminal_title_toggle", default: "none", prevent_default: true, description: "Toggle terminal title" },
    Definition { name: "tips_toggle", default: "<leader>h", prevent_default: true, description: "Toggle tips on home screen" },
    Definition { name: "plugin_manager", default: "none", prevent_default: true, description: "Open plugin manager dialog" },
    Definition { name: "plugin_install", default: "none", prevent_default: true, description: "Install plugin" },
    Definition { name: "which_key_toggle", default: "ctrl+alt+k", prevent_default: true, description: "Toggle which-key panel" },
    Definition { name: "which_key_layout_toggle", default: "ctrl+alt+shift+k", prevent_default: true, description: "Switch which-key layout" },
    Definition { name: "which_key_pending_toggle", default: "ctrl+alt+shift+p", prevent_default: true, description: "Toggle which-key pending preview" },
    Definition { name: "which_key_group_previous", default: "ctrl+alt+left,ctrl+alt+[", prevent_default: true, description: "Previous which-key group" },
    Definition { name: "which_key_group_next", default: "ctrl+alt+right,ctrl+alt+]", prevent_default: true, description: "Next which-key group" },
    Definition { name: "which_key_scroll_up", default: "ctrl+alt+up,ctrl+alt+p", prevent_default: true, description: "Scroll which-key up" },
    Definition { name: "which_key_scroll_down", default: "ctrl+alt+down,ctrl+alt+n", prevent_default: true, description: "Scroll which-key down" },
    Definition { name: "which_key_page_up", default: "ctrl+alt+pageup", prevent_default: true, description: "Page which-key up" },
    Definition { name: "which_key_page_down", default: "ctrl+alt+pagedown", prevent_default: true, description: "Page which-key down" },
    Definition { name: "which_key_home", default: "ctrl+alt+home", prevent_default: true, description: "Jump to first which-key binding" },
    Definition { name: "which_key_end", default: "ctrl+alt+end", prevent_default: true, description: "Jump to last which-key binding" },
];

/// Command map: 163 name -> command entries (keybind.ts:256-420).
pub static COMMAND_MAP: &[(&str, &str)] = &[
    ("app_exit", "app.exit"),
    ("app_debug", "app.debug"),
    ("app_console", "app.console"),
    ("app_heap_snapshot", "app.heap_snapshot"),
    ("app_toggle_animations", "app.toggle.animations"),
    ("app_toggle_file_context", "app.toggle.file_context"),
    ("app_toggle_diffwrap", "app.toggle.diffwrap"),
    ("app_toggle_paste_summary", "app.toggle.paste_summary"),
    ("app_toggle_session_directory_filter", "app.toggle.session_directory_filter"),
    ("command_list", "command.palette.show"),
    ("help_show", "help.show"),
    ("docs_open", "docs.open"),
    ("diff_open", "diff.open"),
    ("diff_close", "diff.close"),
    ("diff_toggle", "diff.toggle"),
    ("diff_expand", "diff.expand"),
    ("diff_expand_all", "diff.expand_all"),
    ("diff_collapse", "diff.collapse"),
    ("diff_switch_focus", "diff.switch_focus"),
    ("diff_next_hunk", "diff.next_hunk"),
    ("diff_previous_hunk", "diff.previous_hunk"),
    ("diff_next_file", "diff.next_file"),
    ("diff_previous_file", "diff.previous_file"),
    ("diff_toggle_file_tree", "diff.toggle_file_tree"),
    ("diff_single_patch", "diff.single_patch"),
    ("diff_switch_source", "diff.switch_source"),
    ("diff_toggle_view", "diff.toggle_view"),
    ("diff_help", "diff.help"),
    ("editor_open", "prompt.editor"),
    ("theme_list", "theme.switch"),
    ("theme_switch_mode", "theme.switch_mode"),
    ("theme_mode_lock", "theme.mode.lock"),
    ("sidebar_toggle", "session.sidebar.toggle"),
    ("scrollbar_toggle", "session.toggle.scrollbar"),
    ("status_view", "opencode.status"),
    ("debug_view", "opencode.debug"),
    ("session_export", "session.export"),
    ("session_copy", "session.copy"),
    ("session_move", "session.move"),
    ("session_new", "session.new"),
    ("session_list", "session.list"),
    ("session_timeline", "session.timeline"),
    ("session_fork", "session.fork"),
    ("session_rename", "session.rename"),
    ("session_delete", "session.delete"),
    ("session_share", "session.share"),
    ("session_unshare", "session.unshare"),
    ("session_interrupt", "session.interrupt"),
    ("session_background", "session.background"),
    ("session_compact", "session.compact"),
    ("session_toggle_timestamps", "session.toggle.timestamps"),
    ("session_toggle_generic_tool_output", "session.toggle.generic_tool_output"),
    ("session_queued_prompts", "session.queued_prompts"),
    ("session_child_first", "session.child.first"),
    ("session_child_cycle", "session.child.next"),
    ("session_child_cycle_reverse", "session.child.previous"),
    ("session_parent", "session.parent"),
    ("session_pin_toggle", "session.pin.toggle"),
    ("session_quick_switch_1", "session.quick_switch.1"),
    ("session_quick_switch_2", "session.quick_switch.2"),
    ("session_quick_switch_3", "session.quick_switch.3"),
    ("session_quick_switch_4", "session.quick_switch.4"),
    ("session_quick_switch_5", "session.quick_switch.5"),
    ("session_quick_switch_6", "session.quick_switch.6"),
    ("session_quick_switch_7", "session.quick_switch.7"),
    ("session_quick_switch_8", "session.quick_switch.8"),
    ("session_quick_switch_9", "session.quick_switch.9"),
    ("stash_delete", "stash.delete"),
    ("model_provider_list", "model.dialog.provider"),
    ("model_favorite_toggle", "model.dialog.favorite"),
    ("model_list", "model.list"),
    ("model_cycle_recent", "model.cycle_recent"),
    ("model_cycle_recent_reverse", "model.cycle_recent_reverse"),
    ("model_cycle_favorite", "model.cycle_favorite"),
    ("model_cycle_favorite_reverse", "model.cycle_favorite_reverse"),
    ("mcp_list", "mcp.list"),
    ("provider_connect", "provider.connect"),
    ("console_org_switch", "console.org.switch"),
    ("agent_list", "agent.list"),
    ("agent_cycle", "agent.cycle"),
    ("agent_cycle_reverse", "agent.cycle.reverse"),
    ("variant_cycle", "variant.cycle"),
    ("variant_list", "variant.list"),
    ("messages_page_up", "session.page.up"),
    ("messages_page_down", "session.page.down"),
    ("messages_line_up", "session.line.up"),
    ("messages_line_down", "session.line.down"),
    ("messages_half_page_up", "session.half.page.up"),
    ("messages_half_page_down", "session.half.page.down"),
    ("messages_first", "session.first"),
    ("messages_last", "session.last"),
    ("messages_next", "session.message.next"),
    ("messages_previous", "session.message.previous"),
    ("messages_last_user", "session.messages_last_user"),
    ("messages_copy", "messages.copy"),
    ("messages_undo", "session.undo"),
    ("messages_redo", "session.redo"),
    ("messages_toggle_conceal", "session.toggle.conceal"),
    ("tool_details", "session.toggle.actions"),
    ("display_thinking", "session.toggle.thinking"),
    ("prompt_submit", "prompt.submit"),
    ("prompt_editor_context_clear", "prompt.editor_context.clear"),
    ("prompt_skills", "prompt.skills"),
    ("prompt_stash", "prompt.stash"),
    ("prompt_stash_pop", "prompt.stash.pop"),
    ("prompt_stash_list", "prompt.stash.list"),
    ("workspace_set", "workspace.set"),
    ("input_clear", "prompt.clear"),
    ("input_paste", "prompt.paste"),
    ("input_submit", "input.submit"),
    ("input_newline", "input.newline"),
    ("input_move_left", "input.move.left"),
    ("input_move_right", "input.move.right"),
    ("input_move_up", "input.move.up"),
    ("input_move_down", "input.move.down"),
    ("input_select_left", "input.select.left"),
    ("input_select_right", "input.select.right"),
    ("input_select_up", "input.select.up"),
    ("input_select_down", "input.select.down"),
    ("input_line_home", "input.line.home"),
    ("input_line_end", "input.line.end"),
    ("input_select_line_home", "input.select.line.home"),
    ("input_select_line_end", "input.select.line.end"),
    ("input_visual_line_home", "input.visual.line.home"),
    ("input_visual_line_end", "input.visual.line.end"),
    ("input_select_visual_line_home", "input.select.visual.line.home"),
    ("input_select_visual_line_end", "input.select.visual.line.end"),
    ("input_buffer_home", "input.buffer.home"),
    ("input_buffer_end", "input.buffer.end"),
    ("input_select_buffer_home", "input.select.buffer.home"),
    ("input_select_buffer_end", "input.select.buffer.end"),
    ("input_delete_line", "input.delete.line"),
    ("input_delete_to_line_end", "input.delete.to.line.end"),
    ("input_delete_to_line_start", "input.delete.to.line.start"),
    ("input_backspace", "input.backspace"),
    ("input_delete", "input.delete"),
    ("input_undo", "input.undo"),
    ("input_redo", "input.redo"),
    ("input_word_forward", "input.word.forward"),
    ("input_word_backward", "input.word.backward"),
    ("input_select_word_forward", "input.select.word.forward"),
    ("input_select_word_backward", "input.select.word.backward"),
    ("input_delete_word_forward", "input.delete.word.forward"),
    ("input_delete_word_backward", "input.delete.word.backward"),
    ("input_select_all", "input.select.all"),
    ("history_previous", "prompt.history.previous"),
    ("history_next", "prompt.history.next"),
    ("terminal_suspend", "terminal.suspend"),
    ("terminal_title_toggle", "terminal.title.toggle"),
    ("tips_toggle", "tips.toggle"),
    ("plugin_manager", "plugins.list"),
    ("plugin_install", "plugins.install"),
    ("which_key_toggle", "which-key.toggle"),
    ("which_key_layout_toggle", "which-key.layout.toggle"),
    ("which_key_pending_toggle", "which-key.pending.toggle"),
    ("which_key_group_previous", "which-key.group.previous"),
    ("which_key_group_next", "which-key.group.next"),
    ("which_key_scroll_up", "which-key.scroll.up"),
    ("which_key_scroll_down", "which-key.scroll.down"),
    ("which_key_page_up", "which-key.page.up"),
    ("which_key_page_down", "which-key.page.down"),
    ("which_key_home", "which-key.home"),
    ("which_key_end", "which-key.end"),
];

/// Description for a keybind name (`Descriptions`, keybind.ts:253-255).
#[must_use]
pub fn description(name: &str) -> Option<&'static str> {
    DEFINITIONS.iter().find(|d| d.name == name).map(|d| d.description)
}

/// Command for a keybind name (`CommandMap`, keybind.ts:256-420).
#[must_use]
pub fn command(name: &str) -> Option<&'static str> {
    COMMAND_MAP.iter().find(|(n, _)| *n == name).map(|(_, c)| *c)
}

/// Description for a command string (`CommandDescriptions`,
/// keybind.ts:421-426): command -> owning definition's description.
#[must_use]
pub fn command_description(cmd: &str) -> Option<&'static str> {
    let name = COMMAND_MAP.iter().find(|(_, c)| *c == cmd)?.0;
    description(name)
}

/// Default [`BindingValue`] for a keybind name (`defaultValue`,
/// keybind.ts:445-447).
pub fn default_value(name: &str) -> Option<Result<BindingValue, TableError>> {
    DEFINITIONS.iter().find(|d| d.name == name).map(|d| BindingValue::parse(d.default).map_err(TableError::from))
}

/// Override names not present in [`DEFINITIONS`] (`unknownKeys`,
/// keybind.ts:462-464).
#[must_use]
pub fn unknown_keys<'a>(overrides: &[(&'a str, &str)]) -> Vec<&'a str> {
    overrides
        .iter()
        .filter(|(name, _)| !DEFINITIONS.iter().any(|d| d.name == *name))
        .map(|(name, _)| *name)
        .collect()
}

/// All defaults as a [`KeymapConfig`].
pub fn defaults_config() -> Result<KeymapConfig, TableError> {
    let mut cfg = KeymapConfig::new();
    for d in DEFINITIONS {
        cfg.insert(d.name, BindingValue::parse(d.default)?)?;
    }
    Ok(cfg)
}

/// Defaults + overrides with unknown-key reject (`parse`, keybind.ts:449-458).
/// Override values use the str form (`false` disables, `"none"` unbinds).
pub fn parse_with_overrides(overrides: &[(&str, &str)]) -> Result<KeymapConfig, TableError> {
    let unknown = unknown_keys(overrides);
    if let Some(first) = unknown.first() {
        return Err(TableError::UnknownKey((*first).to_string()));
    }
    let mut cfg = defaults_config()?;
    for (name, value) in overrides {
        cfg.insert(name, BindingValue::parse(value)?)?;
    }
    Ok(cfg)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::key_event::KeyPress;

    #[test]
    fn parse_object_form_defaults() {
        let o = parse_binding_object("ctrl+v", None, None, None).unwrap();
        assert_eq!(o.key, "ctrl+v");
        assert_eq!(o.event, KeyEvent2::Press);
        assert!(o.prevent_default);
        assert!(!o.fallthrough);
        let ev = object_key_event(&o).unwrap();
        assert_eq!(ev.name, "v");
        assert!(ev.ctrl);
        assert_eq!(ev.event, KeyPress::Press);
        assert!(matches!(binding_value_of_object(&o).unwrap(), BindingValue::Strokes(_)));
    }

    #[test]
    fn parse_object_form_explicit() {
        // Mirrors input_paste default (keybind.ts:162).
        let o = parse_binding_object("ctrl+v", Some("release"), Some(false), Some(true)).unwrap();
        assert_eq!(o.event, KeyEvent2::Release);
        assert!(!o.prevent_default);
        assert!(o.fallthrough);
        let ev = object_key_event(&o).unwrap();
        assert_eq!(ev.event, KeyPress::Release);
        assert_eq!(parse_binding_object("a", Some("RELEASE"), None, None).unwrap().event, KeyEvent2::Release);
        assert_eq!(parse_binding_object("x", Some("bogus"), None, None), Err(TableError::UnknownEvent));
        assert_eq!(parse_binding_object("", None, None, None), Err(TableError::EmptyKey));
        assert_eq!(
            parse_binding_object(&"x".repeat(MAX_KEY_LEN + 1), None, None, None),
            Err(TableError::KeyTooLong)
        );
    }

    #[test]
    fn array_form_merges() {
        let v = parse_binding_array(&[
            BindingArrayItem::Str("ctrl+c,ctrl+d".to_string()),
            BindingArrayItem::Object(parse_binding_object("q", None, None, None).unwrap()),
        ])
        .unwrap();
        match v {
            BindingValue::Strokes(s) => {
                assert_eq!(s.len(), 3);
                assert_eq!(s[2].name, "q");
            }
            _ => panic!("expected strokes"),
        }
        assert_eq!(parse_binding_array(&[]), Err(TableError::InvalidItem));
        assert_eq!(
            parse_binding_array(&[BindingArrayItem::Str("none".to_string())]),
            Err(TableError::InvalidItem)
        );
        let many = vec![BindingArrayItem::Str("a".to_string()); MAX_TABLE_STROKES + 1];
        assert_eq!(parse_binding_array(&many), Err(TableError::TooManyStrokes));
    }

    #[test]
    fn unknown_override_rejects() {
        assert!(unknown_keys(&[("input_clear", "ctrl+c")]).is_empty());
        assert_eq!(unknown_keys(&[("nope", "a")]), vec!["nope"]);
        assert!(matches!(
            parse_with_overrides(&[("bogus_key", "a")]),
            Err(TableError::UnknownKey(_))
        ));
        let cfg = parse_with_overrides(&[("input_clear", "ctrl+k")]).unwrap();
        match cfg.get("input_clear").unwrap() {
            BindingValue::Strokes(s) => assert_eq!(s[0].name, "k"),
            _ => panic!("expected strokes"),
        }
        assert_eq!(cfg.len(), DEFINITIONS.len());
    }

    #[test]
    fn lookup_hit_miss() {
        assert_eq!(description("app_exit"), Some("Exit the application"));
        assert_eq!(command("app_exit"), Some("app.exit"));
        assert_eq!(command_description("app.exit"), Some("Exit the application"));
        assert_eq!(description("leader"), Some("Leader key for keybind combinations"));
        assert_eq!(command("leader"), None);
        assert_eq!(description("nope"), None);
        assert_eq!(command("nope"), None);
        assert_eq!(command_description("nope"), None);
    }

    #[test]
    fn counts_match_ts() {
        // keybind.ts:45-240 holds 184 keybind() calls; :256-420 holds 163 map entries.
        assert_eq!(DEFINITIONS.len(), 184);
        assert_eq!(COMMAND_MAP.len(), 163);
    }

    #[test]
    fn all_defaults_parse() {
        for d in DEFINITIONS {
            assert!(BindingValue::parse(d.default).is_ok(), "bad default: {}", d.name);
            assert!(default_value(d.name).unwrap().is_ok());
        }
        let cfg = defaults_config().unwrap();
        assert_eq!(cfg.len(), DEFINITIONS.len());
        assert!(parse_with_overrides(&[]).is_ok());
    }
}
