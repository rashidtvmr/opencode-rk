#![forbid(unsafe_code)]
//! App-scope modal stack state.
//!
//! Pure state only: no IO, no threads. Caller renders
//! [`ModalStack::top`] and executes confirm/picker side effects;
//! [`ModalStack::render_top`] builds ASCII box lines (pure).

/// Max open modals retained.
pub const MAX_MODALS: usize = 8;
/// Max title chars retained per modal (truncated).
pub const MAX_TITLE_LEN: usize = 128;

/// Modal variant.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalKind {
    Help,
    Confirm,
    Error,
    Picker,
}

/// One modal entry.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Modal {
    pub kind: ModalKind,
    pub title: String,
    pub open: bool,
}

impl Modal {
    #[must_use]
    pub fn new(kind: ModalKind, title: impl Into<String>) -> Self {
        Self {
            kind,
            title: truncate_title(&title.into()),
            open: true,
        }
    }
}

/// Error from [`ModalStack::open`] when full.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum ModalError {
    StackFull,
}

impl core::fmt::Display for ModalError {
    fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
        match self {
            Self::StackFull => write!(f, "modal stack full ({MAX_MODALS})"),
        }
    }
}

impl std::error::Error for ModalError {}

/// LIFO stack of open modals, bounded by [`MAX_MODALS`].
#[derive(Debug, Default)]
pub struct ModalStack {
    stack: Vec<Modal>,
}

impl ModalStack {
    #[must_use]
    pub fn new() -> Self {
        Self { stack: Vec::new() }
    }

    /// Push a modal; title truncated to [`MAX_TITLE_LEN`]. Errors when full.
    pub fn open(
        &mut self,
        kind: ModalKind,
        title: impl Into<String>,
    ) -> Result<(), ModalError> {
        if self.stack.len() >= MAX_MODALS {
            return Err(ModalError::StackFull);
        }
        self.stack.push(Modal::new(kind, title));
        Ok(())
    }

    /// Pop the top modal.
    pub fn close_top(&mut self) -> Option<Modal> {
        self.stack.pop()
    }

    /// Peek the top modal.
    #[must_use]
    pub fn top(&self) -> Option<&Modal> {
        self.stack.last()
    }

    /// Draw the top modal as a bordered box (`+`/`-`/`|` lines).
    ///
    /// Pure: no IO. Returns `None` when the stack is empty or the top
    /// modal is closed. `width` is clamped to `[MIN_BOX_WIDTH,
    /// MAX_BOX_WIDTH]` so tiny/huge viewports stay sane; every line is
    /// exactly the clamped width in chars (char-safe truncation).
    #[must_use]
    pub fn render_top(&self, width: usize) -> Option<Vec<String>> {
        let modal = self.stack.last().filter(|m| m.open)?;
        let w = width.clamp(MIN_BOX_WIDTH, MAX_BOX_WIDTH);
        let inner = w - 2;
        let edge = format!("+{}+", "-".repeat(inner));
        Some(vec![
            edge.clone(),
            format!("|{}|", fit_cell(&modal.title, inner)),
            format!("|{}|", fit_cell(kind_label(modal.kind), inner)),
            edge,
        ])
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.stack.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.stack.is_empty()
    }
}

fn truncate_title(title: &str) -> String {
    if title.chars().count() <= MAX_TITLE_LEN {
        return title.to_owned();
    }
    title.chars().take(MAX_TITLE_LEN).collect()
}

/// Min box width (`+` + 1 char + `+` per side floor): `+--+` shape.
const MIN_BOX_WIDTH: usize = 4;
/// Max box width: keeps huge viewports bounded.
const MAX_BOX_WIDTH: usize = 80;

fn kind_label(kind: ModalKind) -> &'static str {
    match kind {
        ModalKind::Help => "help",
        ModalKind::Confirm => "confirm",
        ModalKind::Error => "error",
        ModalKind::Picker => "picker",
    }
}

/// Pad/truncate a cell to exactly `width` chars (char-safe).
fn fit_cell(text: &str, width: usize) -> String {
    let mut out: String = text.chars().take(width).collect();
    let missing = width.saturating_sub(out.chars().count());
    out.extend(core::iter::repeat(' ').take(missing));
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn stack_cap_errors() {
        let mut stack = ModalStack::new();
        for i in 0..MAX_MODALS {
            assert!(stack.open(ModalKind::Help, format!("m{i}")).is_ok());
        }
        assert_eq!(
            stack.open(ModalKind::Error, "overflow"),
            Err(ModalError::StackFull)
        );
        assert_eq!(stack.len(), MAX_MODALS);
    }

    #[test]
    fn close_empty_returns_none() {
        let mut stack = ModalStack::new();
        assert!(stack.top().is_none());
        assert!(stack.close_top().is_none());
    }

    #[test]
    fn lifo_order_and_title_cap() {
        let mut stack = ModalStack::new();
        stack.open(ModalKind::Help, "first").unwrap();
        stack.open(ModalKind::Confirm, "second").unwrap();
        stack.open(ModalKind::Picker, "x".repeat(MAX_TITLE_LEN + 10)).unwrap();
        assert_eq!(stack.top().unwrap().title.chars().count(), MAX_TITLE_LEN);
        assert_eq!(stack.close_top().unwrap().kind, ModalKind::Picker);
        assert_eq!(stack.top().unwrap().kind, ModalKind::Confirm);
        assert_eq!(stack.close_top().unwrap().title, "second");
        assert_eq!(stack.close_top().unwrap().title, "first");
        assert!(stack.is_empty());
    }

    #[test]
    fn render_top_none_when_empty_or_closed() {
        let stack = ModalStack::new();
        assert!(stack.render_top(20).is_none());
        let mut stack = ModalStack::new();
        stack.open(ModalKind::Help, "hi").unwrap();
        stack.stack.last_mut().unwrap().open = false;
        assert!(stack.render_top(20).is_none());
    }

    #[test]
    fn render_top_box_shape_and_kind() {
        let mut stack = ModalStack::new();
        stack.open(ModalKind::Error, "boom").unwrap();
        let lines = stack.render_top(12).unwrap();
        assert_eq!(lines.len(), 4);
        assert_eq!(lines[0], "+----------+");
        assert_eq!(lines[3], "+----------+");
        assert_eq!(lines[1], "|boom      |");
        assert_eq!(lines[2], "|error     |");
        for line in &lines {
            assert_eq!(line.chars().count(), 12);
        }
    }

    #[test]
    fn render_top_width_clamp_char_safe() {
        let mut stack = ModalStack::new();
        stack.open(ModalKind::Confirm, "héllo-wörld").unwrap();
        let tiny = stack.render_top(0).unwrap();
        assert_eq!(tiny[0].chars().count(), MIN_BOX_WIDTH);
        for line in &tiny {
            assert_eq!(line.chars().count(), MIN_BOX_WIDTH);
        }
        let huge = stack.render_top(10_000).unwrap();
        assert_eq!(huge[0].chars().count(), MAX_BOX_WIDTH);
        // width 6 -> inner 4: title truncated char-safe, no panic
        let narrow = stack.render_top(6).unwrap();
        assert_eq!(narrow[1].chars().count(), 6);
        assert!(narrow[1].starts_with("|héll"));
    }
}
