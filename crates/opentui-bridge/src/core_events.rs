#![forbid(unsafe_code)]
//! Core render/input event contract (pure, std only, no IO/FFI).
//!
//! Mirrors `CliRenderEvents` (16 members, chunk-bun-bb3k0yt8.js:7068-7085)
//! plus runtime extras used by opencode TUI: copy-selection
//! (`app.tsx:204,436-446`), bracketed paste (`prompt/index.tsx:1393-1417`,
//! paste markers `"\x1B[200~"/"\x1B[201~"` at chunk-bun:6427-6428),
//! focus/blur (`attention.ts:126-134`), scroll/mouse (console
//! `scroll-up/down` bindings chunk-bun:4662-4665, `MouseButton`
//! chunk-bun:7035-7042). Reuses `events::{EventBus,EventKind}` and
//! `input::{Key,MouseButton,FocusRing}`; nothing redefined.

use crate::events::EventKind;
use crate::input::{Key, MouseButton};

/// Max stored paste chars. Longer input truncated on char boundary.
pub const MAX_PASTE_LEN: usize = 65_536;

/// Kitty keyboard flag bits (chunk-bun:6968-6972).
pub const KITTY_DISAMBIGUATE: u8 = 1;
pub const KITTY_EVENT_TYPES: u8 = 2;
pub const KITTY_ALTERNATE_KEYS: u8 = 4;
pub const KITTY_ALL_KEYS_AS_ESCAPES: u8 = 8;
pub const KITTY_REPORT_TEXT: u8 = 16;

/// Kitty progressive-enhancement flags.
// NOTE: `buildKittyKeyboardFlags({})` returns 5, not 0: `disambiguate`
// and `alternateKeys` default on (`!== false`, chunk-bun:6975-6995,
// call site chunk-bun:7359-7361, `app.tsx:199 useKittyKeyboard: {}`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct KittyFlags {
    bits: u8,
}

impl KittyFlags {
    #[must_use]
    pub const fn new(bits: u8) -> Self {
        Self { bits }
    }

    /// Flags for empty config `{}`: DISAMBIGUATE|ALTERNATE_KEYS = 5.
    #[must_use]
    pub const fn from_empty() -> Self {
        Self { bits: KITTY_DISAMBIGUATE | KITTY_ALTERNATE_KEYS }
    }

    #[must_use]
    pub const fn empty() -> Self {
        Self { bits: 0 }
    }

    #[must_use]
    pub const fn bits(self) -> u8 {
        self.bits
    }

    #[must_use]
    pub const fn contains(self, flag: u8) -> bool {
        self.bits & flag == flag
    }
}

/// Bracketed paste payload, content truncated to [`MAX_PASTE_LEN`].
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Paste {
    pub bracketed: bool,
    pub content: String,
}

impl Paste {
    #[must_use]
    pub fn new(bracketed: bool, content: &str) -> Self {
        let mut end = content.len().min(MAX_PASTE_LEN);
        while end > 0 && !content.is_char_boundary(end) {
            end -= 1;
        }
        Self { bracketed, content: content[..end].to_string() }
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.content.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.content.is_empty()
    }
}

/// Console key binding: key name + key with modifiers.
/// Upstream default copy is ctrl+shift+c (chunk-bun:4681);
/// opencode overrides to ctrl+y (`app.tsx:204`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct ConsoleBinding {
    pub name: &'static str,
    pub key: Key,
}

impl ConsoleBinding {
    /// Opencode default: `y` + ctrl -> `copy-selection` (`app.tsx:204`).
    #[must_use]
    pub const fn copy_selection() -> Self {
        Self { name: "copy-selection", key: Key::new(121, true, false, false) }
    }

    /// Upstream opentui default: `c` + ctrl + shift (chunk-bun:4681).
    #[must_use]
    pub const fn upstream_copy() -> Self {
        Self { name: "copy-selection", key: Key::new(99, true, false, true) }
    }

    #[must_use]
    pub fn matches(self, key: Key) -> bool {
        self.key == key
    }
}

/// Every `CliRenderEvents` member + runtime extras.
/// Cell coords, origin (0,0). Wheel maps to `Scroll`; `Mouse` reuses
/// `input::MouseButton` (Left/Middle/Right only, no wheel variants).
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum RenderEvent {
    Resize { w: u16, h: u16 },
    Frame,
    RenderError,
    HandlerError,
    ExternalOutput,
    Focus,
    Blur,
    FocusedRenderable,
    FocusedEditor,
    ThemeMode { dark: bool },
    Palette { size: u16 },
    Capabilities,
    Selection { len: usize },
    DebugOverlayToggle,
    Destroy,
    MemorySnapshot,
    CopySelection { len: usize },
    Paste(Paste),
    Scroll { delta: i16 },
    Mouse { x: u16, y: u16, button: MouseButton },
}

