#![forbid(unsafe_code)]
//! Geometry and painting model for the native session tabs.
//!
//! This module deliberately does not own navigation.  [`TabStrip::from_navigation`]
//! takes a snapshot of the existing navigation state and keeps session IDs as the
//! only identity in its hit map.  Consequently clipping, orientation changes and
//! a resize can never make a painted cell select a different session.

use crate::native_navigation::{NavigationState, SessionRef, Tab, WorkState};
use crate::native_view_layout::{fit, visit_unpadded_cells};

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabOrientation {
    Horizontal,
    Vertical,
}

/// The one shell reservation shared by tab painting, content layout, and input.
#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct TabReservation {
    pub orientation: Option<TabOrientation>,
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl TabReservation {
    pub const VERTICAL_WIDTH: u16 = 42;
    pub const CONTENT_PREFERRED_WIDTH: u16 = 64;
    pub const VERTICAL_MIN_WIDTH: u16 = 5;
    pub const VERTICAL_MAX_WIDTH: u16 = 72;
    pub const CONTENT_MIN_WIDTH: u16 = 44;
    pub const ADD_TAB_WIDTH: u16 = 3;

    /// Upstream packages/tui/src/context/session-tabs-model.ts:8
    /// `NEW_SESSION_TAB_TITLE = "New session"`.
    pub const HOME_TITLE: &str = "New session";

    /// Port of upstream `sessionTabsFitVertically` at the pinned revision.
    /// Horizontal and vertical offsets of the reserved content area.
    #[must_use]
    pub const fn x_offset(self) -> u16 {
        match self.orientation {
            Some(TabOrientation::Vertical) => self.width,
            _ => 0,
        }
    }

    #[must_use]
    pub const fn y_offset(self) -> u16 {
        match self.orientation {
            Some(TabOrientation::Horizontal) => self.height,
            _ => 0,
        }
    }

    pub fn for_view(orientation: TabOrientation, cols: u16, rows: u16) -> Self {
        Self::for_view_with_width(orientation, cols, rows, Self::VERTICAL_WIDTH)
    }

    pub fn for_view_with_width(orientation: TabOrientation, cols: u16, rows: u16, vertical_width: u16) -> Self {
        let vertical_width = vertical_width
            .max(Self::VERTICAL_MIN_WIDTH)
            .min(Self::VERTICAL_MAX_WIDTH)
            .min(cols.saturating_sub(Self::CONTENT_MIN_WIDTH).max(Self::VERTICAL_MIN_WIDTH));
        match orientation {
            TabOrientation::Horizontal if rows > 0 => Self {
                orientation: Some(TabOrientation::Horizontal),
                width: cols,
                height: 1,
                ..Self::default()
            },
            TabOrientation::Vertical
                if rows > 0
                    && cols >= vertical_width.saturating_add(Self::CONTENT_PREFERRED_WIDTH) =>
            {
                Self {
                    orientation: Some(TabOrientation::Vertical),
                    width: vertical_width,
                    height: rows,
                    ..Self::default()
                }
            }
            TabOrientation::Vertical => Self::for_view_with_width(TabOrientation::Horizontal, cols, rows, vertical_width),
            _ => Self::default(),
        }
    }
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Default)]
pub struct TabFlags {
    pub active: bool,
    pub busy: bool,
    pub needs_attention: bool,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabInput {
    pub session: SessionRef,
    pub title: String,
    pub flags: TabFlags,
}

impl TabInput {
    pub fn from_tab(nav: &NavigationState, tab: &Tab, active: bool) -> Self {
        Self {
            title: nav.title(&tab.session).unwrap_or(&tab.session.0).to_owned(),
            flags: TabFlags {
                active,
                busy: nav.work_state(&tab.session) == Some(WorkState::Running),
                needs_attention: false,
            },
            session: tab.session.clone(),
        }
    }
}

/// Caller-owned attention facts, kept separate from daemon work state.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attention {
    pub session: SessionRef,
    pub needs_attention: bool,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TabRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl TabRect {
    fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width)
            && y < self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabHit {
    pub session: SessionRef,
    pub close: bool,
}

/// Result of resolving a mouse click into a tab action. Carries the stable
/// session identity of the clicked tab and whether the close child was hit
/// (only true when the tab is also hovered, per upstream).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabAction {
    pub session: SessionRef,
    pub close: bool,
}
/// Stable horizontal-strip identity. Home is a real pseudo-tab, never a miss
/// reinterpreted as the active session; `NewControl` is the separate idle
/// add affordance (upstream `showPlus`, mutually exclusive with Home).
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HorizontalTarget {
    Session(SessionRef),
    Home,
    NewControl,
}

/// Middle-meaning split, source-backed upstream session-tabs.tsx middle press:
/// `tabs.close(tab === NEW_SESSION_TAB ? undefined : tab.sessionID)`.
/// `close(undefined)` on Home navigates to the previous focused session or the
/// last remaining tab (context/session-tabs.tsx close(): `previous.sessionID
/// ?? state().tabs.at(-1)?.sessionID`), and stays Home with none. The
/// standalone plus has no parent tab, so its middle press is inert.
#[derive(Clone, Debug, Eq, PartialEq)]
pub enum HorizontalMiddleAction {
    CloseSession(SessionRef),
    CloseHomeNavigatePrevious,
    Inert,
}

/// Pure identity mapping shared by the live input loop and tests.
#[must_use]
pub fn middle_action(target: &HorizontalTarget) -> HorizontalMiddleAction {
    match target {
        HorizontalTarget::Session(id) => HorizontalMiddleAction::CloseSession(id.clone()),
        HorizontalTarget::Home => HorizontalMiddleAction::CloseHomeNavigatePrevious,
        HorizontalTarget::NewControl => HorizontalMiddleAction::Inert,
    }
}

/// Per-row color role for the horizontal strip, enabling multi-color rows
/// without guessing glyph text. Vertical rail rows do not use this; they are
/// painted by native_tab_rail with its own CellColor semantics.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabCellColor {
    /// Idle tab: bg=background.base, fg=text.muted.
    Idle,
    /// Selected tab: bg=raised.high, fg=text.base, bold.
    Selected,
    /// Hovered (not selected): bg=raised.high, fg=text.base.
    Hovered,
    /// New-session idle affordance: plus fg=idleNumber, "New session" fg=TextMuted.
    AddIdle,
    /// New-session hovered affordance: bg=raised.high, plus+text fg=text.base.
    AddHovered,
    /// Promoted Home slot (active new tab): bg=raised.high fg=text.base, bold.
    HomeActive,
    /// Promoted Home slot hovered (not active): bg=raised.high fg=text.base.
    HomeHovered,
}

/// Resolved horizontal paint parameters for one tab row, derived from the
/// palette via upstream theme mappings (session-tabs.tsx :1569-1736,
/// :1770-1794). All colors are raw palette `[u8;4]`. Zero-valued channels in
/// unused roles are ignored by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabRowPaint {
    /// Background: idle->background.base, hovered/selected->raised.high.
    pub background: [u8; 4],
    /// Title/number fg: idle=text.muted, hovered/selected=text.base.
    pub title_fg: [u8; 4],
    /// Close glyph fg: tint(text.muted, text.base, 0.6) per upstream :1731-1736.
    pub close_fg: [u8; 4],
    /// Plus "+" fg for the add affordance: idleNumber (idle) / Text (hovered).
    pub plus_fg: [u8; 4],
    /// "New session" text fg: TextMuted (idle) / Text (hovered).
    pub new_text_fg: [u8; 4],
    /// Bold attribute: selected (or dragged) -> 1, else 0.
    pub title_attr: u32,
    /// Color role tag for this row (Idle/Selected/Hovered/Add/Hover/Home).
    pub color: TabCellColor,
    /// Number/indicator column fg per upstream session-tabs.tsx:1648-1658.
    /// Zero channels mean "caller falls back to title_fg" (Home/add conven-
    /// tional rows keep their own upstream prop colors).
    pub number_fg: [u8; 4],
}

