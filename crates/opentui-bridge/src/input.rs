#![forbid(unsafe_code)]
//! Key/mouse input events + focus navigation. Pure, bounded, no IO/FFI.

/// Max focusable ids in one ring.
pub const MAX_FOCUS: usize = 64;

/// Key press: raw code plus modifiers.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Key {
    pub code: u32,
    pub ctrl: bool,
    pub alt: bool,
    pub shift: bool,
}

impl Key {
    #[must_use]
    pub const fn new(code: u32, ctrl: bool, alt: bool, shift: bool) -> Self {
        Self { code, ctrl, alt, shift }
    }

    /// Plain key, no modifiers held.
    #[must_use]
    pub const fn plain(code: u32) -> Self {
        Self::new(code, false, false, false)
    }

    /// True when no modifier is held.
    #[must_use]
    pub const fn is_plain(self) -> bool {
        !self.ctrl && !self.alt && !self.shift
    }
}

/// Mouse button.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Middle,
    Right,
}

/// Input event. Cell coords/sizes, origin (0, 0).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum InputEvent {
    Key(Key),
    Mouse { x: u16, y: u16, button: MouseButton },
    Resize { w: u16, h: u16 },
    Focus(bool),
}

/// Fail-closed focus-ring error.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum FocusError {
    Empty,
    Full,
    Invalid,
}

/// Bounded ring of focusable ids. `next`/`prev` wrap around.
#[derive(Debug, Clone)]
pub struct FocusRing {
    ids: [u32; MAX_FOCUS],
    len: usize,
    current: usize,
}

impl FocusRing {
    #[must_use]
    pub const fn new() -> Self {
        Self { ids: [0; MAX_FOCUS], len: 0, current: 0 }
    }

    #[must_use]
    pub const fn len(&self) -> usize {
        self.len
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.len == 0
    }

    #[must_use]
    pub const fn is_full(&self) -> bool {
        self.len >= MAX_FOCUS
    }

    /// Push a nonzero id. Fail-closed on 0 or a full ring.
    pub fn push(&mut self, id: u32) -> Result<(), FocusError> {
        if id == 0 {
            return Err(FocusError::Invalid);
        }
        if self.is_full() {
            return Err(FocusError::Full);
        }
        self.ids[self.len] = id;
        self.len += 1;
        Ok(())
    }

    /// Currently focused id, if any.
    #[must_use]
    pub const fn current(&self) -> Option<u32> {
        if self.len == 0 {
            return None;
        }
        Some(self.ids[self.current])
    }

    /// Advance with wrap, return newly focused id.
    pub fn next(&mut self) -> Result<u32, FocusError> {
        if self.is_empty() {
            return Err(FocusError::Empty);
        }
        self.current = (self.current + 1) % self.len;
        Ok(self.ids[self.current])
    }

    /// Retreat with wrap, return newly focused id.
    pub fn prev(&mut self) -> Result<u32, FocusError> {
        if self.is_empty() {
            return Err(FocusError::Empty);
        }
        self.current = (self.current + self.len - 1) % self.len;
        Ok(self.ids[self.current])
    }

    pub fn clear(&mut self) {
        self.len = 0;
        self.current = 0;
    }
}

impl Default for FocusRing {
    fn default() -> Self {
        Self::new()
    }
}

const _: () = assert!(MAX_FOCUS == 64);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn key_equality() {
        assert_eq!(Key::plain(65), Key::new(65, false, false, false));
        assert_ne!(Key::plain(65), Key::new(65, true, false, false));
        assert_ne!(Key::plain(65), Key::new(65, false, true, false));
        assert_ne!(Key::plain(65), Key::new(65, false, false, true));
        assert_ne!(Key::plain(65), Key::plain(66));
        assert!(Key::plain(65).is_plain());
        assert!(!Key::new(65, true, false, false).is_plain());
    }

    #[test]
    fn empty_ring_error() {
        let mut ring = FocusRing::new();
        assert!(ring.is_empty());
        assert_eq!(ring.len(), 0);
        assert_eq!(ring.current(), None);
        assert_eq!(ring.next(), Err(FocusError::Empty));
        assert_eq!(ring.prev(), Err(FocusError::Empty));
        assert_eq!(ring.push(0), Err(FocusError::Invalid));
    }

    #[test]
    fn ring_wrap() {
        let mut ring = FocusRing::new();
        assert!(ring.push(1).is_ok());
        assert!(ring.push(2).is_ok());
        assert!(ring.push(3).is_ok());
        assert_eq!(ring.current(), Some(1));
        assert_eq!(ring.next(), Ok(2));
        assert_eq!(ring.next(), Ok(3));
        assert_eq!(ring.next(), Ok(1));
        assert_eq!(ring.prev(), Ok(3));
        assert_eq!(ring.prev(), Ok(2));
        assert_eq!(ring.current(), Some(2));
    }

    #[test]
    fn ring_full_and_clear() {
        let mut ring = FocusRing::new();
        for id in 1..=MAX_FOCUS as u32 {
            assert!(ring.push(id).is_ok());
        }
        assert!(ring.is_full());
        assert_eq!(ring.push(MAX_FOCUS as u32 + 1), Err(FocusError::Full));
        ring.clear();
        assert!(ring.is_empty());
        assert_eq!(ring.next(), Err(FocusError::Empty));
    }

    #[test]
    fn input_event_equality() {
        let key = InputEvent::Key(Key::plain(13));
        assert_eq!(key, InputEvent::Key(Key::plain(13)));
        assert_ne!(
            key,
            InputEvent::Mouse { x: 0, y: 0, button: MouseButton::Left }
        );
        assert_eq!(
            InputEvent::Resize { w: 80, h: 24 },
            InputEvent::Resize { w: 80, h: 24 }
        );
        assert_ne!(InputEvent::Focus(true), InputEvent::Focus(false));
    }
}
