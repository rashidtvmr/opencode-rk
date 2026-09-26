#![forbid(unsafe_code)]
//! Renderable prop contracts (BRIDGE-038).
//!
//! SOURCE EVIDENCE (TS checkout /home/rashid/projects/opencode, commit a0d9b6c,
//! NOT pinned 95daf90; cite file:line):
//! - Kinds: `BoxRenderable` (`component/prompt/index.tsx:144`, `util/layout.ts:1`),
//!   `TextareaRenderable` (`component/prompt/index.tsx:143`, `ui/dialog-export-options.tsx:1`,
//!   `ui/dialog-prompt.tsx:1`), `InputRenderable` (`ui/dialog-select.tsx:2`,
//!   `keymap.tsx:1`; `TextareaRenderable` subclass per `keymap.tsx:177`),
//!   `ScrollBoxRenderable` (`ui/dialog-select.tsx:4`, `routes/session/index.tsx:28`),
//!   base `Renderable` (`ui/dialog.tsx:4`, `config/keybind.ts:3`).
//! - Box: `paddingLeft/Right/Top/Bottom` (`ui/dialog-select.tsx:571,603,610-611`,
//!   `routes/session/index.tsx:1166`), `flexGrow/flexShrink`
//!   (`ui/dialog-select.tsx:598`, `routes/home.tsx:72-75`),
//!   `flexDirection` (`ui/dialog-select.tsx:542,640`), `borderColor`
//!   (`routes/session/permission.tsx:477`, `routes/session/index.tsx:1216,1387,1448`,
//!   `ui/toast.tsx:35`), `marginLeft` (`component/prompt/index.tsx:1518`),
//!   `marginRight` (`ui/dialog-select.tsx:756,761`).
//! - Text: `wrapMode="word"|"none"` (`ui/toast.tsx:44`, `ui/dialog-select.tsx:700,770`,
//!   `ui/link.tsx:11,26` with `wrapMode?: "word"|"none"`; `"char"` NOT evidenced in
//!   `packages/tui/src`, kept as upstream-core variant), `truncate`
//!   (`feature-plugins/system/which-key.tsx:477,494`),
//!   `attributes={TextAttributes.BOLD}` (`ui/dialog-select.tsx:625`,
//!   `feature-plugins/system/which-key.tsx:477,494`).
//! - Input/textarea: `placeholder` (`ui/dialog-select.tsx:592`,
//!   `ui/dialog-export-options.tsx:114`), `initialValue`
//!   (`ui/dialog-export-options.tsx:113`, `ui/dialog-prompt.tsx:93`),
//!   `plainText` reads (`ui/dialog-export-options.tsx:101`, `ui/dialog-prompt.tsx:30`,
//!   `routes/session/permission.tsx:468`), `cursorColor`
//!   (`ui/dialog-select.tsx:581`, `ui/dialog-export-options.tsx:118`),
//!   `focusedTextColor`/`textColor` (`ui/dialog-select.tsx:580,582`,
//!   `component/prompt/index.tsx:1370-1371`), `minHeight`/`maxHeight`
//!   (`component/prompt/index.tsx:1372-1373`), `height`
//!   (`ui/dialog-export-options.tsx:108`). `multiline`/`readonly` NOT evidenced as
//!   JSX props in `packages/tui/src` (only `Database {readonly:true}` in
//!   `editor-zed.ts:92`); kept per contract, flagged unevidenced.
//! - Scroll: `stickyScroll` + `stickyStart="bottom"`
//!   (`routes/session/index.tsx:1181-1182`), `scrollAcceleration`
//!   (`ui/dialog-select.tsx:88,613`, `routes/session/index.tsx:274,1184`,
//!   `component/error-component.tsx:181`), `scrollbarOptions={{visible:false}}`
//!   (`ui/dialog-select.tsx:612`).
//! - Focus: `.focus()` (`ui/dialog-select.tsx:589`, `ui/dialog-export-options.tsx:79`,
//!   `ui/dialog-prompt.tsx:54,72`, `ui/dialog.tsx:101`),
//!   `focusedBackgroundColor` (`ui/dialog-select.tsx:580`),
//!   `currentFocusedRenderable` (`ui/dialog.tsx:152`, `util/selection.ts:33,73`).
//!
//! Reuses `crate::attributes::TextAttributes`, `crate::scroll_accel::AccelKind`,
//! `crate::safe_renderer::MAX_TEXT_BYTES`; defines nothing from
//! `buffer.rs`/`color.rs`/`attributes.rs`.