impl TabRowPaint {
    /// Idle horizontal session tab: bg=background.base fg=text.muted.
    #[must_use]
    pub fn horizontal_idle(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::Background);
        let title_fg = palette.get(Slot::TextMuted);
        let close_fg = crate::native_tab_rail::tint(
            palette.get(Slot::TextMuted),
            palette.get(Slot::Text),
            0.6,
        );
        TabRowPaint {
            background: bg,
            title_fg,
            close_fg,
            plus_fg: [0, 0, 0, 0],
            new_text_fg: [0, 0, 0, 0],
            title_attr: 0,
            number_fg: [0, 0, 0, 0],
            color: TabCellColor::Idle,
        }
    }

    /// Hovered horizontal session tab: bg=raised.high fg=text.base.
    #[must_use]
    pub fn horizontal_hovered(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::BackgroundElement);
        let title_fg = palette.get(Slot::Text);
        let close_fg = crate::native_tab_rail::tint(
            palette.get(Slot::TextMuted),
            palette.get(Slot::Text),
            0.6,
        );
        TabRowPaint {
            background: bg,
            title_fg,
            close_fg,
            plus_fg: [0, 0, 0, 0],
            new_text_fg: [0, 0, 0, 0],
            title_attr: 0,
            number_fg: [0, 0, 0, 0],
            color: TabCellColor::Hovered,
        }
    }

    /// Selected horizontal session tab: bg=raised.high fg=text.base bold.
    #[must_use]
    pub fn horizontal_selected(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        // selected bg = tint(action.hovered:bg->raised.high, decrease(raised.base), 1) = raised.high
        let bg = palette.get(Slot::BackgroundElement);
        let title_fg = palette.get(Slot::Text);
        let close_fg = crate::native_tab_rail::tint(
            palette.get(Slot::TextMuted),
            palette.get(Slot::Text),
            0.6,
        );
        TabRowPaint {
            background: bg,
            title_fg,
            close_fg,
            plus_fg: [0, 0, 0, 0],
            new_text_fg: [0, 0, 0, 0],
            title_attr: 1,
            number_fg: [0, 0, 0, 0],
            color: TabCellColor::Selected,
        }
    }

    /// Idle horizontal plus inherits background.base and uses text.muted.
    #[must_use]
    pub fn horizontal_add_idle(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::Background);
        let plus_fg = palette.get(Slot::TextMuted);
        let new_text_fg = palette.get(Slot::TextMuted);
        TabRowPaint {
            background: bg,
            title_fg: [0, 0, 0, 0],
            close_fg: [0, 0, 0, 0],
            plus_fg,
            new_text_fg,
            title_attr: 0,
            number_fg: [0, 0, 0, 0],
            color: TabCellColor::AddIdle,
        }
    }

    /// Hovered add-session affordance: bg=raised.high, plus+text=Text.
    #[must_use]
    pub fn horizontal_add_hovered(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::BackgroundElement);
        let text = palette.get(Slot::Text);
        TabRowPaint {
            background: bg,
            title_fg: [0, 0, 0, 0],
            close_fg: [0, 0, 0, 0],
            plus_fg: text,
            new_text_fg: text,
            title_attr: 0,
            number_fg: [0, 0, 0, 0],
            color: TabCellColor::AddHovered,
        }
    }

    /// Promoted Home slot (active new tab): bg=raised.high fg=text.base bold.
    #[must_use]
    pub fn horizontal_home_active(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::BackgroundElement);
        let text = palette.get(Slot::Text);
        TabRowPaint {
            background: bg,
            title_fg: text,
            close_fg: [0, 0, 0, 0],
            plus_fg: [0, 0, 0, 0],
            new_text_fg: [0, 0, 0, 0],
            title_attr: 1,
            number_fg: [0, 0, 0, 0],
            color: TabCellColor::HomeActive,
        }
    }

    /// Promoted Home slot hovered (not active): bg=raised.high fg=text.base.
    #[must_use]
    pub fn horizontal_home_hovered(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::BackgroundElement);
        let text = palette.get(Slot::Text);
        TabRowPaint {
            background: bg,
            title_fg: text,
            close_fg: [0, 0, 0, 0],
            plus_fg: [0, 0, 0, 0],
            new_text_fg: [0, 0, 0, 0],
            title_attr: 0,
            number_fg: [0, 0, 0, 0],
            color: TabCellColor::HomeHovered,
        }
    }
}

/// Classify a horizontal session tab's color role from upstream formulas
/// (session-tabs.tsx :1569-1583 :1609-1612 :1642):
/// - selected -> Selected (bg=raised.high, fg=Text, bold)
/// - hovered && !selected -> Hovered (bg=raised.high, fg=Text)
/// - otherwise -> Idle (bg=background.base, fg=TextMuted)
#[must_use]
pub fn cell_color(_flags: &TabFlags, is_hovered: bool, is_active: bool) -> TabCellColor {
    if is_active {
        TabCellColor::Selected
    } else if is_hovered {
        TabCellColor::Hovered
    } else {
        TabCellColor::Idle
    }
}

/// Full horizontal session-row paint including the number/indicator column
/// color, per upstream session-tabs.tsx:1648-1658 with the recorded theme
/// mappings: runs (busy) and attention feedback numbers use the Primary slot
/// (upstream activeNumber orange.200 / attention accent[200]; accent has no
/// palette slot yet, recorded I16 divergence); hovered -> Text; selected ->
/// tint(Text, Background, 0.25) (selection alpha 1 inside
/// tint(idleNumber(), ..)); idle -> idleNumber().
#[must_use]
pub fn horizontal_session_paint(
    color: TabCellColor,
    flags: &TabFlags,
    palette: &crate::native_theme_runtime::Palette,
) -> TabRowPaint {
    horizontal_session_paint_with_semantics(color, flags, palette, None)
}

/// Semantic V2 status colors, with explicit legacy fallback for custom themes.
#[must_use]
pub fn horizontal_session_paint_with_semantics(
    color: TabCellColor,
    flags: &TabFlags,
    palette: &crate::native_theme_runtime::Palette,
    semantic: Option<&crate::native_theme_runtime::SemanticPalette>,
) -> TabRowPaint {
    use crate::native_theme_runtime::Slot;
    let mut paint = row_paint_for(color, palette);
    paint.number_fg = if flags.busy {
        semantic.map_or_else(|| palette.get(Slot::Primary), |theme| theme.tab_interactive.rgba)
    } else if flags.needs_attention {
        semantic.map_or_else(|| palette.get(Slot::Warning), |theme| theme.tab_attention.rgba)
    } else if color == TabCellColor::Hovered {
        palette.get(Slot::Text)
    } else if color == TabCellColor::Selected {
        crate::native_tab_rail::tint(palette.get(Slot::Text), palette.get(Slot::Background), 0.25)
    } else {
        crate::native_tab_rail::idle_number(palette)
    };
    paint
}

