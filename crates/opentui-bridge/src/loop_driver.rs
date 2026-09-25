#![forbid(unsafe_code)]
//! Testable CLI loop driver: [`LoopState`] + [`step`] over [`InputChunk`].
//!
//! Mirrors `tui_entry.rs:626-672` match arms (read-only ref; that file is
//! absent from this tree): Quit sets quit, Page routes overlay, text/paste
//! append to a capped draft, Backspace pops, empty-submit flushes the draft
//! to the capped transcript, Resize/Noop are noops.

use crate::native_input::{InputChunk, Page};

/// Draft byte cap (4 KiB; mirrors `native_input::MAX_TEXT`).
pub const MAX_DRAFT: usize = 4096;
/// Transcript line cap.
pub const MAX_TRANSCRIPT: usize = 500;

/// Visible overlay page.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum LoopPage {
    #[default]
    Chat,
    Palette,
    Context,
    Help,
}

impl From<Page> for LoopPage {
    fn from(p: Page) -> Self {
        match p {
            Page::Chat => Self::Chat,
            Page::Palette => Self::Palette,
            Page::Context => Self::Context,
            Page::Help => Self::Help,
        }
    }
}

/// CLI-callable loop state.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct LoopState {
    pub page: LoopPage,
    pub draft: String,
    pub transcript: Vec<String>,
    pub quit: bool,
}

/// One driver step = one decoded [`InputChunk`] passthrough.
pub type LoopStep = InputChunk;

fn push_capped(buf: &mut String, text: &str) {
    let room = MAX_DRAFT.saturating_sub(buf.len());
    if room == 0 {
        return;
    }
    let mut end = text.len().min(room);
    while end > 0 && !text.is_char_boundary(end) {
        end -= 1;
    }
    buf.push_str(&text[..end]);
}

fn submit(state: &mut LoopState) {
    if state.draft.is_empty() {
        return;
    }
    let line = core::mem::take(&mut state.draft);
    state.transcript.push(line);
    if state.transcript.len() > MAX_TRANSCRIPT {
        let overflow = state.transcript.len() - MAX_TRANSCRIPT;
        state.transcript.drain(..overflow);
    }
}

/// Apply one chunk to `state` in place.
pub fn step(state: &mut LoopState, chunk: LoopStep) {
    match chunk {
        InputChunk::Quit => state.quit = true,
        InputChunk::Page(p) => state.page = LoopPage::from(p),
        InputChunk::SubmitText(t) if t.is_empty() => submit(state),
        InputChunk::SubmitText(t) => push_capped(&mut state.draft, &t),
        InputChunk::EditBackspace => {
            state.draft.pop();
        }
        InputChunk::Paste(t) => push_capped(&mut state.draft, &t),
        InputChunk::Resize | InputChunk::Noop => {}
    }
}

/// Drain and return the transcript, leaving it empty.
#[must_use]
pub fn take_transcript(state: &mut LoopState) -> Vec<String> {
    core::mem::take(&mut state.transcript)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn state() -> LoopState {
        LoopState::default()
    }

    #[test]
    fn quit_flag_set() {
        let mut s = state();
        step(&mut s, InputChunk::Quit);
        assert!(s.quit);
    }

    #[test]
    fn page_switch_palette_context_help_chat() {
        let mut s = state();
        step(&mut s, InputChunk::Page(Page::Palette));
        assert_eq!(s.page, LoopPage::Palette);
        step(&mut s, InputChunk::Page(Page::Context));
        assert_eq!(s.page, LoopPage::Context);
        step(&mut s, InputChunk::Page(Page::Help));
        assert_eq!(s.page, LoopPage::Help);
        step(&mut s, InputChunk::Page(Page::Chat));
        assert_eq!(s.page, LoopPage::Chat);
    }

    #[test]
    fn text_append_and_enter_submits() {
        let mut s = state();
        step(&mut s, InputChunk::SubmitText("hi".to_string()));
        assert_eq!(s.draft, "hi");
        step(&mut s, InputChunk::SubmitText(String::new()));
        assert_eq!(s.draft, "");
        assert_eq!(s.transcript, vec!["hi".to_string()]);
    }

    #[test]
    fn backspace_pops_and_empty_noop() {
        let mut s = state();
        step(&mut s, InputChunk::SubmitText("hé".to_string()));
        step(&mut s, InputChunk::EditBackspace);
        assert_eq!(s.draft, "h");
        step(&mut s, InputChunk::EditBackspace);
        step(&mut s, InputChunk::EditBackspace);
        assert_eq!(s.draft, "");
    }

    #[test]
    fn paste_capped_at_4kib() {
        let mut s = state();
        step(&mut s, InputChunk::Paste("x".repeat(MAX_DRAFT + 100)));
        assert_eq!(s.draft.len(), MAX_DRAFT);
        step(&mut s, InputChunk::Paste("y".to_string()));
        assert_eq!(s.draft.len(), MAX_DRAFT);
    }

    #[test]
    fn transcript_capped_at_500() {
        let mut s = state();
        for i in 0..(MAX_TRANSCRIPT + 10) {
            step(&mut s, InputChunk::SubmitText(format!("l{i}")));
            step(&mut s, InputChunk::SubmitText(String::new()));
        }
        assert_eq!(s.transcript.len(), MAX_TRANSCRIPT);
        assert_eq!(s.transcript[0], "l10");
    }

    #[test]
    fn draft_capped_char_boundary() {
        let mut s = state();
        step(&mut s, InputChunk::SubmitText("é".repeat(MAX_DRAFT)));
        assert!(s.draft.len() <= MAX_DRAFT);
        assert!(s.draft.is_char_boundary(s.draft.len()));
    }

    #[test]
    fn resize_noop_and_take_transcript() {
        let mut s = state();
        step(&mut s, InputChunk::SubmitText("a".to_string()));
        step(&mut s, InputChunk::Resize);
        step(&mut s, InputChunk::Noop);
        assert_eq!(s.draft, "a");
        assert!(!s.quit);
        step(&mut s, InputChunk::SubmitText(String::new()));
        assert_eq!(take_transcript(&mut s), vec!["a".to_string()]);
        assert!(s.transcript.is_empty());
    }
}
