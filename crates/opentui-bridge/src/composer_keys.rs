#![forbid(unsafe_code)]
//! Pure classification of resolved prompt key actions.

/// A semantic action exposed by the prompt composer.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ComposerKey {
    Submit,
    Newline,
    Interrupt,
    ShellToggle,
    Quit,
}

/// Classify a resolved keymap action, applying the empty-buffer quit guard.
#[must_use]
pub fn classify(resolve_outcome_action: &str, buf_empty: bool) -> Option<ComposerKey> {
    match resolve_outcome_action {
        "submit" => Some(ComposerKey::Submit),
        "newline" => Some(ComposerKey::Newline),
        "interrupt" => Some(ComposerKey::Interrupt),
        "shell" => Some(ComposerKey::ShellToggle),
        "quit" | "q" if buf_empty => Some(ComposerKey::Quit),
        _ => None,
    }
}

#[cfg(test)]
mod tests {
    use super::{classify, ComposerKey};

    #[test]
    fn submit_maps_to_submit() {
        assert_eq!(classify("submit", false), Some(ComposerKey::Submit));
    }

    #[test]
    fn newline_maps_to_newline() {
        assert_eq!(classify("newline", false), Some(ComposerKey::Newline));
    }

    #[test]
    fn interrupt_maps_to_interrupt() {
        assert_eq!(classify("interrupt", false), Some(ComposerKey::Interrupt));
    }

    #[test]
    fn shell_maps_to_toggle() {
        assert_eq!(classify("shell", false), Some(ComposerKey::ShellToggle));
    }

    #[test]
    fn quit_maps_only_for_empty_buffer() {
        assert_eq!(classify("quit", true), Some(ComposerKey::Quit));
        assert_eq!(classify("quit", false), None);
    }

    #[test]
    fn q_maps_only_for_empty_buffer() {
        assert_eq!(classify("q", true), Some(ComposerKey::Quit));
        assert_eq!(classify("q", false), None);
    }

    #[test]
    fn unknown_action_is_none() {
        assert_eq!(classify("prompt.other", true), None);
    }
}