/// Resolve a TabCellColor into a concrete TabRowPaint from the palette.
/// Vertical rail rows (Idle) map to the rail's own CellColor semantics.
#[must_use]
pub fn row_paint_for(
    color: TabCellColor,
    palette: &crate::native_theme_runtime::Palette,
) -> TabRowPaint {
    match color {
        TabCellColor::Idle => TabRowPaint::horizontal_idle(palette),
        TabCellColor::Selected => TabRowPaint::horizontal_selected(palette),
        TabCellColor::Hovered => TabRowPaint::horizontal_hovered(palette),
        TabCellColor::AddIdle => TabRowPaint::horizontal_add_idle(palette),
        TabCellColor::AddHovered => TabRowPaint::horizontal_add_hovered(palette),
        TabCellColor::HomeActive => TabRowPaint::horizontal_home_active(palette),
        TabCellColor::HomeHovered => TabRowPaint::horizontal_home_hovered(palette),
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabStrip {
    /// Painted add-session control, independent of session hit rectangles.
    pub add_rect: Option<TabRect>,
    /// Promoted Home pseudo-tab slot (upstream NEW_SESSION_TAB): a full
    /// adaptive-width member with its own identity, never a session miss.
    pub home_rect: Option<TabRect>,
    /// Composed paint row aligned to `home_rect`.
    pub home_row: Option<String>,
    /// Structural close child of the Home slot (right=1,width=1), which the
    /// upstream also renders for NEW_SESSION_TAB.
    pub home_close_rect: Option<TabRect>,
    /// Pointer-over-Home fact for this frame: gates the close glyph, exactly
    /// like `hovered` gates session close glyphs.
    home_hovered: bool,
    pub orientation: TabOrientation,
    pub available_width: u16,
    pub rows: Vec<String>,
    /// Marker rows follow the session rows in `rows`; their rectangles never
    /// participate in session identity or close hit testing.
    pub overflow_rects: Vec<TabRect>,
    pub rects: Vec<(SessionRef, TabRect)>,
    pub close_rects: Vec<(SessionRef, TabRect)>,
    pub overflow_before: bool,
    pub overflow_after: bool,
    pub scroll: usize,
    /// Session whose upstream close child is currently hovered.  The child is
    /// structural for every tab wide enough to contain `right=1,width=1`, but
    /// it is only a close hit while its parent is hovered.
    hovered: Option<SessionRef>,
    /// Explicit paint projection for the hovered close child. Structural
    /// `close_rects` remain available for hit testing but are not paint commands.
    visible_close_session: Option<SessionRef>,
    /// Per-row color metadata (horizontal only). Empty for vertical rail; the
    /// caller paints uniform fg/bg when empty. Length equals `rows` for the
    /// session rows; home_paint and add_paint cover the promoted Home and the
    /// idle plus control respectively.
    pub row_paints: Vec<TabRowPaint>,
    pub home_paint: Option<TabRowPaint>,
    pub add_paint: Option<TabRowPaint>,
    /// Caller-reported pointer-over-add control fact (horizontal).
    add_hovered: bool,
    /// Whether the promoted Home slot is the active new tab.
    home_active: bool,
    /// Cells composed for the number/indicator column (number_width(items)+1,
    /// clamped by the row composer); 0 for the vertical rail. The caller uses
    /// this to paint the leading cells with TabRowPaint.number_fg (upstream
    /// session-tabs.tsx TabIndicator owns this column's color).
    pub indicator_cells: u16,
    /// Palette-free color facts for the visible horizontal session rows,
    /// aligned one-to-one with the session entries in `rects`/`rows`;
    /// `with_row_colors` resolves them into `row_paints` once a palette is
    /// supplied by the caller.
    row_colors: Vec<TabCellColor>,
    /// Palette-free flag facts per visible session row, aligned with
    /// `row_colors`; they slot the number/indicator column color into the
    /// resolved paints.
    row_flags: Vec<TabFlags>,
    /// Palette-free color fact for the promoted Home slot, if painted.
    home_color: Option<TabCellColor>,
}

impl TabStrip {
    /// Width available to session tabs, excluding the horizontal add control.
    #[must_use]
    pub fn content_width(&self) -> u16 { self.available_width }

    /// Translate both paint and hit rectangles by the same viewport origin.
    #[must_use]
    pub fn translated(mut self, x: u16, y: u16) -> Self {
        for (_, rect) in &mut self.rects {
            rect.x = rect.x.saturating_add(x);
            rect.y = rect.y.saturating_add(y);
        }
        for (_, rect) in &mut self.close_rects {
            rect.x = rect.x.saturating_add(x);
            rect.y = rect.y.saturating_add(y);
        }
        for rect in &mut self.overflow_rects {
            rect.x = rect.x.saturating_add(x);
            rect.y = rect.y.saturating_add(y);
        }
        if let Some(rect) = &mut self.add_rect {
            rect.x = rect.x.saturating_add(x);
            rect.y = rect.y.saturating_add(y);
        }
        // The promoted Home slot is a paint+hit member exactly like add: both
        // its slot rect and its hovered close child must follow the viewport
        // origin or Home becomes unhittable and unpaintable after translation
        // (home_row is metric-aligned text, so only the rects move).
        if let Some(rect) = &mut self.home_rect {
            rect.x = rect.x.saturating_add(x);
            rect.y = rect.y.saturating_add(y);
        }
        if let Some(rect) = &mut self.home_close_rect {
            rect.x = rect.x.saturating_add(x);
            rect.y = rect.y.saturating_add(y);
        }
        self
    }

    /// Build a stable snapshot from the read-only navigation state.
    pub fn from_navigation(nav: &NavigationState, attention: &[Attention]) -> Vec<TabInput> {
        nav.tabs()
            .iter()
            .map(|tab| {
                let mut input = TabInput::from_tab(
                    nav,
                    tab,
                    nav.active()
                        .map(|a| a.session == tab.session)
                        .unwrap_or(false),
                );
                input.flags.needs_attention = attention
                    .iter()
                    .any(|a| a.session == tab.session && a.needs_attention);
                input
            })
            .collect()
    }

    /// Render one horizontal strip or vertical rail. `scroll` is the previous
    /// visible-tab start, matching the upstream adaptive layout's
    /// `previousStart` argument (it is not a pixel/cell scroll offset).
    #[must_use]
    pub fn render(
        inputs: &[TabInput],
        orientation: TabOrientation,
        width: u16,
        height: u16,
        scroll: usize,
    ) -> Self {
        Self::render_with_hover(inputs, orientation, width, height, scroll, None)
    }

    /// Upstream's close child is a separate absolute child: it reserves the
    /// `right: 1; width: 1` cell, but paints `✕` only for the hovered tab.
    #[must_use]
    pub fn render_with_hover(
        inputs: &[TabInput],
        orientation: TabOrientation,
        width: u16,
        height: u16,
        scroll: usize,
        hovered: Option<&SessionRef>,
    ) -> Self {
        Self::render_with_hover_and_add(inputs, orientation, width, height, scroll, hovered, false)
    }

    /// Runtime composition matching the upstream separate new-session control.
    /// The tab budget reserves the button; its position follows the final child.
    #[must_use]
    pub fn render_with_hover_and_add(
        inputs: &[TabInput],
        orientation: TabOrientation,
        width: u16,
        height: u16,
        scroll: usize,
        hovered: Option<&SessionRef>,
        show_add: bool,
    ) -> Self {
        Self::render_with_hover_add_home(inputs, orientation, width, height, scroll, hovered, show_add, false, false)
    }

    /// Full horizontal geometry including the promoted Home slot. `home_hovered`
    /// is the caller's pointer-over-Home fact; the close glyph paints only then,
    /// exactly like the per-tab close children. Placing Home in the layout is
    /// source-backed: upstream appends NEW_SESSION_TAB to items() and lets
    /// adaptiveSessionTabLayout + activeID() (= NEW_SESSION_TAB) solve it as a
    /// full adaptive-width member.
    #[must_use]
    pub fn render_with_hover_add_home(
        inputs: &[TabInput],
        orientation: TabOrientation,
        width: u16,
        height: u16,
        scroll: usize,
        hovered: Option<&SessionRef>,
        mut show_add: bool,
        home_active: bool,
        home_hovered: bool,
    ) -> Self {
        let mut out = Self {
            orientation,
            available_width: width,
            add_rect: None,
            home_rect: None,
            home_row: None,
            home_close_rect: None,
            rows: Vec::new(),
            home_hovered: false,
            overflow_rects: Vec::new(),
            rects: Vec::new(),
            close_rects: Vec::new(),
            overflow_before: false,
            overflow_after: false,
            scroll: 0,
            hovered: hovered.cloned(),
            visible_close_session: None,
            row_paints: Vec::new(),
            home_paint: None,
            add_paint: None,
            add_hovered: false,
            home_active: false,
            row_colors: Vec::new(),
            row_flags: Vec::new(),
            home_color: None,
            indicator_cells: 0,
        };
        if width == 0 || height == 0 {
            return out;
        }
        let horizontal = matches!(orientation, TabOrientation::Horizontal);
        // Upstream showPlus() = Boolean(tabs.add) && !newTab(): the idle plus
        // and the promoted Home slot are mutually exclusive states of one
        // control. The vertical rail keeps its own New row untouched.
        let home_active = home_active && horizontal;
        out.home_active = home_active;
        if home_active {
            show_add = false;
        }
        let items_len = inputs.len() + usize::from(home_active);
        let available = if horizontal && show_add {
            width.saturating_sub(TabReservation::ADD_TAB_WIDTH)
        } else {
            match orientation {
                TabOrientation::Horizontal => width,
                TabOrientation::Vertical => height,
            }
        };
        let layout_active = if home_active {
            items_len.checked_sub(1)
        } else {
            inputs.iter().position(|tab| tab.flags.active)
        };
        let solved = if horizontal {
            adaptive_layout(items_len, layout_active, usize::from(available), scroll)
        } else {
            let start = scroll.min(inputs.len());
            let count = inputs.len().saturating_sub(start).min(usize::from(height));
            AdaptiveLayout {
                start,
                count,
                before: start,
                after: inputs.len().saturating_sub(start + count),
                widths: vec![usize::from(width); count],
            }
        };
        out.scroll = solved.start;
        out.overflow_before = solved.before > 0;
        out.overflow_after = solved.after > 0;
        let home_visible = home_active && solved.start + solved.count > inputs.len();
        let visible_sessions =
            (solved.count - usize::from(home_visible)).min(inputs.len().saturating_sub(solved.start));
        let before_cells = if solved.before > 0 {
            overflow_width(solved.before)
        } else {
            0
        };
        // Keep tiny viewports in bounds the same way the box layout does upstream.
        let marker_left = before_cells.min(usize::from(available));
        let mut x_or_y = marker_left as u16;
        let gauge = if horizontal {
            usize::from(number_width(items_len)) + 1
        } else {
            0
        };
        out.indicator_cells = if horizontal { gauge as u16 } else { 0 };
        if horizontal {
            for (position, input) in inputs[solved.start..solved.start + visible_sessions].iter().enumerate() {
                let slot_width = solved.widths[position] as u16;
                let slot = TabRect { x: x_or_y, y: 0, width: slot_width, height: height.min(1) };
                let close = if slot_width >= 2 {
                    Some(TabRect {
                        x: slot.x + slot_width - 2,
                        y: slot.y,
                        width: 1,
                        height: slot.height,
                    })
                } else {
                    None
                };
                out.rows.push(horizontal_row(input, slot_width, gauge, hovered == Some(&input.session)));
                out.rects.push((input.session.clone(), slot));
                if let Some(close) = close {
                    out.close_rects.push((input.session.clone(), close));
                }
                if horizontal {
                    // Palette-free fact first; concrete paints resolve in
                    // with_row_colors so every render entry point stays
                    // palette-free and existing callers/tests are unchanged.
                    let color = cell_color(
                        &input.flags,
                        hovered == Some(&input.session),
                        input.flags.active,
                    );
                    out.row_colors.push(color);
                    out.row_flags.push(input.flags);
                }
                x_or_y = x_or_y.saturating_add(slot_width);
            }
            if home_visible {
                // The promoted Home slot is the final entry of the solved window.
                let slot_width = solved.widths[solved.count - 1] as u16;
                let slot = TabRect { x: x_or_y, y: 0, width: slot_width, height: height.min(1) };
                out.home_row = Some(horizontal_row_home(slot_width, gauge, home_hovered));
                // home_visible implies home_active; upstream checks selected
                // before hovered (session-tabs.tsx:766-779), so the slot
                // resolves with the active-new style in with_row_colors.
                out.home_color = Some(TabCellColor::HomeActive);
                out.home_rect = Some(slot);
                if slot_width >= 2 {
                    out.home_close_rect = Some(TabRect {
                        x: slot.x + slot_width - 2,
                        y: 0,
                        width: 1,
                        height: slot.height,
                    });
                }
                x_or_y = x_or_y.saturating_add(slot_width);
            }
        } else {
            for input in inputs[solved.start..solved.start + solved.count].iter() {
                let label = format_label(input);
                let rect = TabRect { x: 0, y: x_or_y, width, height: 1 };
                let close = if width >= 2 {
                    Some(TabRect { x: width - 2, y: rect.y, width: 1, height: 1 })
                } else {
                    None
                };
                let label_width = if close.is_some() { width - 2 } else { width };
                let rendered = if close.is_some() {
                    let mut row = fit(&label, label_width);
                    row.push_str(if hovered == Some(&input.session) { "\u{2715} " } else { "  " });
                    row
                } else {
                    fit(&label, width)
                };
                out.rows.push(rendered);
                out.rects.push((input.session.clone(), rect));
                if let Some(close) = close {
                    out.close_rects.push((input.session.clone(), close));
                }
                x_or_y = x_or_y.saturating_add(1);
            }
        }
        if horizontal {
            let after_cells = if solved.after > 0 { overflow_width(solved.after) } else { 0 };
            for (count, x, cells_width, label) in [
                (solved.before, 0u16, marker_left as u16, format!("\u{2039}{}", solved.before)),
                (
                    solved.after,
                    x_or_y,
                    after_cells.min(usize::from(available.saturating_sub(x_or_y))) as u16,
                    format!(" {}\u{203a}", solved.after),
                ),
            ] {
                if count > 0 && cells_width > 0 {
                    out.rows.push(fit(&label, cells_width));
                    out.overflow_rects.push(TabRect { x, y: 0, width: cells_width, height: 1 });
                }
            }
        }
        if horizontal && show_add {
            let x = usize::from(x_or_y)
                .saturating_add(if solved.after > 0 { overflow_width(solved.after) } else { 0 })
                .min(usize::from(available)) as u16;
            let add_width = TabReservation::ADD_TAB_WIDTH.min(width.saturating_sub(x));
            out.add_rect = Some(TabRect {
                x,
                y: 0,
                width: add_width,
                height: 1,
            });
            // add_paint resolves in with_row_colors from the add_hovered
            // fact; the fact defaults to explicit false in the constructor
            // and is only changed via the with_add_hovered builder.
        }
        out.home_hovered = home_hovered && out.home_rect.is_some();
        out.visible_close_session = out
            .hovered
            .as_ref()
            .filter(|session| out.close_rects.iter().any(|(id, _)| id == *session))
            .cloned();
        out
    }

    /// The Home close child rectangle stays structural for hit testing; glyph
    /// visibility is the caller's pointer-over-Home fact (home_hovered), the
    /// same gate session close children keep via `hovered`.
    #[must_use]
    pub fn home_close_visible_rect(&self) -> Option<TabRect> {
        self.home_close_rect.filter(|_| self.home_hovered)
    }

    #[must_use]
    pub fn home_close_hit(&self, x: u16, y: u16) -> bool {
        self.home_close_rect.is_some_and(|rect| rect.contains(x, y))
    }

    #[must_use]
    pub fn home_hit(&self, x: u16, y: u16) -> bool {
        self.home_rect.is_some_and(|rect| rect.contains(x, y))
    }


    #[must_use]
    pub fn visible_close_rect(&self) -> Option<TabRect> {
        let session = self.visible_close_session.as_ref()?;
        self.close_rects
            .iter()
            .find(|(id, _)| id == session)
            .map(|(_, rect)| *rect)
    }

    /// The session whose close glyph is actually painted in this frame.
    pub fn hovered_session(&self) -> Option<&SessionRef> {
        self.visible_close_session.as_ref()
    }

    #[must_use]
    pub fn hit(&self, x: u16, y: u16) -> Option<TabHit> {
        for (session, rect) in &self.rects {
            if rect.contains(x, y) {
                let close = self.hovered.as_ref() == Some(session)
                    && self
                        .close_rects
                        .iter()
                        .any(|(id, r)| id == session && r.contains(x, y));
                return Some(TabHit {
                    session: session.clone(),
                    close,
                });
            }
        }
        None
    }

    /// Resolve a mouse click into a tab action. Returns the stable session
    /// identity of the clicked tab and whether the close child was hit.
    ///
    /// This is the actual mouse-to-action boundary used by the live TUI loop.
    /// It never returns a row index or a stale position. The `hovered` state
    /// passed into `render_with_hover` gates whether a close child is a close
    /// hit, mirroring upstream's `hovered() !== tab.sessionID` guard in
    /// session-tabs.tsx.
    #[must_use]
    pub fn mouse_action(&self, x: u16, y: u16) -> Option<TabAction> {
        for (session, rect) in &self.rects {
            if rect.contains(x, y) {
                let close = self.hovered.as_ref() == Some(session)
                    && self
                        .close_rects
                        .iter()
                        .any(|(id, r)| id == session && r.contains(x, y));
                return Some(TabAction {
                    session: session.clone(),
                    close,
                });
            }
        }
        None
    }

    /// Hit test the dedicated horizontal add-session control.
    pub fn add_hit(&self, x: u16, y: u16) -> bool {
        self.add_rect.is_some_and(|rect| rect.contains(x, y))
    }

    /// Resolve horizontal targets explicitly. Session identity comes from the
    /// painted rect map only; Home and the idle plus are separate targets; a
    /// miss is None and never falls back to the active session.
    #[must_use]
    pub fn horizontal_target(&self, x: u16, y: u16) -> Option<HorizontalTarget> {
        if let Some((id, rect)) = self.rects.iter().find(|(_, rect)| rect.contains(x, y)) {
            return Some(HorizontalTarget::Session(id.clone()));
        }
        if self.home_hit(x, y) {
            return Some(HorizontalTarget::Home);
        }
        if self.add_hit(x, y) {
            return Some(HorizontalTarget::NewControl);
        }
        None
    }
}

impl TabStrip {
    /// Resolve stored palette-free color facts into concrete paint metadata
    /// (`row_paints`, `home_paint`, `add_paint`) using `palette`. Pure
    /// builder: geometry, identity and hit rectangles are untouched, and
    /// renders that skip this builder keep empty/default metadata.
    #[must_use]
    pub fn with_row_colors(mut self, palette: &crate::native_theme_runtime::Palette) -> Self {
        self.with_row_colors_semantic(palette, None)
    }

    #[must_use]
    pub fn with_row_colors_semantic(
        mut self,
        palette: &crate::native_theme_runtime::Palette,
        semantic: Option<&crate::native_theme_runtime::SemanticPalette>,
    ) -> Self {
        self.row_paints = self
            .row_colors
            .iter()
            .zip(self.row_flags.iter())
            .map(|(color, flags)| horizontal_session_paint_with_semantics(*color, flags, palette, semantic))
            .collect();
        self.home_paint = self.home_color.map(|color| row_paint_for(color, palette));
        self.add_paint = self.add_rect.map(|_| {
            if self.add_hovered {
                TabRowPaint::horizontal_add_hovered(palette)
            } else {
                TabRowPaint::horizontal_add_idle(palette)
            }
        });
        self
    }

    /// Record the caller's pointer-over-add fact (explicit default `false`)
    /// and refresh the add control paint to the matching hovered/idle style.
    #[must_use]
    pub fn with_add_hovered(
        mut self,
        hovered: bool,
        palette: &crate::native_theme_runtime::Palette,
    ) -> Self {
        self.add_hovered = hovered;
        if self.add_rect.is_some() {
            self.add_paint = Some(if hovered {
                TabRowPaint::horizontal_add_hovered(palette)
            } else {
                TabRowPaint::horizontal_add_idle(palette)
            });
        }
        self
    }

    /// Caller-reported pointer-over-add fact for this frame.
    #[must_use]
    pub fn add_hovered(&self) -> bool {
        self.add_hovered
    }

    /// Whether the promoted Home slot is the active new tab in this frame.
    #[must_use]
    pub fn home_active(&self) -> bool {
        self.home_active
    }
}

struct AdaptiveLayout {
    start: usize,
    count: usize,
    before: usize,
    after: usize,
    widths: Vec<usize>,
}

fn overflow_width(count: usize) -> usize {
    count.to_string().len() + 2
}

/// Direct port of `adaptiveSessionTabLayout` from the pinned upstream
/// `packages/tui/src/context/session-tabs-model.ts:178-260`.
fn adaptive_layout(
    len: usize,
    active: Option<usize>,
    available: usize,
    previous_start: usize,
) -> AdaptiveLayout {
    const NORMAL: usize = 22;
    const MAX: usize = 32;
    const MIN: usize = 8;
    if len == 0 {
        return AdaptiveLayout {
            start: 0,
            count: 0,
            before: 0,
            after: 0,
            widths: Vec::new(),
        };
    }
    let fit = |width: usize| -> usize {
        std::cmp::min(
            len,
            std::cmp::max(
                1,
                match active {
                    None => width / MIN,
                    Some(_) => 1 + (width.saturating_sub(NORMAL) / MIN),
                },
            ),
        )
    };
    let mut count = fit(available);
    let mut start = previous_start.min(len - count);
    for _ in 0..4 {
        let bounded = start.min(len - count);
        let next_start = match active {
            None => bounded,
            Some(index) if index < bounded => index,
            Some(index) if index >= bounded + count => index - count + 1,
            Some(_) => bounded,
        }
        .min(len - count);
        let before = next_start;
        let after = len - next_start - count;
        let markers = if before > 0 {
            overflow_width(before)
        } else {
            0
        } + if after > 0 { overflow_width(after) } else { 0 };
        let next_count = fit(available.saturating_sub(markers));
        start = next_start;
        if next_count == count {
            break;
        }
        count = next_count;
    }
    let before = start;
    let after = len - start - count;
    let content = std::cmp::max(
        1,
        available
            .saturating_sub(if before > 0 {
                overflow_width(before)
            } else {
                0
            })
            .saturating_sub(if after > 0 { overflow_width(after) } else { 0 }),
    );
    let roomy = content >= NORMAL * count;
    let total = if roomy {
        std::cmp::min(content, MAX * count)
    } else {
        content
    };
    let widths = if roomy || active.is_none() {
        let each = total / count;
        let remainder = total - each * count;
        (0..count)
            .map(|i| each + usize::from(i < remainder))
            .collect()
    } else {
        let inactive = if count == 1 {
            0
        } else {
            std::cmp::min(
                NORMAL,
                std::cmp::max(MIN, (total - std::cmp::min(NORMAL, total)) / (count - 1)),
            )
        };
        let active_width = if count == 1 {
            total
        } else {
            total - inactive * (count - 1)
        };
        (start..start + count)
            .map(|i| {
                if Some(i) == active {
                    active_width
                } else {
                    inactive
                }
            })
            .collect()
    };
    AdaptiveLayout {
        start,
        count,
        before,
        after,
        widths,
    }
}

/// Upstream `config.tabs.indicators` defaults to "status"
/// (packages/tui/src/config/index.tsx:336), so the number column paints the
/// static status glyph: attention permission "!"; question "?" (kind not
/// representable in canonical Attention facts yet); busy ->
/// TAB_SPINNERS.dots frames[0] (session-tabs.tsx:57-60, spinner-frames.ts:1);
/// the promoted Home tab always labels "+"; everything else idleLabel "".
/// Unread markers need an unread fact the canonical model does not carry
/// (TAB_UNREAD_MARKERS["small-dot"] = "\u{2022}"); logged as a known gap.
#[must_use]
pub fn indicator_label(flags: &TabFlags, is_home: bool) -> &'static str {
    if is_home {
        return "+";
    }
    if flags.needs_attention {
        return "!";
    }
    if flags.busy {
        return "\u{280b}";
    }
    ""
}

/// `Math.max(2, String(items().length).length)` for the number column.
#[must_use]
pub fn number_width(items: usize) -> u16 {
    2u16.max(items.to_string().len() as u16)
}

fn format_label(input: &TabInput) -> String {
    // Upstream status renders in its own indicator column (TabIndicator) with
    // separate colors instead of mutating the title text. Titles stay pure;
    // the horizontal composer paints status via indicator_label().
    input.title.clone()
}

/// One horizontal session row: number/indicator column (numberWidth+1 cells
/// with the status glyph right-aligned before a 1-cell pad, TabIndicator
/// box width = numberWidth + 1, justifyContent flex-end), then the title
/// region spanning to the strip edge; the hovered close child paints its
/// glyph over the title's second-to-last cell (position absolute right=1
/// width=1), shrinking the title by two as upstream hoveredTitleWidth does.
fn horizontal_row(input: &TabInput, slot_width: u16, gauge: usize, hovered: bool) -> String {
    let glyph = indicator_label(&input.flags, false);
    horizontal_composed_row(glyph, &input.title, slot_width, gauge, hovered)
}

/// The promoted Home row: "+" indicator glyph and the upstream
/// NEW_SESSION_TAB_TITLE text, with the same gauge/title/close geometry.
fn horizontal_row_home(slot_width: u16, gauge: usize, hovered: bool) -> String {
    horizontal_composed_row("+", TabReservation::HOME_TITLE, slot_width, gauge, hovered)
}

fn horizontal_composed_row(glyph: &str, title: &str, slot_width: u16, gauge: usize, hovered: bool) -> String {
    let w = usize::from(slot_width);
    // The gauge never eats the whole slot: at least one title cell remains and
    // the close cell stays overlay-able, mirroring upstream MIN width 8.
    let col = gauge.min(w.saturating_sub(2)).max(if w >= 3 { 1 } else { w });
    if hovered && w >= col + 2 {
        // title cells shrink by two; close glyph lands at w-2; final cell pads.
        let title_cells = w - col - 2;
        let mut row = format_indicator_column(glyph, col);
        row.push_str(&fit(title, title_cells as u16));
        row.push_str("\u{2715} ");
        row
    } else {
        let title_cells = w - col;
        let mut row = format_indicator_column(glyph, col);
        row.push_str(&fit(title, title_cells as u16));
        row
    }
}

fn format_indicator_column(glyph: &str, col: usize) -> String {
    if col == 0 || glyph.is_empty() {
        return " ".repeat(col);
    }
    let glyph_cells = display_width(glyph);
    let cell_count = glyph_cells.min(col.saturating_sub(1));
    if cell_count == 0 {
        // A 1-cell indicator column cannot hold glyph AND pad without breaking
        // display_width(row) == slot width (review D1, upstream TabIndicator
        // keeps its own column width). Emit spaces so the composed row
        // conserves the slot; the glyph reappears as the slot widens.
        return " ".repeat(col);
    }
    let mut row = " ".repeat(col - cell_count - 1);
    row.push_str(glyph);
    row.push(' ');
    row
}

fn display_width(s: &str) -> usize {
    visit_unpadded_cells(s, 4096, |_, _| {}).min(4096)
}

#[cfg(test)]
mod tests {
    use super::*;
    #[test]
    fn horizontal_new_follows_short_or_empty_tab_list_without_right_anchoring() {
        for count in 0..=2 {
            let inputs: Vec<_> = (0..count).map(|id| input(&id.to_string(), "tab")).collect();
            let strip = TabStrip::render_with_hover_and_add(&inputs, TabOrientation::Horizontal, 80, 1, 0, None, true);
            let after_tabs = strip.rects.last().map_or(0, |(_, rect)| rect.x + rect.width);
            let add = strip.add_rect.unwrap();
            assert_eq!(add.x, after_tabs, "New is the next flex child, not pinned to the screen edge");
            assert!(strip.add_hit(add.x + 1, 0));
            assert!(strip.mouse_action(add.x + 1, 0).is_none());
        }
    }

    #[test]
    fn horizontal_overflow_labels_are_paint_rows_not_session_targets() {
        let tabs = vec![input("a", "first"), input("b", "second"), input("c", "third")];
        let strip = TabStrip::render_with_hover_and_add(&tabs, TabOrientation::Horizontal, 12, 1, 1, None, true);
        assert!(strip.overflow_before && strip.overflow_after);
        assert!(strip.rows.iter().any(|row| row.trim_end() == "‹1"), "before count must be painted");
        assert!(strip.rows.iter().any(|row| row.trim_end() == " 1›"), "after count must be painted");
        assert!(strip.hit(0, 0).is_none());
        assert!(strip.hit(6, 0).is_none());
        assert_eq!(strip.add_rect.unwrap().x, 9);
    }
    fn input(id: &str, title: &str) -> TabInput {
        TabInput {
            session: SessionRef(id.into()),
            title: title.into(),
            flags: TabFlags::default(),
        }
    }

    #[test]
    fn horizontal_add_control_is_reserved_and_identity_free() {
        let strip = TabStrip::render_with_hover_and_add(&[input("a", "alpha")], TabOrientation::Horizontal, 20, 1, 0, None, true);
        let add = strip.add_rect.expect("horizontal add control");
        assert_eq!(add, TabRect { x: 17, y: 0, width: 3, height: 1 });
        assert!(strip.add_hit(18, 0));
        assert!(strip.hit(18, 0).is_none());
    }

    #[test]
    fn unicode_labels_use_cells_and_close_hit_is_identity_stable() {
        let tabs = vec![input("a", "界面"), input("b", "second")];
        let strip = TabStrip::render_with_hover(
            &tabs,
            TabOrientation::Horizontal,
            14,
            2,
            0,
            Some(&SessionRef("a".into())),
        );
        // adaptiveSessionTabLayout(available=14, no active, 2 tabs) solves a
        // one-tab window. The right overflow marker reserves 3 cells, so the
        // visible tab receives 11 cells and tab-plus-marker conserves 14.
        assert_eq!(strip.rects[0].1.width, 11);
        assert!(!strip.overflow_before);
        assert!(strip.overflow_after);
        assert_eq!(overflow_width(1), 3);
        assert_eq!(
            display_width(&strip.rows[0]),
            usize::from(strip.rects[0].1.width)
        );
        assert_eq!(display_width(&strip.rows[0]) + overflow_width(1), 14);
        let r = strip.close_rects[0].1;
        assert_eq!(r.x, strip.rects[0].1.x + strip.rects[0].1.width - 2);
        assert_eq!(
            strip.hit(r.x, r.y),
            Some(TabHit {
                session: SessionRef("a".into()),
                close: true
            })
        );
        // Actual mouse-to-action: clicking the close child of a hovered tab
        // returns the stable session identity with close=true. The clicked ID
        // must equal the hovered session, never a row index.
        assert_eq!(
            strip.mouse_action(r.x, r.y),
            Some(TabAction {
                session: SessionRef("a".into()),
                close: true
            })
        );
        // A click on the tab body (not the close child) is a selection, not
        // a close — even while hovered.
        let body = strip.rects[0].1;
        assert_eq!(
            strip.mouse_action(body.x, body.y).map(|a| a.close),
            Some(false)
        );
    }

    #[test]
    fn runtime_add_control_is_visible_and_non_overlapping_for_all_tab_counts() {
        for count in [0usize, 1, 2, 8, 32] {
            let inputs: Vec<_> = (0..count).map(|i| input(&format!("id-{i}"), "tab")).collect();
            let strip = TabStrip::render_with_hover_and_add(&inputs, TabOrientation::Horizontal, 80, 1, 0, None, true);
            let add = strip.add_rect.expect("runtime plus is always visible");
            let x = match count { 0 => 0, 1 => 32, 2 => 64, _ => 77 };
            assert_eq!(add, TabRect { x, y: 0, width: 3, height: 1 },
                "New follows capped tab widths and overflow markers");
            assert!(strip.rects.iter().all(|(_, rect)| rect.x + rect.width <= add.x));
        }
        for width in 1..=2 {
            let strip = TabStrip::render_with_hover_and_add(&[], TabOrientation::Horizontal, width, 1, 0, None, true);
            let add = strip.add_rect.expect("bounded tiny plus");
            assert_eq!(add.width, width);
            assert!(strip.add_hit(width - 1, 0));
        }
    }

    #[test]
    fn add_control_has_non_overlapping_geometry_for_single_or_empty_strip() {
        for tabs in [Vec::new(), vec![input("a", "alpha")]] {
            let strip = TabStrip::render_with_hover_and_add(&tabs, TabOrientation::Horizontal, 20, 1, 0, None, true);
            let add = strip.add_rect.expect("horizontal add control");
            assert!(add.x + add.width <= 20);
            assert!(strip.rects.iter().all(|(_, rect)| rect.x + rect.width <= add.x));
        }
    }

    #[test]
    fn mouse_action_returns_clicked_session_not_active_row() {
        let tabs = vec![input("a", "alpha"), input("b", "beta"), input("c", "gamma")];
        let strip = TabStrip::render(&tabs, TabOrientation::Horizontal, 9, 1, 1);
        assert!(strip.overflow_before && strip.overflow_after);
        assert_eq!(strip.scroll, 1);
        assert_eq!(strip.rects[0].1.x, 3); // left marker: sessionTabOverflowWidth(1)
        assert!(strip.hit(0, 0).is_none());
        assert_eq!(
            strip.hit(strip.rects[0].1.x, 0).unwrap().session,
            SessionRef("b".into())
        );
    }

    #[test]
    fn orientation_preserves_session_ids() {
        let tabs = vec![input("first", "one"), input("second", "two")];
        let h = TabStrip::render(&tabs, TabOrientation::Horizontal, 20, 2, 0);
        let v = TabStrip::render(&tabs, TabOrientation::Vertical, 20, 2, 0);
        assert_eq!(
            h.rects.iter().map(|x| &x.0).collect::<Vec<_>>(),
            v.rects.iter().map(|x| &x.0).collect::<Vec<_>>()
        );
    }

    #[test]
    fn canonical_clusters_strip_zwj_flags_combining_and_ansi() {
        let mut tab = input("stable", "👩‍💻🇺🇸e\u{1ab0}\x1b[31mX\x1b[0m");
        tab.flags = TabFlags {
            active: true,
            busy: true,
            needs_attention: true,
        };
        let strip = TabStrip::render(&[tab], TabOrientation::Horizontal, 8, 1, 0);
        assert_eq!(strip.rows[0].chars().filter(|c| *c == '\x1b').count(), 0);
        assert_eq!(
            strip.hit(0, 0).unwrap().session,
            SessionRef("stable".into())
        );
        assert!(
            !strip.overflow_after,
            "title clipping does not change the solved tab window"
        );
        assert_eq!(
            display_width(&strip.rows[0]),
            usize::from(strip.rects[0].1.width)
        );
    }

    #[test]
    fn tiny_viewports_only_hit_painted_close_cells() {
        let tabs = vec![input("id", "title")];
        for width in 0..=6 {
            let strip = TabStrip::render(&tabs, TabOrientation::Horizontal, width, 1, 0);
            assert!(strip
                .close_rects
                .iter()
                .all(|(_, r)| r.width > 0 && r.x < width));
            if let Some((_, close)) = strip.close_rects.first() {
                assert_eq!(strip.hit(close.x, close.y).map(|h| h.close), Some(false));
            }
        }
        let vertical = TabStrip::render(&tabs, TabOrientation::Vertical, 1, 1, 0);
        assert!(vertical.close_rects.is_empty());
        assert_eq!(vertical.hit(0, 0).unwrap().close, false);
    }

    #[test]
    fn flags_are_visible_in_same_stable_id_model() {
        let tab = TabInput {
            session: SessionRef("busy".into()),
            title: "x".into(),
            flags: TabFlags {
                active: true,
                busy: true,
                needs_attention: true,
            },
        };
        let strip = TabStrip::render(&[tab], TabOrientation::Vertical, 6, 1, 0);
        assert_eq!(strip.rects[0].0, SessionRef("busy".into()));
        assert_eq!(display_width(&strip.rows[0]), 6);
        assert!(strip.rows[0].starts_with("x "));
        assert_eq!(&strip.rows[0][strip.rows[0].len() - 2..], "  ");
        assert_eq!(strip.close_rects[0].1.x, 4);
    }

    #[test]
    fn title_close_glyphs_never_become_close_hits() {
        for title in ["×leading", "middle×title", "many××marks"] {
            let strip = TabStrip::render_with_hover(
                &[input("stable", title)],
                TabOrientation::Horizontal,
                40,
                1,
                0,
                Some(&SessionRef("stable".into())),
            );
            let close = strip.close_rects[0].1;
            assert_eq!(strip.hit(close.x, close.y).unwrap().close, true);
            assert_eq!(
                strip.hit(close.x.saturating_sub(1), close.y).unwrap().close,
                false
            );
            assert!(strip.rows[0].contains('✕'));
        }
    }

    #[test]
    fn clipped_titles_have_only_structural_visible_close_control() {
        for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
            for width in 0..=6 {
                let strip = TabStrip::render_with_hover(
                    &[input("id", "a very long × title")],
                    orientation,
                    width,
                    1,
                    0,
                    Some(&SessionRef("id".into())),
                );
                for (id, close) in &strip.close_rects {
                    assert_eq!(id, &SessionRef("id".into()));
                    assert!(close.width > 0 && close.x < width);
                    assert_eq!(strip.hit(close.x, close.y).unwrap().close, true);
                    assert_eq!(close.x, strip.rects[0].1.x + strip.rects[0].1.width - 2);
                }
                if let Some((_, rect)) = strip.rects.first() {
                    assert_eq!(display_width(&strip.rows[0]), usize::from(rect.width));
                }
                if strip.close_rects.is_empty() {
                    assert!(strip.hit(0, 0).is_none() || !strip.hit(0, 0).unwrap().close);
                }
            }
        }
    }

    #[test]
    fn close_is_hover_only_and_title_multiplication_sign_is_not_close() {
        let id = SessionRef("id".into());
        let plain = TabStrip::render(
            &[input("id", "many× title")],
            TabOrientation::Horizontal,
            20,
            1,
            0,
        );
        assert_eq!(plain.close_rects.len(), 1);
        assert!(!plain.rows[0].contains('✕'));
        assert_eq!(plain.hit(plain.close_rects[0].1.x, 0).unwrap().close, false);
        let hovered = TabStrip::render_with_hover(
            &[input("id", "many× title")],
            TabOrientation::Horizontal,
            20,
            1,
            0,
            Some(&id),
        );
        assert_eq!(hovered.close_rects.len(), 1);
        assert_eq!(
            hovered.hit(hovered.close_rects[0].1.x, 0).unwrap().session,
            id
        );
        assert!(hovered.rows[0].contains('✕'));
    }

    #[test]
    fn visible_close_projection_is_hover_gated_while_hit_rect_stays_structural() {
        let first = SessionRef("first".into());
        let second = SessionRef("second".into());
        let inputs = [input("first", "one"), input("second", "two")];
        let plain = TabStrip::render(&inputs, TabOrientation::Horizontal, 40, 1, 0);
        assert!(plain.visible_close_rect().is_none());
        assert_eq!(plain.close_rects.len(), 2);
        let hovered = TabStrip::render_with_hover(
            &inputs, TabOrientation::Horizontal, 40, 1, 0, Some(&second),
        );
        let visible = hovered.visible_close_rect().expect("hovered close is painted");
        assert_eq!(visible, hovered.close_rects[1].1);
        assert_eq!(hovered.hit(hovered.close_rects[0].1.x, 0).unwrap().close, false);
        assert_eq!(hovered.hit(visible.x, visible.y).unwrap().close, true);
        assert_eq!(first, hovered.rects[0].0);
    }

    #[test]
    fn reservation_fit_boundary_and_translated_hit_coordinates() {
        let vertical = TabReservation::for_view(TabOrientation::Vertical, 106, 20);
        assert_eq!((vertical.width, vertical.height), (42, 20));
        let fallback = TabReservation::for_view(TabOrientation::Vertical, 105, 20);
        assert_eq!(fallback.orientation, Some(TabOrientation::Horizontal));
        assert_eq!((fallback.width, fallback.height), (105, 1));

        let item = input("session", "chat");
        let local = TabStrip::render(&[item], TabOrientation::Horizontal, 40, 1, 0);
        let rect = local.rects[0].1;
        let translated = local.translated(3, 2);
        assert!(translated.hit(rect.x, rect.y).is_none());
        assert_eq!(translated.hit(rect.x + 3, rect.y + 2).unwrap().session,
            SessionRef("session".into()));
    }


    #[test]
    fn translated_rects_are_terminal_absolute_and_input_does_not_double_translate() {
        let id = SessionRef("clicked".into());
        let local = TabStrip::render_with_hover(
            &[input("clicked", "title")], TabOrientation::Horizontal, 20, 1, 0, Some(&id));
        let absolute = local.translated(7, 3);
        let close = absolute.close_rects[0].1;
        assert_eq!(absolute.mouse_action(close.x, close.y).unwrap().session, id);
        assert!(absolute.mouse_action(close.x.saturating_sub(7), close.y.saturating_sub(3)).is_none());
        let rail = TabStrip::render(&[input("rail", "rail")], TabOrientation::Vertical, 42, 4, 0).translated(5, 2);
        assert_eq!(rail.hit(5, 2).unwrap().session, SessionRef("rail".into()));
        assert!(rail.hit(0, 0).is_none());
    }

    #[test]
    fn zero_extent_has_no_layout_or_hits() {
        let tabs = [input("id", "title")];
        for orientation in [TabOrientation::Horizontal, TabOrientation::Vertical] {
            for (width, height) in [(0, 1), (1, 0), (0, 0)] {
                let strip = TabStrip::render_with_hover(&tabs, orientation, width, height, 0, None);
                assert!(
                    strip.rows.is_empty() && strip.rects.is_empty() && strip.close_rects.is_empty()
                );
                assert!(strip.hit(0, 0).is_none());
            }
        }
    }

    #[test]
    fn adaptive_layout_matches_upstream_allocation_vectors() {
        let cases = [
            // (available, active index, previous start, start, count, before, after, widths)
            (9, None, 0, 0, 1, 0, 2, vec![6]),
            (9, None, 1, 1, 1, 1, 1, vec![3]),
            (14, None, 0, 0, 1, 0, 2, vec![11]),
            (14, None, 1, 1, 1, 1, 1, vec![8]),
            (40, None, 0, 0, 3, 0, 0, vec![14, 13, 13]),
            (9, Some(0), 0, 0, 1, 0, 2, vec![6]),
            (9, Some(1), 0, 1, 1, 1, 1, vec![3]),
            (9, Some(2), 0, 2, 1, 2, 0, vec![6]),
            (40, Some(0), 0, 0, 3, 0, 0, vec![22, 9, 9]),
            (40, Some(1), 0, 0, 3, 0, 0, vec![9, 22, 9]),
            (40, Some(2), 0, 0, 3, 0, 0, vec![9, 9, 22]),
        ];
        for (available, active, previous, start, count, before, after, widths) in cases {
            let got = adaptive_layout(3, active, available, previous);
            assert_eq!(
                (got.start, got.count, got.before, got.after, got.widths),
                (start, count, before, after, widths),
                "available={available} active={active:?} previous={previous}"
            );
        }
    }

    #[test]
    fn adaptive_rows_conserve_allocated_cells_and_markers_are_not_hits() {
        let tabs = vec![input("a", "alpha"), input("b", "beta"), input("c", "gamma")];
        for (width, previous) in [(9, 1), (14, 1), (40, 0)] {
            let strip = TabStrip::render(&tabs, TabOrientation::Horizontal, width, 1, previous);
            for (index, (_, rect)) in strip.rects.iter().enumerate() {
                assert_eq!(display_width(&strip.rows[index]), usize::from(rect.width));
                assert_eq!(
                    strip.hit(rect.x, rect.y).unwrap().session,
                    strip.rects[index].0
                );
            }
            if strip.overflow_before {
                assert!(strip.hit(0, 0).is_none());
            }
        }
    }
    #[test]
    fn horizontal_title_is_pure_and_status_goes_to_the_indicator_column() {
        let mut tab = input("a", "Draft");
        tab.flags = TabFlags { active: true, busy: true, needs_attention: true };
        assert_eq!(format_label(&tab), "Draft");
        assert_eq!(indicator_label(&tab.flags, false), "!");
        tab.flags.needs_attention = false;
        assert_eq!(indicator_label(&tab.flags, false), "\u{280b}", "busy paints the dots spinner frame 0");
        tab.flags.busy = false;
        assert_eq!(indicator_label(&tab.flags, false), "", "idle status column is blank in status mode");
        assert_eq!(indicator_label(&tab.flags, true), "+", "promoted Home always labels +");
        assert_eq!(number_width(9), 2);
        assert_eq!(number_width(10), 2);
        assert_eq!(number_width(123), 3);
    }

    #[test]
    fn home_slot_is_a_full_adaptive_member_with_plus_and_title() {
        // items = 2 sessions + Home(active, last): adaptive_layout(3, Some(2), 40)
        // solves [9, 9, 22] with the active (Home) slot last, per the upstream
        // allocation exercise in adaptive_layout_matches_upstream_allocation_vectors.
        let tabs = vec![input("a", "alpha"), input("b", "beta")];
        let strip =
            TabStrip::render_with_hover_add_home(&tabs, TabOrientation::Horizontal, 40, 1, 0, None, true, true, false);
        let home = strip.home_rect.expect("promoted Home slot");
        assert_eq!(strip.add_rect, None, "plus and promoted Home are mutually exclusive");
        assert_eq!((home.x, home.width), (18, 22), "adaptive widths [9,9,22] with Home last and active");
        assert_eq!(strip.rects.len(), 2, "session identity map stays session-only");
        let row = strip.home_row.as_ref().expect("home paint row");
        assert!(row.contains('+'), "indicator glyph + right-aligned in the number column");
        assert!(row.contains("New session"), "upstream NEW_SESSION_TAB_TITLE paints as the title text");
        assert!(!row.contains('\u{2715}'), "no close glyph without home_hovered");
        assert_eq!(display_width(&row), usize::from(home.width), "row fills exactly the home slot");
        assert!(row.starts_with(" + "), "gauge layout: pad, plus, pad");
        assert_eq!(strip.home_close_rect.map(|r| (r.x, r.width)), Some((home.x + home.width - 2, 1)));
        assert_eq!(strip.horizontal_target(home.x + 1, 0), Some(HorizontalTarget::Home));
        assert_eq!(strip.horizontal_target(home.x + home.width - 2, 0), Some(HorizontalTarget::Home));
        assert_eq!(
            middle_action(&strip.horizontal_target(home.x, 0).unwrap()),
            HorizontalMiddleAction::CloseHomeNavigatePrevious
        );
        assert_eq!(strip.horizontal_target(39, 0), Some(HorizontalTarget::Home)); // last in-slot cell
        assert_eq!(strip.horizontal_target(40, 0), None, "first cell past the slot is a miss");
    }

    #[test]
    fn home_close_glyph_tracks_home_hover_and_hit_geometry() {
        let tabs = vec![input("a", "alpha")];
        let strip =
            TabStrip::render_with_hover_add_home(&tabs, TabOrientation::Horizontal, 40, 1, 0, None, true, true, true);
        let home = strip.home_rect.expect("home slot");
        let close = strip.home_close_visible_rect().expect("hovered home paints its close child");
        assert_eq!(close, strip.home_close_rect.unwrap());
        assert_eq!(close.x, home.x + home.width - 2);
        assert!(strip.home_close_hit(close.x, 0));
        assert!(strip.home_row.as_ref().expect("row").contains('\u{2715}'));
        // Only the hovered slot shows a close glyph; the session row stays clean.
        assert!(!strip.rows[0].contains('\u{2715}'), "session row without hover has no close glyph");
        let (ref id, rect) = strip.rects[0];
        let (_, crect) = strip.close_rects[0];
        assert_eq!(crect.x, rect.x + rect.width - 2);
        assert_eq!(strip.hit(crect.x, 0).map(|h| h.session.clone()), Some(id.clone()));
    }

    #[test]
    fn idle_plus_control_keeps_upstream_plus_geometry_when_not_on_home() {
        let strip =
            TabStrip::render_with_hover_and_add(&[input("a", "alpha")], TabOrientation::Horizontal, 20, 1, 0, None, true);
        let add = strip.add_rect.expect("idle plus exists off Home");
        assert!(strip.add_hit(add.x + 1, 0));
        assert_eq!(strip.horizontal_target(add.x + 1, 0), Some(HorizontalTarget::NewControl));
        assert_eq!(middle_action(&HorizontalTarget::NewControl), HorizontalMiddleAction::Inert);
        assert!(strip.home_rect.is_none());
    }

    #[test]
    fn translated_strip_keeps_home_identity_and_close_geometry() {
        let tabs = vec![input("a", "alpha")];
        let strip =
            TabStrip::render_with_hover_add_home(&tabs, TabOrientation::Horizontal, 40, 1, 0, None, true, true, false);
        let (hx, hcx) = (strip.home_rect.unwrap(), strip.home_close_rect.unwrap());
        let shifted = strip.translated(7, 3);
        let shome = shifted.home_rect.unwrap();
        assert_eq!((shome.x, shome.y), (hx.x + 7, hx.y + 3));
        assert_eq!(shifted.horizontal_target(hx.x + 7, 3), Some(HorizontalTarget::Home));
        assert_eq!(shifted.home_close_rect.unwrap().x, hcx.x + 7);
    }

    #[test]
    fn home_projection_is_half_open_and_translated_hits_match_paint() {
        let tabs = vec![input("a", "alpha")];
        let strip =
            TabStrip::render_with_hover_add_home(&tabs, TabOrientation::Horizontal, 40, 1, 0, None, true, true, true);
        let home = strip.home_rect.expect("promoted Home slot");
        // Upstream slot of width 22 covers half-open cells 18 through 39:
        // final cell is a Home hit and the first cell past the slot is a miss.
        assert_eq!(
            strip.horizontal_target(home.x + home.width - 1, 0),
            Some(HorizontalTarget::Home),
            "last slot cell is inside the Home geometry"
        );
        assert_eq!(strip.horizontal_target(home.x + home.width, 0), None, "first cell past the slot is a miss");
        // Paint and hit projections stay coincident after translation.
        let shifted = strip.translated(5, 2);
        let shome = shifted.home_rect.expect("translated Home slot");
        assert_eq!((shome.x, shome.y), (home.x + 5, home.y + 2));
        assert_eq!(
            shifted.horizontal_target(home.x + home.width - 1, 2),
            Some(HorizontalTarget::Home)
        );
        assert_eq!(shifted.horizontal_target(home.x + home.width + 5, 2), None);
        let sclose = shifted.home_close_visible_rect().expect("translated hovered close child");
        assert_eq!(sclose.x, home.x + home.width - 2 + 5);
        assert!(shifted.home_close_hit(sclose.x, 2), "close child stays hittable at its painted cell");
    }
    #[test]
    fn narrow_indicator_slots_conserve_row_width_for_busy_attention_and_home() {
        // All fixtures compose through the production row writer with
        // col == 1 and a non-empty 1-cell status glyph (review defect D1
        // regime); conservation binds the ALLOCATED geometry, never the
        // viewport parameter.
        let mut busy = input("b", "job");
        busy.flags = TabFlags { active: false, busy: true, needs_attention: false };
        let mut attention = input("attn", "perm");
        attention.flags = TabFlags { active: false, busy: false, needs_attention: true };

        // (A) PRODUCTION allocated-3 rows: upstream adaptiveSessionTabLayout
        // vector (9, active=None, scroll=1) -> widths [3], pinned in
        // adaptive_layout_matches_upstream_allocation_vectors. The middle tab
        // is the visible one and carries the status flag.
        for tab in [&busy, &attention] {
            let tabs = vec![input("a", "perm"), tab.clone(), input("c", "third")];
            let strip = TabStrip::render_with_hover_add_home(
                &tabs, TabOrientation::Horizontal, 9, 1, 1, None, false, false, false,
            );
            let rect = strip
                .rects
                .iter()
                .find(|(id, _)| id == &tab.session)
                .expect("production narrow slot")
                .1;
            assert_eq!((rect.x, rect.width), (3, 3), "upstream (9,None,1): 3-cell marker + 3-cell slot");
            assert!(strip.overflow_before && strip.overflow_after);
            // D1: the whole composed row fills exactly the ALLOCATED slot even
            // when the status glyph ("!" or busy dots frame 0) shares the
            // 1-cell indicator column with its mandatory pad.
            assert_eq!(
                display_width(&strip.rows[0]),
                usize::from(rect.width),
                "status row conserves the allocated 3-cell slot for glyph {:?}",
                indicator_label(&tab.flags, false)
            );
            assert_eq!(
                display_width(&strip.rows[0]) + overflow_width(1) + overflow_width(1),
                9,
                "full strip conservation: marker + row + marker"
            );
        }

        // Optional strength add (review 5D): hovered allocated-3 row is the
        // extra RED anchor (pre-clamp it composes 4 cells).
        {
            let tabs = vec![input("a", "perm"), busy.clone(), input("c", "third")];
            let hovered = Some(&busy.session);
            let strip = TabStrip::render_with_hover_add_home(
                &tabs, TabOrientation::Horizontal, 9, 1, 1, hovered, false, false, false,
            );
            let rect = strip.rects.iter().find(|(id, _)| id == &busy.session).expect("hovered narrow slot").1;
            assert_eq!((rect.x, rect.width), (3, 3));
            assert_eq!(
                display_width(&strip.rows[0]),
                usize::from(rect.width),
                "hovered status row conserves the allocated 3-cell slot"
            );
            assert_eq!(
                strip.rows[0].chars().nth(usize::from(rect.width) - 2),
                Some('\u{2715}'),
                "close glyph lands at rect.width-2 on the hovered narrow row"
            );
        }

        // (B) DEGENERATE allocated-1 exercise (explicit fixture label): the 3
        // cell plus reservation consumes the whole 3-cell viewport
        // (available = 0 -> the solver's 1-cell floor). The label component is
        // degenerate, not the app. Bind AND pin the allocation.
        for tab in [&busy, &attention] {
            let strip = TabStrip::render_with_hover_add_home(
                std::slice::from_ref(tab),
                TabOrientation::Horizontal,
                3,
                1,
                0,
                None,
                true,
                false,
                false,
            );
            let rect = strip.rects.iter().find(|(id, _)| id == &tab.session).expect("floor slot").1;
            assert_eq!(rect.width, 1, "fixture: plus reservation leaves the solver's 1-cell floor");
            assert_eq!(
                display_width(&strip.rows[0]),
                usize::from(rect.width),
                "conservation at allocated 1 for glyph {:?}",
                indicator_label(&tab.flags, false)
            );
        }

        // (C) PROMOTED HOME on the empty strip: home_active suppresses the
        // plus, so viewport == slot IS the production allocation at 1 and 3.
        // Geometry pins stay; conservation binds home.width.
        for width in [1u16, 3u16] {
            let home_strip = TabStrip::render_with_hover_add_home(
                &[],
                TabOrientation::Horizontal,
                width,
                1,
                0,
                None,
                true,
                true,
                false,
            );
            let home = home_strip.home_rect.expect("promoted Home slot");
            assert_eq!(home.width, width, "empty-strip Home pivot allocates the full viewport");
            let row = home_strip.home_row.as_ref().expect("home paint row");
            assert_eq!(display_width(row), usize::from(home.width), "Home row conserves its allocated slot at {width}");
        }
    }

    // -- Additive horizontal paint-metadata fixtures (I16) ----------------

    #[test]
    fn row_paint_metadata_resolves_distinct_idle_hovered_selected_states() {
        use crate::native_theme_runtime::{Palette, Slot};
        let palette = Palette::opencode_dark();
        let mut hovered_tab = input("hov", "hovered");
        hovered_tab.flags = TabFlags { active: false, busy: false, needs_attention: false };
        let mut selected_tab = input("sel", "selected");
        selected_tab.flags = TabFlags { active: true, busy: false, needs_attention: false };
        let strip = TabStrip::render_with_hover(
            &[input("idle", "idle"), hovered_tab, selected_tab],
            TabOrientation::Horizontal,
            120,
            1,
            0,
            Some(&SessionRef("hov".into())),
        )
        .with_row_colors(&palette);
        assert_eq!(strip.row_paints.len(), 3, "one paint fact per visible session row");
        assert_eq!(strip.indicator_cells, 3, "number_width(3 sessions)+1");
        assert_eq!(strip.row_paints[0].color, TabCellColor::Idle);
        assert_eq!(strip.row_paints[1].color, TabCellColor::Hovered);
        assert_eq!(strip.row_paints[2].color, TabCellColor::Selected);
        // Upstream session-tabs.tsx:1569-1583 with the recorded theme mapping:
        // idle bg=background.base fg=text.muted; hovered/selected bg=raised.high
        // fg=text.base (selected bold).
        assert_eq!(strip.row_paints[0].background, palette.get(Slot::Background));
        assert_eq!(strip.row_paints[0].title_fg, palette.get(Slot::TextMuted));
        assert_eq!(strip.row_paints[2].background, palette.get(Slot::BackgroundElement));
        assert_eq!(strip.row_paints[2].title_fg, palette.get(Slot::Text));
        assert_eq!(strip.row_paints[2].title_attr, 1);
        assert_eq!(strip.row_paints[1].title_attr, 0, "hovered alone is not bold");
        // Close glyph fg = tint(text.muted, text.base, 0.6), upstream :1731-1736.
        let close = crate::native_tab_rail::tint(
            palette.get(Slot::TextMuted),
            palette.get(Slot::Text),
            0.6,
        );
        assert_eq!(strip.row_paints[1].close_fg, close);
        // Number/indicator column colors, upstream session-tabs.tsx:1648-1658.
        assert_eq!(
            strip.row_paints[0].number_fg,
            crate::native_tab_rail::idle_number(&palette),
            "idle number is idleNumber()"
        );
        assert_eq!(
            strip.row_paints[2].number_fg,
            crate::native_tab_rail::tint(
                palette.get(Slot::Text),
                palette.get(Slot::Background),
                0.25,
            ),
            "selected number is the selection-tinted inner color"
        );
        // Legacy palette-only API keeps explicit fallback roles; semantic V2
        // callers use the focused vector below to distinguish hues.
        let mut busy_tab = input("b", "job");
        busy_tab.flags = TabFlags { active: false, busy: true, needs_attention: false };
        let mut attention_tab = input("attn", "perm");
        attention_tab.flags = TabFlags { active: false, busy: false, needs_attention: true };
        let status = TabStrip::render_with_hover(
            &[busy_tab, attention_tab],
            TabOrientation::Horizontal,
            80,
            1,
            0,
            None,
        )
        .with_row_colors(&palette);
        assert_eq!(status.row_paints[0].number_fg, palette.get(Slot::Primary));
        assert_eq!(status.row_paints[1].number_fg, palette.get(Slot::Warning));
    }

    #[test]
    fn semantic_status_vectors_distinguish_busy_interactive_and_attention_accent() {
        use crate::native_theme_runtime::{Palette, RuntimeTheme, ThemeMode};
        let theme = RuntimeTheme::builtin_default(ThemeMode::Dark);
        let palette = Palette::opencode_dark();
        let mut busy = input("busy", "job");
        busy.flags.busy = true;
        let mut attention = input("attention", "permission");
        attention.flags.needs_attention = true;
        let strip = TabStrip::render_with_hover(&[busy, attention], TabOrientation::Horizontal, 80, 1, 0, None)
            .with_row_colors_semantic(&palette, theme.semantic.as_ref());
        let semantic = theme.semantic.as_ref().expect("built-in V2 semantic palette");
        assert_eq!(strip.row_paints[0].number_fg, semantic.tab_interactive.rgba);
        assert_eq!(strip.row_paints[1].number_fg, semantic.tab_attention.rgba);
        assert_ne!(strip.row_paints[0].number_fg, palette.get(crate::native_theme_runtime::Slot::Warning));
    }

    #[test]
    fn paint_metadata_is_empty_until_with_row_colors_is_applied() {
        use crate::native_theme_runtime::Palette;
        let palette = Palette::opencode_dark();
        let strip = TabStrip::render(&[input("a", "alpha")], TabOrientation::Horizontal, 40, 1, 0);
        assert!(
            strip.row_paints.is_empty() && strip.home_paint.is_none() && strip.add_paint.is_none(),
            "default render entry points stay metadata-free and byte-compatible"
        );
        // Vertical rows keep their rail-owned painting; TabStrip metadata and
        // the add-hover fact stay off for the vertical rail.
        let vertical = TabStrip::render(&[input("a", "alpha")], TabOrientation::Vertical, 20, 4, 0)
            .with_row_colors(&palette);
        assert!(vertical.row_paints.is_empty(), "vertical rows paint via the rail");
        assert!(vertical.home_paint.is_none() && vertical.add_paint.is_none());
        assert!(!vertical.add_hovered(), "add-hover fact defaults to false");
        assert_eq!(vertical.indicator_cells, 0, "vertical rows paint via the rail");
    }

    #[test]
    fn home_and_add_paint_metadata_follow_upstream_styles() {
        use crate::native_theme_runtime::{Palette, Slot};
        let palette = Palette::opencode_dark();
        // Promoted Home slot always resolves HomeActive when visible.
        let home_strip = TabStrip::render_with_hover_add_home(
            &[input("a", "alpha")],
            TabOrientation::Horizontal,
            40,
            1,
            0,
            None,
            true,
            true,
            false,
        )
        .with_row_colors(&palette);
        let home_paint = home_strip.home_paint.expect("home paint metadata");
        assert_eq!(home_paint.color, TabCellColor::HomeActive);
        assert_eq!(home_paint.background, palette.get(Slot::BackgroundElement));
        assert_eq!(home_paint.title_fg, palette.get(Slot::Text));
        assert_eq!(home_paint.title_attr, 1, "active new tab is bold");

        // Horizontal plus inherits base background and text.muted
        // (upstream session-tabs.tsx:1766-1770); vertical numbering is separate.
        let add_strip = TabStrip::render_with_hover_and_add(
            &[input("a", "alpha")],
            TabOrientation::Horizontal,
            20,
            1,
            0,
            None,
            true,
        )
        .with_row_colors(&palette);
        let add_paint = add_strip.add_paint.expect("add paint metadata");
        assert_eq!(add_paint.color, TabCellColor::AddIdle);
        assert_eq!(add_paint.plus_fg, palette.get(Slot::TextMuted));
        assert_eq!(add_paint.background, palette.get(Slot::Background));
        assert_eq!(add_paint.new_text_fg, palette.get(Slot::TextMuted));
        assert!(!add_strip.add_hovered());

        // Hovered add control: bg=raised.high and both fg=Text.
        let add_hover = add_strip.with_add_hovered(true, &palette);
        assert!(add_hover.add_hovered());
        let paint = add_hover.add_paint.expect("add paint after builder");
        assert_eq!(paint.color, TabCellColor::AddHovered);
        assert_eq!(paint.background, palette.get(Slot::BackgroundElement));
        assert_eq!(paint.plus_fg, palette.get(Slot::Text));
        assert_eq!(paint.new_text_fg, palette.get(Slot::Text));

        // Disabling the fact again restores the idle style.
        let back = add_hover.with_add_hovered(false, &palette);
        assert_eq!(
            back.add_paint.expect("idle restore").color,
            TabCellColor::AddIdle
        );
    }
}
