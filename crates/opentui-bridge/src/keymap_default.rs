#![forbid(unsafe_code)]
//! Default key bindings (BRIDGE-PAR-179).
//!
//! TS truth: `crate::keymap_dispatch` mirrors
//! `packages/tui/src/keymap.tsx:22` (`COMMAND_PALETTE_COMMAND`).

use crate::keymap_dispatch::{KeyAction, KeymapDispatch};

/// Prebound default keymap.
#[must_use]
pub fn default_keymap() -> KeymapDispatch {
    let mut m = KeymapDispatch::new();
    let binds = [
        ("ctrl-p", "Command palette", "command.palette.show"),
        ("ctrl-t", "Model context", "context.show"),
        ("?", "Help", "help.show"),
        ("esc", "Back to chat", "chat.focus"),
        ("ctrl-c", "Quit", "app.quit"),
        ("enter", "Submit", "input.submit"),
        ("backspace", "Delete char", "input.backspace"),
    ];
    for (k, label, cmd) in binds {
        m.bind(k, KeyAction::new(label, cmd));
    }
    m
}

/// Sorted `"key -> command"` lines, capped at 64.
#[must_use]
pub fn describe(map: &KeymapDispatch) -> Vec<String> {
    let mut out: Vec<String> = map
        .bindings
        .iter()
        .map(|(k, a)| format!("{k} -> {}", a.command))
        .collect();
    out.sort();
    out.truncate(64);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn palette_bound() {
        let m = default_keymap();
        assert_eq!(
            m.dispatch("ctrl-p").unwrap().command,
            "command.palette.show"
        );
    }

    #[test]
    fn all_prebinds_present() {
        let m = default_keymap();
        for k in [
            "ctrl-p",
            "ctrl-t",
            "?",
            "esc",
            "ctrl-c",
            "enter",
            "backspace",
        ] {
            assert!(m.dispatch(k).is_some(), "missing {k}");
        }
        assert_eq!(m.bindings.len(), 7);
    }

    #[test]
    fn labels_human() {
        let m = default_keymap();
        for (_, a) in &m.bindings {
            assert!(!a.label.is_empty() && a.label != a.command, "{}", a.command);
        }
    }

    #[test]
    fn describe_sorted_and_capped() {
        let m = default_keymap();
        let d = describe(&m);
        assert_eq!(d.len(), 7);
        let mut s = d.clone();
        s.sort();
        assert_eq!(d, s);
        assert!(d.iter().all(|l| l.contains(" -> ")));
        let big = KeymapDispatch::new();
        let mut full = KeymapDispatch::new();
        for i in 0..70 {
            full.bind(&format!("k{i:02}"), KeyAction::new("L", "c"));
        }
        let _ = big;
        assert_eq!(describe(&full).len(), 64);
    }

    #[test]
    fn unbound_falls_through() {
        assert!(default_keymap().dispatch("ctrl-z").is_none());
    }

    #[test]
    fn quit_and_submit() {
        let m = default_keymap();
        assert_eq!(m.dispatch("ctrl-c").unwrap().command, "app.quit");
        assert_eq!(m.dispatch("enter").unwrap().command, "input.submit");
        assert_eq!(m.dispatch("backspace").unwrap().command, "input.backspace");
    }
}