use crate::attributes::TextAttributes;
use crate::safe_renderer::MAX_TEXT_BYTES;
use crate::scroll_accel::AccelKind;
use crate::color::Rgba;

/// Renderable discriminant (evidenced kinds only).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub enum RenderableKind {
    Box,
    Text,
    Input,
    Textarea,
    Scroll,
    FrameBufferCustom,
}

/// Text wrap mode. `Char` unevidenced in `packages/tui/src` (only
/// `"word"|"none"` per `ui/link.tsx:11`); kept for upstream core parity.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum WrapMode {
    #[default]
    None,
    Word,
    Char,
}

impl WrapMode {
    /// Parse TS `wrapMode` prop values (`"word"|"none"`); `None` on unknown.
    #[must_use]
    pub const fn parse(s: &str) -> Option<Self> {
        match s.as_bytes() {
            b"none" => Some(Self::None),
            b"word" => Some(Self::Word),
            b"char" => Some(Self::Char),
            _ => None,
        }
    }
}

/// Validation failure.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderableError {
    TextTooLarge,
    CursorOutOfRange,
    FocusedWithoutFocusable,
    InputMinExceedsMax,
    FrameBufferNameInvalid,
}

/// Box padding (`paddingLeft/Right/Top/Bottom`, e.g. `dialog-select.tsx:571`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Padding {
    pub left: u8,
    pub right: u8,
    pub top: u8,
    pub bottom: u8,
}

/// Box margin (`marginLeft` `prompt/index.tsx:1518`, `marginRight`
/// `dialog-select.tsx:756`; top/bottom NOT evidenced).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Margin {
    pub left: u8,
    pub right: u8,
}

/// Flex (`flexGrow`/`flexShrink`, e.g. `dialog-select.tsx:598`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Flex {
    pub grow: u8,
    pub shrink: u8,
}

/// Box props: border flag + RGBA color (`borderColor`, e.g.
/// `permission.tsx:477`); `[u16;4]` lanes per `buffer.rs` convention.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BoxProps {
    pub border: bool,
    pub border_color: Option<[u16; 4]>,
    pub padding: Padding,
    pub margin: Margin,
    pub flex: Option<Flex>,
}

/// Text props (`wrapMode` `toast.tsx:44`, `truncate` `which-key.tsx:477`).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct TextProps {
    pub content: String,
    pub wrap: WrapMode,
    pub attributes: TextAttributes,
    pub truncate: bool,
}

/// Input/textarea props (`placeholder` `dialog-select.tsx:592`, `cursorColor`
/// `dialog-select.tsx:581`; `multiline`/`readonly` unevidenced in tui src).
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct InputProps {
    pub value: String,
    pub cursor: usize,
    pub multiline: bool,
    pub readonly: bool,
    pub placeholder: Option<String>,
}

/// Scroll props (`stickyScroll`/`stickyStart="bottom"` `session/index.tsx:1181-1182`,
/// `scrollAcceleration` `dialog-select.tsx:613`,
/// `scrollbarOptions.visible` `dialog-select.tsx:612`).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct ScrollProps {
    pub stick_to_bottom: bool,
    pub accel: AccelKind,
    pub scrollbar_visible: bool,
}

impl Default for ScrollProps {
    fn default() -> Self {
        Self {
            stick_to_bottom: false,
            accel: AccelKind::Custom(crate::scroll_accel::DEFAULT_SPEED),
            scrollbar_visible: true,
        }
    }
}

/// Focus state (`.focus()` `dialog-select.tsx:589`,
/// `focusedBackgroundColor` `dialog-select.tsx:580`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct Focusable {
    pub focusable: bool,
    pub focused: bool,
}

