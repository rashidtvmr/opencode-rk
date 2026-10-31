#![forbid(unsafe_code)]
//! App-scope modal stack state.
//!
//! Pure state only: no IO, no threads. Caller renders
//! [`ModalStack::top`] and executes confirm/picker side effects;
//! [`ModalStack::render_top`] builds ASCII box lines (pure).
//!
//! [`KeyScope`] models which layer consumes keyboard input. Upstream keeps
//! input modes mutually exclusive: the form route pushes `FORM_MODE` on
//! mount and pops it through the returned cleanup
//! (`packages/tui/src/routes/session/form.tsx:536`), while the keymap mode
//! API documents "Mutually exclusive OpenCode input modes" and "pushes a
//! mode until the returned cleanup is called"
//! (`packages/tui/src/context/keymap.tsx:173-178`). Permission and question
//! overlays likewise register their key layer only while mounted
//! (`packages/tui/src/routes/session/permission.tsx:284`,
//! `permission.tsx:440`), so an open overlay suppresses composer keys and
//! closing it restores the previous owner.

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
    pub fn open(&mut self, kind: ModalKind, title: impl Into<String>) -> Result<(), ModalError> {
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

/// Which layer consumes keyboard input right now. [`KeyOwner::Composer`] is
/// the resting state; every overlay pushes over it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum KeyOwner {
    Composer,
    Permission,
    Question,
    Form,
    Modal,
}

/// LIFO stack of keyboard owners, bounded by [`MAX_MODALS`].
///
/// Only the top owner receives keys. Pushing an overlay suppresses composer
/// input; popping restores the previous owner, and the composer owns keys
/// again once the stack empties (the upstream mode-push cleanup contract,
/// `form.tsx:536`, `keymap.tsx:173-178`).
#[derive(Debug, Default)]
pub struct KeyScope {
    owners: Vec<KeyOwner>,
}

impl KeyScope {
    #[must_use]
    pub fn new() -> Self {
        Self { owners: Vec::new() }
    }

    /// Take keyboard ownership for an overlay. Returns `false` when
    /// [`KeyOwner::Composer`] is pushed (it rests instead of stacking) or
    /// when the stack is full. Re-pushing the current owner is allowed so a
    /// caller can re-assert ownership idempotently.
    pub fn push(&mut self, owner: KeyOwner) -> bool {
        if owner == KeyOwner::Composer || self.owners.len() >= MAX_MODALS {
            return false;
        }
        self.owners.push(owner);
        true
    }

    /// Give keyboard ownership back to the previous owner. Returns the
    /// owner that was popped, or `None` when only the composer holds keys.
    pub fn pop(&mut self) -> Option<KeyOwner> {
        self.owners.pop()
    }

    /// The layer that receives keys: the top overlay, else the composer.
    #[must_use]
    pub fn owner(&self) -> KeyOwner {
        self.owners.last().copied().unwrap_or(KeyOwner::Composer)
    }

    /// True while any overlay owns keys, i.e. composer routing must be
    /// suppressed.
    #[must_use]
    pub fn suppresses_composer(&self) -> bool {
        !self.owners.is_empty()
    }

    /// Drop every overlay owner and return to the composer (overlay teardown
    /// or hard dismissal). Returns how many owners were dropped.
    pub fn restore_composer(&mut self) -> usize {
        let dropped = self.owners.len();
        self.owners.clear();
        dropped
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.owners.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.owners.is_empty()
    }
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
    out.extend(std::iter::repeat_n(' ', missing));
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
        stack
            .open(ModalKind::Picker, "x".repeat(MAX_TITLE_LEN + 10))
            .unwrap();
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

    // Upstream pops the form mode through the push cleanup
    // (`packages/tui/src/routes/session/form.tsx:536`) and only the mounted
    // route's key layer is reachable (`packages/tui/src/context/keymap.tsx:173-178`,
    // `routes/session/permission.tsx:284`). Nested overlays therefore own the
    // keyboard in LIFO order and closing them returns keys to the composer.
    #[test]
    fn form_008_overlay_key_scope_restores_composer() {
        let mut scope = KeyScope::new();
        assert_eq!(scope.owner(), KeyOwner::Composer);
        assert!(!scope.suppresses_composer());

        // A form takes keys from the composer.
        assert!(scope.push(KeyOwner::Form));
        assert_eq!(scope.owner(), KeyOwner::Form);
        assert!(scope.suppresses_composer());

        // A modal opened over the form owns keys; the form keeps its place.
        assert!(scope.push(KeyOwner::Modal));
        assert_eq!(scope.owner(), KeyOwner::Modal);
        assert_eq!(scope.len(), 2);

        // Closing the modal restores the form, not the composer.
        assert_eq!(scope.pop(), Some(KeyOwner::Modal));
        assert_eq!(scope.owner(), KeyOwner::Form);

        // Closing the form returns keys to the composer.
        assert_eq!(scope.pop(), Some(KeyOwner::Form));
        assert_eq!(scope.owner(), KeyOwner::Composer);
        assert!(!scope.suppresses_composer());
        assert_eq!(scope.pop(), None, "composer is never popped");

        // The composer can never be stacked; re-pushing is idempotent-safe.
        assert!(!scope.push(KeyOwner::Composer));
        assert!(scope.push(KeyOwner::Permission));
        assert!(scope.push(KeyOwner::Permission));
        assert_eq!(scope.len(), 2);
        assert_eq!(scope.restore_composer(), 2, "teardown drops every owner");
        assert_eq!(scope.owner(), KeyOwner::Composer);

        // Ownership is bounded like the modal stack.
        for _ in 0..MAX_MODALS {
            assert!(scope.push(KeyOwner::Question));
        }
        assert!(!scope.push(KeyOwner::Question));
        assert_eq!(scope.len(), MAX_MODALS);
    }
}