/// Route a render event into the existing bus kind.
#[must_use]
pub const fn route_event(ev: &RenderEvent) -> EventKind {
    match ev {
        RenderEvent::Resize { .. } => EventKind::Resize,
        RenderEvent::Frame => EventKind::Tick,
        RenderEvent::RenderError
        | RenderEvent::HandlerError
        | RenderEvent::ExternalOutput
        | RenderEvent::Destroy
        | RenderEvent::MemorySnapshot => EventKind::Paint,
        _ => EventKind::Input,
    }
}

const _: () = assert!(MAX_PASTE_LEN == 65_536);

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kitty_from_empty_is_five() {
        // buildKittyKeyboardFlags({}) => DISAMBIGUATE|ALTERNATE_KEYS.
        let f = KittyFlags::from_empty();
        assert_eq!(f.bits(), 5);
        assert!(f.contains(KITTY_DISAMBIGUATE));
        assert!(f.contains(KITTY_ALTERNATE_KEYS));
        assert!(!f.contains(KITTY_EVENT_TYPES));
    }

    #[test]
    fn kitty_bits_roundtrip() {
        let f = KittyFlags::new(KITTY_EVENT_TYPES | KITTY_REPORT_TEXT);
        assert_eq!(f.bits(), 18);
        assert!(f.contains(KITTY_EVENT_TYPES));
        assert!(!f.contains(KITTY_ALL_KEYS_AS_ESCAPES));
        assert_eq!(KittyFlags::empty().bits(), 0);
    }

    #[test]
    fn paste_truncates_to_bound() {
        let long = "a".repeat(MAX_PASTE_LEN + 100);
        let p = Paste::new(true, &long);
        assert!(p.bracketed);
        assert_eq!(p.len(), MAX_PASTE_LEN);
        let p2 = Paste::new(false, "");
        assert!(p2.is_empty());
        assert!(!p2.bracketed);
    }

    #[test]
    fn copy_binding_defaults() {
        // app.tsx:204 y+ctrl; upstream ctrl+shift+c.
        let b = ConsoleBinding::copy_selection();
        assert_eq!(b.name, "copy-selection");
        assert_eq!(b.key, Key::new(121, true, false, false));
        assert!(b.matches(Key::new(121, true, false, false)));
        assert!(!b.matches(Key::plain(121)));
        assert_eq!(
            ConsoleBinding::upstream_copy().key,
            Key::new(99, true, false, true)
        );
    }

    #[test]
    fn route_mapping() {
        assert_eq!(route_event(&RenderEvent::Resize { w: 80, h: 24 }), EventKind::Resize);
        assert_eq!(route_event(&RenderEvent::Frame), EventKind::Tick);
        assert_eq!(route_event(&RenderEvent::RenderError), EventKind::Paint);
        assert_eq!(route_event(&RenderEvent::Destroy), EventKind::Paint);
        assert_eq!(route_event(&RenderEvent::Focus), EventKind::Input);
        assert_eq!(route_event(&RenderEvent::Blur), EventKind::Input);
        assert_eq!(
            route_event(&RenderEvent::Mouse { x: 1, y: 2, button: MouseButton::Left }),
            EventKind::Input
        );
        assert_eq!(route_event(&RenderEvent::Scroll { delta: -1 }), EventKind::Input);
        assert_eq!(
            route_event(&RenderEvent::Paste(Paste::new(true, "hi"))),
            EventKind::Input
        );
        assert_eq!(route_event(&RenderEvent::CopySelection { len: 3 }), EventKind::Input);
    }

    #[test]
    fn all_cli_members_constructible() {
        // All 16 CliRenderEvents members evidenced at chunk-bun:7070-7085.
        let all = [
            RenderEvent::Resize { w: 1, h: 1 },
            RenderEvent::Frame,
            RenderEvent::RenderError,
            RenderEvent::HandlerError,
            RenderEvent::ExternalOutput,
            RenderEvent::Focus,
            RenderEvent::Blur,
            RenderEvent::FocusedRenderable,
            RenderEvent::FocusedEditor,
            RenderEvent::ThemeMode { dark: true },
            RenderEvent::Palette { size: 16 },
            RenderEvent::Capabilities,
            RenderEvent::Selection { len: 0 },
            RenderEvent::DebugOverlayToggle,
            RenderEvent::Destroy,
            RenderEvent::MemorySnapshot,
        ];
        assert_eq!(all.len(), 16);
        for ev in &all {
            let _ = route_event(ev);
        }
    }
}