/// Reject `content` over [`MAX_TEXT_BYTES`] bytes.
pub const fn validate_text(p: &TextProps) -> Result<(), RenderableError> {
    if p.content.len() > MAX_TEXT_BYTES {
        return Err(RenderableError::TextTooLarge);
    }
    Ok(())
}

/// Reject `value` over [`MAX_TEXT_BYTES`] bytes or `cursor` past char count.
pub fn validate_input(p: &InputProps) -> Result<(), RenderableError> {
    if p.value.len() > MAX_TEXT_BYTES {
        return Err(RenderableError::TextTooLarge);
    }
    if p.cursor > p.value.chars().count() {
        return Err(RenderableError::CursorOutOfRange);
    }
    Ok(())
}

/// Reject `focused` without `focusable`.
pub const fn validate_focus(f: &Focusable) -> Result<(), RenderableError> {
    if f.focused && !f.focusable {
        return Err(RenderableError::FocusedWithoutFocusable);
    }
    Ok(())
}


/// Flex direction (`flexDirection="row"|"column"`, `ui/dialog-select.tsx:542`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum FlexDir {
    #[default]
    Row,
    Column,
}

impl FlexDir {
    /// Parse TS `flexDirection` values; `None` on unknown.
    #[must_use]
    pub const fn parse(s: &str) -> Option<Self> {
        match s.as_bytes() {
            b"row" => Some(Self::Row),
            b"column" => Some(Self::Column),
            _ => None,
        }
    }
}

/// Justify content (`justifyContent`, `ui/dialog-select.tsx:558`
/// `"space-between"`, `ui/dialog-confirm.tsx` `"flex-end"`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum Justify {
    #[default]
    Start,
    Center,
    SpaceBetween,
    End,
}

impl Justify {
    /// Parse TS `justifyContent` values; `None` on unknown.
    #[must_use]
    pub const fn parse(s: &str) -> Option<Self> {
        match s.as_bytes() {
            b"flex-start" => Some(Self::Start),
            b"center" => Some(Self::Center),
            b"space-between" => Some(Self::SpaceBetween),
            b"flex-end" => Some(Self::End),
            _ => None,
        }
    }
}

/// Flex layout (`flexDirection`/`justifyContent`/`gap`/`width`,
/// `ui/dialog-select.tsx:542,558`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct FlexLayout {
    pub direction: FlexDir,
    pub justify: Justify,
    pub gap: u16,
}

/// `FlexLayout` is fully bounded (`gap: u16`); always valid.
pub const fn validate_flex_layout(_l: &FlexLayout) -> Result<(), RenderableError> {
    Ok(())
}

/// Box fg/bg colors (`backgroundColor`/`fg`, `ui/dialog-select.tsx:543`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct BoxStyle {
    pub fg: Option<Rgba>,
    pub bg: Option<Rgba>,
}

/// `BoxStyle` carries only bounded `Rgba`; always valid.
pub const fn validate_box_style(_s: &BoxStyle) -> Result<(), RenderableError> {
    Ok(())
}

/// Text fg + clickability (`fg` + `onMouseUp`, `ui/dialog-confirm.tsx:62`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct TextStyle {
    pub fg: Option<Rgba>,
    pub clickable: bool,
}

/// `TextStyle` carries only bounded `Rgba`/bool; always valid.
pub const fn validate_text_style(_s: &TextStyle) -> Result<(), RenderableError> {
    Ok(())
}

/// Input colors + height clamp (`placeholderColor`/`textColor`/`cursorColor`,
/// `minHeight`/`maxHeight`, `component/prompt/index.tsx:1368-1373`).
/// Event callbacks (`onContentChange`/`onCursorChange`/`onKeyDown`/`onSubmit`/
/// `onPaste`, `:1375-1393`) stay TS-side; only action-id consts cross here.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct InputStyle {
    pub placeholder_color: Option<Rgba>,
    pub text_color: Option<Rgba>,
    pub cursor_color: Option<Rgba>,
    pub min_height: u16,
    pub max_height: u16,
}

/// Input event action ids (callbacks stay TS-side).
pub const INPUT_CONTENT_CHANGE: &str = "input.content_change";
pub const INPUT_CURSOR_CHANGE: &str = "input.cursor_change";
pub const INPUT_KEY_DOWN: &str = "input.key_down";
pub const INPUT_SUBMIT: &str = "input.submit";
pub const INPUT_PASTE: &str = "input.paste";

/// Reject `min_height` exceeding `max_height`.
pub const fn validate_input_style(s: &InputStyle) -> Result<(), RenderableError> {
    if s.min_height > s.max_height {
        return Err(RenderableError::InputMinExceedsMax);
    }
    Ok(())
}

/// Scrollbar track colors (`trackOptions.backgroundColor`/`foregroundColor`,
/// `routes/session/index.tsx:1175-1180`).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct ScrollTrack {
    pub bg: Option<Rgba>,
    pub fg: Option<Rgba>,
}

/// `ScrollTrack` carries only bounded `Rgba`; always valid.
pub const fn validate_scroll_track(_t: &ScrollTrack) -> Result<(), RenderableError> {
    Ok(())
}

/// Sticky edge (`stickyStart="bottom"`, `routes/session/index.tsx:1181-1182`;
/// `"top"` is the upstream-core counterpart).
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub enum StickyStart {
    Top,
    #[default]
    Bottom,
}

impl StickyStart {
    /// Parse TS `stickyStart` values; `None` on unknown.
    #[must_use]
    pub const fn parse(s: &str) -> Option<Self> {
        match s.as_bytes() {
            b"top" => Some(Self::Top),
            b"bottom" => Some(Self::Bottom),
            _ => None,
        }
    }
}

/// `StickyStart` is a closed enum; always valid.
pub const fn validate_sticky_start(_s: StickyStart) -> Result<(), RenderableError> {
    Ok(())
}

/// Custom framebuffer payload (`FrameBufferRenderable` subclass,
/// `component/bg-pulse.tsx:19`). Name travels here so `RenderableKind` keeps
/// its `Copy` bound (unit variant below).
#[derive(Debug, Clone, Default, PartialEq, Eq, Hash)]
pub struct FrameBufferCustom {
    pub name: String,
}

/// Reject empty names or names over [`MAX_TEXT_BYTES`] bytes.
pub fn validate_framebuffer_custom(f: &FrameBufferCustom) -> Result<(), RenderableError> {
    if f.name.is_empty() || f.name.len() > MAX_TEXT_BYTES {
        return Err(RenderableError::FrameBufferNameInvalid);
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn kind_variants_distinct() {
        assert_ne!(RenderableKind::Box, RenderableKind::Scroll);
        assert_ne!(RenderableKind::Input, RenderableKind::Textarea);
    }

    #[test]
    fn wrap_parse_evidenced_values() {
        // ui/link.tsx:11, toast.tsx:44.
        assert_eq!(WrapMode::parse("word"), Some(WrapMode::Word));
        assert_eq!(WrapMode::parse("none"), Some(WrapMode::None));
        assert_eq!(WrapMode::parse("bogus"), None);
        assert_eq!(WrapMode::default(), WrapMode::None);
    }

    #[test]
    fn text_rejects_oversize() {
        let ok = TextProps { content: "hi".into(), ..Default::default() };
        assert!(validate_text(&ok).is_ok());
        let big = TextProps { content: "x".repeat(MAX_TEXT_BYTES + 1), ..Default::default() };
        assert_eq!(validate_text(&big), Err(RenderableError::TextTooLarge));
    }

    #[test]
    fn input_cursor_bounded_by_chars() {
        let ok = InputProps { value: "ab".into(), cursor: 2, ..Default::default() };
        assert!(validate_input(&ok).is_ok());
        let past = InputProps { value: "ab".into(), cursor: 3, ..Default::default() };
        assert_eq!(validate_input(&past), Err(RenderableError::CursorOutOfRange));
        // Multibyte: cursor counts chars, not bytes.
        let uni = InputProps { value: "é".into(), cursor: 1, ..Default::default() };
        assert!(validate_input(&uni).is_ok());
    }

    #[test]
    fn focus_requires_focusable() {
        let bad = Focusable { focusable: false, focused: true };
        assert_eq!(validate_focus(&bad), Err(RenderableError::FocusedWithoutFocusable));
        let good = Focusable { focusable: true, focused: true };
        assert!(validate_focus(&good).is_ok());
        assert!(validate_focus(&Focusable::default()).is_ok());
    }

    #[test]
    fn scroll_sticky_bottom_evidence() {
        // routes/session/index.tsx:1181-1182.
        let p = ScrollProps { stick_to_bottom: true, scrollbar_visible: false, ..Default::default() };
        assert!(p.stick_to_bottom);
        assert!(!p.scrollbar_visible);
        assert_eq!(p.accel, AccelKind::Custom(crate::scroll_accel::DEFAULT_SPEED));
    }

    #[test]
    fn box_padding_margin_defaults_zero() {
        let b = BoxProps::default();
        assert_eq!(b.padding, Padding { left: 0, right: 0, top: 0, bottom: 0 });
        assert_eq!(b.margin, Margin { left: 0, right: 0 });
        assert!(!b.border);
        assert_eq!(b.flex, None);
    }

    #[test]
    fn flex_layout_parse_and_validate() {
        // ui/dialog-select.tsx:542,558.
        assert_eq!(FlexDir::parse("row"), Some(FlexDir::Row));
        assert_eq!(FlexDir::parse("column"), Some(FlexDir::Column));
        assert_eq!(FlexDir::parse("bogus"), None);
        assert_eq!(Justify::parse("space-between"), Some(Justify::SpaceBetween));
        assert_eq!(Justify::parse("flex-end"), Some(Justify::End));
        assert_eq!(Justify::parse("bogus"), None);
        let l = FlexLayout { direction: FlexDir::Row, justify: Justify::SpaceBetween, gap: 1 };
        assert!(validate_flex_layout(&l).is_ok());
    }

    #[test]
    fn box_style_validate() {
        // ui/dialog-select.tsx:543.
        let s = BoxStyle { fg: Some(Rgba::rgb(1, 2, 3)), bg: None };
        assert!(validate_box_style(&s).is_ok());
        assert!(validate_box_style(&BoxStyle::default()).is_ok());
    }

    #[test]
    fn text_style_validate() {
        // ui/dialog-confirm.tsx:62.
        let s = TextStyle { fg: None, clickable: true };
        assert!(validate_text_style(&s).is_ok());
    }

    #[test]
    fn input_style_height_clamp() {
        // component/prompt/index.tsx:1368-1373.
        let ok = InputStyle { min_height: 1, max_height: 5, ..Default::default() };
        assert!(validate_input_style(&ok).is_ok());
        let bad = InputStyle { min_height: 6, max_height: 5, ..Default::default() };
        assert_eq!(validate_input_style(&bad), Err(RenderableError::InputMinExceedsMax));
        assert_eq!(INPUT_SUBMIT, "input.submit");
        assert_eq!(INPUT_PASTE, "input.paste");
    }

    #[test]
    fn scroll_track_and_sticky_start() {
        // routes/session/index.tsx:1175-1182.
        let t = ScrollTrack { bg: Some(Rgba::rgb(0, 0, 0)), fg: None };
        assert!(validate_scroll_track(&t).is_ok());
        assert_eq!(StickyStart::parse("bottom"), Some(StickyStart::Bottom));
        assert_eq!(StickyStart::default(), StickyStart::Bottom);
        assert!(validate_sticky_start(StickyStart::Top).is_ok());
    }

    #[test]
    fn framebuffer_custom_kind_and_name() {
        // component/bg-pulse.tsx:19.
        assert_ne!(RenderableKind::FrameBufferCustom, RenderableKind::Box);
        let ok = FrameBufferCustom { name: "go-upsell".into() };
        assert!(validate_framebuffer_custom(&ok).is_ok());
        assert_eq!(
            validate_framebuffer_custom(&FrameBufferCustom::default()),
            Err(RenderableError::FrameBufferNameInvalid)
        );
    }
}
