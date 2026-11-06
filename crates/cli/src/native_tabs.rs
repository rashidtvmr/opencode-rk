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
    /// Attention kind for the indicator column: `Permission` paints `!`,
    /// `Question` paints `?` (`component/session-tabs.tsx:172-174`). `None`
    /// when the tab carries no dialog attention.
    pub attention: Option<TabAttention>,
    /// Activity-unread fact from the live `NavigationState`: a background
    /// completion the route has not acknowledged. Idle status mode paints the
    /// `TAB_UNREAD_MARKERS["small-dot"]` glyph (`session-tabs.tsx:63-70,176-178`)
    /// in the accent `unreadColor` (`:125-129,565`). Error-unread has no native
    /// producer yet (TAB-21).
    pub unread: bool,
    /// TAB-07: error-unread fact (`status.unread === "error"`,
    /// `context/session-tabs.tsx:160-162`): an errored background turn paints
    /// its unread indicator `theme.text.feedback.error.base` instead of the
    /// accent (`session-tabs.tsx:125-130`). Fed from
    /// `NavigationState::is_unread_failed`; only meaningful alongside `unread`.
    pub unread_error: bool,
    /// Marquee window start for the hovered title (0 paints the plain title).
    /// The strip applies it with the exact title budget its layout reserves.
    pub marquee_offset: usize,
    /// TAB-04: `config.tabs.indicators` (`config/index.tsx:199`, default
    /// `"status"`). `Numbers` keeps the numeric label in the indicator column
    /// instead of the status glyph (`session-tabs.tsx:525`). Defaults to
    /// `Status`, so a caller that never sets it keeps the status column.
    pub indicators: TabIndicators,
    /// The tab's 1-based number (`sessionTabNumberLabel(index)`,
    /// `context/session-tabs-model.ts:10-12`), painted in `Numbers` mode.
    /// Empty when the producer did not assign one; `Numbers` then paints the
    /// empty column rather than fabricating a number.
    pub number: String,
    /// TAB-06: the live `promptPulse` feed (`context/session-tabs.tsx:316-329`).
    /// Copied from the enqueue pulse map the queue seam feeds; a pulse change
    /// ignites the number cell exactly like `createNumberIgnition`
    /// (`component/session-tabs.tsx:213-227`). Zero keeps the idle number.
    pub prompt_pulse: u64,
    /// TAB-04: the live spinner frame for a busy tab. `None` (the default)
    /// keeps the static `TAB_SPINNERS.dots.frames[0]` glyph `⠋`; `Some(n)`
    /// paints `TAB_SPINNER_DOTS[n % len]`. The live loop feeds
    /// `now_ms / 80`, the upstream `<spinner interval={80}>` cadence
    /// (`component/session-tabs.tsx:57-58,198`), and leaves it `None` when
    /// `config.animations` is off so reduced motion stays on frame 0.
    pub spinner_frame: Option<u64>,
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
            attention: None,
            session: tab.session.clone(),
            unread: false,
            unread_error: false,
            marquee_offset: 0,
            indicators: TabIndicators::Status,
            number: String::new(),
            prompt_pulse: 0,
            spinner_frame: None,
        }
    }

    /// TAB-04: apply the live `config.tabs.indicators` selection to every tab.
    #[must_use]
    pub fn with_indicators(mut self, indicators: TabIndicators) -> Self {
        self.indicators = indicators;
        self
    }
}

/// Walk a session's `parent` links among the currently open tabs. Returns true
/// when `session` is a descendant of `ancestor`. Cycle-safe.
///
/// Port of the family walk behind `context/session-tabs.tsx:127-131` +
/// `packages/client/src/solid/data.ts:545-555` (`resolveRoot`), restricted to
/// the parent links the native navigation retains (`Tab.parent`, set by
/// `open_forked`). A descendant with no open tab is not reachable here.
fn is_descendant(nav: &NavigationState, session: &SessionRef, ancestor: &SessionRef) -> bool {
    let tabs = nav.tabs();
    let mut current: &SessionRef = session;
    for _ in 0..tabs.len() {
        let Some(tab) = tabs.iter().find(|t| &t.session == current) else {
            return false;
        };
        let Some(parent) = &tab.parent else { return false };
        if parent == ancestor {
            return true;
        }
        current = parent;
    }
    false
}

/// TAB-04: whether any open tab is running, so the live loop keeps its 80 ms
/// repaint cadence while a busy-tab spinner animates. Mirrors the family
/// roll-up in [`TabStrip::from_navigation`]: a tab is running when its own
/// session is running or any open descendant is (`context/session-tabs.tsx:
/// 171-176`). A busy tab that is not the active session still needs the tick,
/// so the loop cannot rely on the active worker alone.
#[must_use]
pub fn any_running(nav: &NavigationState) -> bool {
    let running = |session: &SessionRef| nav.work_state(session) == Some(WorkState::Running);
    nav.tabs().iter().any(|tab| {
        running(&tab.session)
            || nav.tabs().iter().any(|candidate| {
                is_descendant(nav, &candidate.session, &tab.session) && running(&candidate.session)
            })
    })
}

/// Visible-cluster fade window for an overflowing title, a port of
/// `session-tabs.tsx:230-236` `fadeTitleColor`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub struct TitleFade {
    /// Cell offset of the first title cluster inside the composed row.
    pub start: u16,
    /// Number of visible title clusters (`visibleTitleParts().length`).
    pub length: u16,
    /// The leading edge ramps only while the marquee is running.
    pub leading: bool,
}

impl TitleFade {
    /// Opacity toward the background for visible cluster `index`
    /// (0 keeps the text color, 1 dissolves it).
    #[must_use]
    pub fn weight(&self, index: u16) -> f64 {
        f64::from(title_fade_weight(
            usize::from(index),
            usize::from(self.length),
            if self.leading { 1.0 } else { 0.0 },
        ))
    }

    /// Apply the fade to one cell color.
    #[must_use]
    pub fn color(&self, index: u16, base: [u8; 4], background: [u8; 4]) -> [u8; 4] {
        let weight = self.weight(index);
        if weight <= 0.0 {
            base
        } else {
            crate::native_tab_rail::tint(base, background, weight)
        }
    }
}

/// Caller-owned attention facts, kept separate from daemon work state.
///
/// `needs_attention` preserves the legacy bool contract (a tab marked
/// attention is `true`); `kind` carries which dialog gates it, because the
/// indicator column paints `!` for a permission and `?` for a question
/// (`component/session-tabs.tsx:172-174`).
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Attention {
    pub session: SessionRef,
    pub needs_attention: bool,
    pub kind: TabAttention,
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
    /// Hovered (not selected, not dragged): bg=raised.base fg=text.base
    /// (`session-tabs.tsx:1568-1573` lifts `background.action.primary.hovered`,
    /// which both bundled themes pin to `$hue.neutral.700` == `raised.base`).
    Hovered,
    /// Dragged (not selected, pointer no longer over the source): bg=raised.high
    /// fg=text.base bold. Upstream lifts `action.primary.hovered` toward
    /// `decrease(raised.base)` by 1, which is exactly `raised.high`, and forces
    /// BOLD (`session-tabs.tsx:1568-1573,1642`). Its number color is NOT
    /// `text.base`: upstream `numberColor` (`:1632-1641`) only takes
    /// `foreground()` when `hovered() && !selected()`, so a dragged-but-not-
    /// hovered tab keeps the idle number formula.
    Dragged,
    /// Dragged (not selected) AND still hovered: same bg/bold as [`Self::Dragged`]
    /// but the number takes `foreground()` = `text.base` because
    /// `hovered() && !selected()` holds (`session-tabs.tsx:1611-1614,1632-1641`).
    DraggedHovered,
    /// New-session idle affordance: plus fg=idleNumber, "New session" fg=TextMuted.
    AddIdle,
    /// New-session hovered affordance: bg=raised.base, plus+text fg=text.base.
    AddHovered,
    /// Promoted Home slot (active new tab): bg=raised.high fg=text.base, bold.
    HomeActive,
    /// Promoted Home slot hovered (not active): bg=raised.base fg=text.base.
    HomeHovered,
}

/// Resolved horizontal paint parameters for one tab row, derived from the
/// palette via upstream theme mappings (session-tabs.tsx :1569-1736,
/// :1770-1794). All colors are raw palette `[u8;4]`. Zero-valued channels in
/// unused roles are ignored by the caller.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct TabRowPaint {
    /// Background: idle->background.base, hovered->raised.base,
    /// dragged/selected->raised.high.
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
    /// Color role tag for this row (Idle/Selected/Dragged/Hovered/Add/Home).
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

    /// Hovered horizontal session tab: bg=raised.base fg=text.base. Upstream
    /// `session-tabs.tsx:1568-1573` lifts `theme.background.action.primary.hovered`
    /// with `dragged ? 1 : selection()` = 0 for a plain hover; both bundled
    /// themes define `action.primary.$hovered = $hue.neutral.700`
    /// (`opencode.json:63`, `opencode2.json:65`), which is `raised.base`
    /// (`opencode.json:56`, `opencode2.json:58`), i.e. `Slot::BackgroundPanel`.
    #[must_use]
    pub fn horizontal_hovered(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::BackgroundPanel);
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

    /// Dragged horizontal session tab: bg=raised.high, BOLD. `session-tabs.tsx:1568-1573`:
    /// the dragged tab lifts `action.primary.hovered` toward
    /// `theme.decrease(theme.background.raised.base)` by 1, which is the exact
    /// overlay `decrease(raised.base)` = `raised.high`
    /// (`native_theme_runtime.rs:3245-3257`), i.e. `Slot::BackgroundElement`;
    /// `:1642` forces BOLD while held.
    ///
    /// The title fg follows `foreground()` (`session-tabs.tsx:1609-1612`):
    /// `text.base` only while the pointer is still over the dragged tab, else
    /// `tint(text.muted, text.base, selection())` with `selection() == 0` for a
    /// non-selected tab, i.e. `text.muted`.
    #[must_use]
    pub fn horizontal_dragged(palette: &crate::native_theme_runtime::Palette, hovered: bool) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::BackgroundElement);
        let title_fg = if hovered {
            palette.get(Slot::Text)
        } else {
            palette.get(Slot::TextMuted)
        };
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
            color: TabCellColor::Dragged,
        }
    }

    /// Selected horizontal session tab: bg=raised.high fg=text.base bold.
    #[must_use]
    pub fn horizontal_selected(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        // `session-tabs.tsx:1568-1575,467`: a selected tab is never `lifted`, so
        // bg = tint(background.base, decrease(raised.base), 1). `tint` at alpha 1
        // is the exact overlay, so the result is `decrease(raised.base)` =
        // `raised.high` (neutral.600) in both light and dark.
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

    /// Hovered add-session affordance: bg=raised.base, plus+text=Text.
    #[must_use]
    pub fn horizontal_add_hovered(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::BackgroundPanel);
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

    /// Promoted Home slot hovered (not active): bg=raised.base fg=text.base.
    /// Home is a normal adaptive member of the strip, so its hovered paint is
    /// the same `action.primary.hovered` = `raised.base` as a session tab
    /// (`session-tabs.tsx:1568-1573`).
    #[must_use]
    pub fn horizontal_home_hovered(palette: &crate::native_theme_runtime::Palette) -> Self {
        use crate::native_theme_runtime::Slot;
        let bg = palette.get(Slot::BackgroundPanel);
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
/// - dragged (non-selected) -> Dragged (bg=raised.high, fg=Text, bold), stamped
///   later by [`TabStrip::with_dragged`] from the caller's drag fact
/// - hovered && !selected -> Hovered (bg=raised.base, fg=Text)
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
    // `session-tabs.tsx:958-963,1648-1658` on the non-numbers path resolves
    // the indicator color as `tabFeedbackColor(attention, unread) ?? (runs ?
    // activeNumber : foreground())`. Our `needs_attention` flag carries the
    // attention kind, and a tab can be both busy and attention at once, so
    // attention must be tested before busy or a crying `!`/`?` indicator would
    // keep the running hue instead of the accent. The activity-unread accent
    // override is applied for the row's `unread` fact in
    // `with_row_colors_semantic`. (The `unread == Error` half resolves the
    // failed-outcome tint in `with_row_colors_semantic` via `row_unread_error`,
    // fed by `NavigationState::is_unread_failed` — see TAB-07.)
    paint.number_fg = if flags.needs_attention {
        semantic.map_or_else(|| palette.get(Slot::Warning), |theme| theme.tab_attention.rgba)
    } else if flags.busy {
        semantic.map_or_else(|| palette.get(Slot::Primary), |theme| theme.tab_interactive.rgba)
    } else if color == TabCellColor::DraggedHovered {
        // `session-tabs.tsx:1632-1641`: the number takes `foreground()` =
        // text.base only when `hovered() && !selected()`. A dragged tab that is
        // still hovered satisfies that; a dragged tab the pointer has left does
        // not, and falls through to the idle formula below.
        palette.get(Slot::Text)
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
    let mut paint = match color {
        TabCellColor::Idle => TabRowPaint::horizontal_idle(palette),
        TabCellColor::Selected => TabRowPaint::horizontal_selected(palette),
        TabCellColor::Dragged => TabRowPaint::horizontal_dragged(palette, false),
        TabCellColor::DraggedHovered => TabRowPaint::horizontal_dragged(palette, true),
        TabCellColor::Hovered => TabRowPaint::horizontal_hovered(palette),
        TabCellColor::AddIdle => TabRowPaint::horizontal_add_idle(palette),
        TabCellColor::AddHovered => TabRowPaint::horizontal_add_hovered(palette),
        TabCellColor::HomeActive => TabRowPaint::horizontal_home_active(palette),
        TabCellColor::HomeHovered => TabRowPaint::horizontal_home_hovered(palette),
    };
    // The role tag must reflect the actual classification, not the shared
    // constructor's default: `Dragged` and `DraggedHovered` reuse the same
    // dragged paint (same bg/bold) but differ in the number color, so the tag
    // distinguishes them for consumers and tests.
    paint.color = color;
    paint
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
    /// Count of tabs hidden by the leading overflow marker (`layout().before`).
    /// Together with the painted `rects` it lets a pointer column resolve to an
    /// absolute slot exactly as upstream `slotAt` does (`session-tabs.tsx:1490-1500`).
    pub before: usize,
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
    /// Activity-unread fact per visible session row, aligned with `row_colors`
    /// and `row_flags` (horizontal only). `with_row_colors_semantic` swaps the
    /// indicator color to the accent `unreadColor` when the row is unread and
    /// not pending (`session-tabs.tsx:148-169`).
    row_unread: Vec<bool>,
    /// TAB-07: error-unread fact per visible session row, aligned with
    /// `row_unread`. A failed outcome (`context/session-tabs.tsx:160-162`)
    /// swaps the indicator color to `text.feedback.error.base`
    /// (`session-tabs.tsx:125-130`) instead of the accent.
    row_unread_error: Vec<bool>,
    /// Fade window per visible session row, aligned with `rects` (horizontal
    /// only). `None` means every title cell keeps its solid color.
    pub row_fades: Vec<Option<TitleFade>>,
    /// TAB-06: per-row `promptPulse` fact, aligned with `row_colors`
    /// (horizontal only). A nonzero pulse ignites the row's number color in
    /// `with_row_colors_semantic` (`component/session-tabs.tsx:213-227`).
    row_prompt_pulse: Vec<u64>,
    /// Palette-free color fact for the promoted Home slot, if painted.
    home_color: Option<TabCellColor>,
}

impl TabStrip {
    /// Width available to session tabs, excluding the horizontal add control.
    #[must_use]
    pub fn content_width(&self) -> u16 { self.available_width }

    /// Title cell budget this strip's layout gave `session`, for the marquee's
    /// overflow test. `hovered` is the caller's pointer fact, not the strip's
    /// previous frame, because the close child shrinks the title by two.
    /// Only horizontal strips carry per-tab slot rects; the vertical rail
    /// computes its budget from `native_tab_rail::expanded_title_width`.
    #[must_use]
    pub fn title_width(&self, session: &SessionRef, hovered: bool) -> Option<u16> {
        let (_, rect) = self.rects.iter().find(|(id, _)| id == session)?;
        Some(horizontal_title_cells(
            rect.width,
            usize::from(self.indicator_cells),
            hovered,
        ))
    }

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
            .enumerate()
            .map(|(index, tab)| {
                let mut input = TabInput::from_tab(
                    nav,
                    tab,
                    nav.active()
                        .map(|a| a.session == tab.session)
                        .unwrap_or(false),
                );
                // TAB-04: the 1-based tab number painted in `Numbers` mode
                // (`sessionTabNumberLabel(index)` = `String(index + 1)`,
                // `context/session-tabs-model.ts:10-12`). The strip paints the
                // absolute index, not the visible-window position.
                input.number = (index + 1).to_string();
                // `context/session-tabs.tsx:171-176`: any running family member
                // marks the root tab busy. Roll open descendant tabs up so a
                // child's work state reaches its parent tab.
                if !input.flags.busy
                    && nav.tabs().iter().any(|candidate| {
                        is_descendant(nav, &candidate.session, &tab.session)
                            && nav.work_state(&candidate.session) == Some(WorkState::Running)
                    })
                {
                    input.flags.busy = true;
                }
                // `context/session-tabs.tsx:163-171`: attention scans every
                // family member (the root tab and all open descendants), with
                // permission outranking question across the whole family. A
                // descendant's pending permission/question therefore wakes the
                // root tab's attention indicator.
                let mut attention_kind: Option<TabAttention> = None;
                for a in attention.iter().filter(|a| a.needs_attention) {
                    if a.session != tab.session
                        && !is_descendant(nav, &a.session, &tab.session)
                    {
                        continue;
                    }
                    attention_kind = match (attention_kind, a.kind) {
                        (Some(TabAttention::Permission), _) | (_, TabAttention::Permission) => {
                            Some(TabAttention::Permission)
                        }
                        _ => Some(TabAttention::Question),
                    };
                }
                input.flags.needs_attention = attention_kind.is_some();
                input.attention = attention_kind;
                // `context/session-tabs.tsx:139-146`: unread reads the root
                // session only, so a tab's activity-unread fact is the exact
                // `NavigationState` mark for that session. The failed-outcome
                // mark (`:160-162`) rides alongside it.
                input.unread = nav.is_unread(&tab.session);
                input.unread_error = nav.is_unread_failed(&tab.session);
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
        show_add: bool,
        home_active: bool,
        home_hovered: bool,
    ) -> Self {
        Self::render_impl(
            inputs, orientation, width, height, scroll, hovered, show_add, home_active,
            home_hovered, None,
        )
    }

    /// Render the strip while the pointer holds a physical close cell after a
    /// close (`session-tabs.tsx:868-900,1348-1363`): the held layout replaces
    /// the adaptive width solve so the pointer's close cell stays put until
    /// leave, another close, or the 5 s timer. `held` is ignored for the
    /// vertical rail (its layout is stride-based, not width-solved).
    #[must_use]
    pub fn render_with_held(
        inputs: &[TabInput],
        orientation: TabOrientation,
        width: u16,
        height: u16,
        scroll: usize,
        hovered: Option<&SessionRef>,
        show_add: bool,
        home_active: bool,
        home_hovered: bool,
        held: Option<&HeldLayout>,
    ) -> Self {
        Self::render_impl(
            inputs, orientation, width, height, scroll, hovered, show_add, home_active,
            home_hovered, held,
        )
    }

    #[allow(clippy::too_many_arguments)]
    fn render_impl(
        inputs: &[TabInput],
        orientation: TabOrientation,
        width: u16,
        height: u16,
        scroll: usize,
        hovered: Option<&SessionRef>,
        mut show_add: bool,
        home_active: bool,
        home_hovered: bool,
        held: Option<&HeldLayout>,
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
            before: 0,
            hovered: hovered.cloned(),
            visible_close_session: None,
            row_paints: Vec::new(),
            home_paint: None,
            add_paint: None,
            add_hovered: false,
            home_active: false,
            row_colors: Vec::new(),
            row_flags: Vec::new(),
            row_unread: Vec::new(),
            row_unread_error: Vec::new(),
            row_prompt_pulse: Vec::new(),
            row_fades: Vec::new(),
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
            match held {
                // The pointer holds a physical close cell: upstream swaps the
                // whole layout memo for `heldSessionTabLayout`
                // (`session-tabs.tsx:1348-1363`), so the visible window, its
                // widths and both overflow counts come from the held layout.
                Some(held) => AdaptiveLayout {
                    start: held.start,
                    count: held.tabs.len(),
                    before: held.start,
                    after: items_len.saturating_sub(held.start + held.tabs.len()),
                    widths: held.widths.iter().map(|width| usize::from(*width)).collect(),
                },
                None => adaptive_layout(items_len, layout_active, usize::from(available), scroll),
            }
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
        out.before = solved.before;
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
                let (row, fade) = horizontal_row(input, slot_width, gauge, hovered == Some(&input.session));
                out.rows.push(row);
                out.row_fades.push(fade);
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
                    out.row_unread.push(input.unread);
                    out.row_unread_error.push(input.unread_error);
                    out.row_prompt_pulse.push(input.prompt_pulse);
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

    /// Screen column of the strip element's left edge: the first painted tab
    /// minus the leading overflow marker. Upstream `strip.screenX`
    /// (`session-tabs.tsx:1483,1491`); used to convert a pointer column into
    /// [`Self::drag_slot`]/[`CloseHold`] space.
    #[must_use]
    pub fn strip_origin_x(&self) -> u16 {
        let first = self.rects.first().map(|(_, rect)| rect.x).unwrap_or(0);
        let gutter = if self.overflow_before {
            u16::try_from(overflow_width(self.before)).unwrap_or(u16::MAX)
        } else {
            0
        };
        first.saturating_sub(gutter)
    }

    /// Map an absolute pointer column to the absolute slot index of the
    /// painted tab beneath it, exactly as upstream `slotAt`
    /// (`component/session-tabs.tsx:1490-1500`): walk the painted slot edges
    /// (the first rect already begins after the overflow marker) and return
    /// `before + painted_index` for the first edge past `x`, else the last
    /// painted slot. `None` when nothing is painted yet. The caller clamps the
    /// result via [`crate::native_tabs::DragState::drag_to`].
    #[must_use]
    pub fn drag_slot(&self, x: u16, tab_len: usize) -> Option<usize> {
        if self.rects.is_empty() || tab_len == 0 {
            return None;
        }
        let mut edge = self.rects[0].1.x;
        for (index, (_, rect)) in self.rects.iter().enumerate() {
            edge = edge.saturating_add(rect.width);
            if x < edge {
                return Some(self.before + index);
            }
        }
        Some(self.before + self.rects.len() - 1)
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

    /// TAB-24: record the caller's per-session title-pending fact
    /// (`data.session.title.pending`, `context/data.tsx:22-47`) and the
    /// resolved `config.animations ?? true`, then stamp the matching title
    /// attribute onto the resolved paints. A renaming title is DIM only while
    /// animations are disabled; otherwise the selected OR dragged tab keeps
    /// BOLD (`session-tabs.tsx:1061-1074,1642,1713-1720`). The promoted
    /// Home/add rows are left untouched: upstream keeps them BOLD
    /// unconditionally (`session-tabs.tsx:1201,1210`).
    #[must_use]
    pub fn with_title_renaming(
        mut self,
        renaming: &dyn Fn(&SessionRef) -> bool,
        animations: bool,
    ) -> Self {
        // The dragged source is painted BOLD while held (`session-tabs.tsx:1642`),
        // so the lifted flag is active OR Dragged, not just `flags.active`.
        let lifted: Vec<bool> = self
            .row_flags
            .iter()
            .zip(self.row_colors.iter())
            .map(|(flags, color)| {
                flags.active
                    || matches!(*color, TabCellColor::Dragged | TabCellColor::DraggedHovered)
            })
            .collect();
        for (index, paint) in self.row_paints.iter_mut().enumerate() {
            let Some((session, _)) = self.rects.get(index) else { continue };
            let is_lifted = lifted.get(index).copied().unwrap_or(false);
            paint.title_attr = title_attributes(is_lifted, renaming(session), animations);
        }
        self
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
            .zip(self.row_unread.iter())
            .zip(self.row_unread_error.iter())
            .enumerate()
            .map(|(index, (((color, flags), unread), unread_error))| {
                let mut paint =
                    horizontal_session_paint_with_semantics(*color, flags, palette, semantic);
                // TAB-06: a `promptPulse` change ignites the number cell —
                // upstream jumps the ignition tween to 0.85 and eases it back
                // (`component/session-tabs.tsx:213-227,786-792`), tinting the
                // number toward `theme.text.base`. A terminal strip has no
                // animation clock, so one nonzero feed paints the ignition
                // peak; the live feed drains the pulse after the frame.
                if self.row_prompt_pulse.get(index).copied().unwrap_or(0) > 0 {
                    use crate::native_theme_runtime::Slot;
                    paint.number_fg = crate::native_tab_rail::tint(
                        paint.number_fg,
                        palette.get(Slot::Text),
                        0.85,
                    );
                }
                // `TabIndicator` color expression for a non-pending tab prefers
                // `tabFeedbackColor ?? (runs ? activeNumber : foreground())`, but
                // its own `unreadColor` memo (`session-tabs.tsx:148-169`) swaps in
                // the accent `unreadColor` (`:125-129,565`) whenever the tab is
                // unread and not pending. Status mode's idle indicator is blank,
                // so the accent shows through the marker glyph painted in
                // `indicator_label`. A selected tab keeps `theme.text.base`.
                if *unread
                    && !flags.needs_attention
                    && !flags.busy
                    && *color != TabCellColor::Selected
                {
                    use crate::native_theme_runtime::Slot;
                    // TAB-07: a failed outcome swaps the accent for
                    // `theme.text.feedback.error.base` (`session-tabs.tsx:
                    // 125-130`); otherwise the activity accent shows.
                    paint.number_fg = if *unread_error {
                        semantic
                            .map_or_else(|| palette.get(Slot::Error), |theme| theme.text_feedback_error.rgba)
                    } else {
                        semantic.map_or_else(|| palette.get(Slot::Warning), |theme| theme.tab_attention.rgba)
                    };
                }
                paint
            })
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

    /// TAB-07: paint the pointer-held drag source at full selected elevation
    /// (`session-tabs.tsx:1568-1573,1642`): a dragged (non-selected) tab lifts
    /// `action.primary.hovered` toward `decrease(raised.base)` by 1, which is
    /// exactly `raised.high`, and turns BOLD. Recomputes the palette-free
    /// `row_colors` from the drag fact plus the frame `hovered`/`active` facts,
    /// so it is idempotent and frame-local; call it before
    /// `with_row_colors_semantic`/`with_title_renaming`. The selected tab wins
    /// when both apply. The vertical rail owns its own paint (no `row_colors`),
    /// so this is a no-op there — upstream's vertical drag reuses the hovered
    /// style (`session-tabs.tsx:776-777`).
    #[must_use]
    pub fn with_dragged(mut self, dragged: Option<&SessionRef>) -> Self {
        for (index, color) in self.row_colors.iter_mut().enumerate() {
            let Some((session, _)) = self.rects.get(index) else { continue };
            if self.row_flags.get(index).is_some_and(|flags| flags.active) {
                continue;
            }
            let is_hovered = self.hovered.as_ref() == Some(session);
            *color = if dragged == Some(session) {
                // The number color follows `hovered() && !selected()`
                // (`session-tabs.tsx:1632-1641`), independent of the drag fact,
                // so a dragged tab the pointer has left keeps the idle number.
                if is_hovered {
                    TabCellColor::DraggedHovered
                } else {
                    TabCellColor::Dragged
                }
            } else if is_hovered {
                TabCellColor::Hovered
            } else {
                TabCellColor::Idle
            };
        }
        self
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
/// static status glyph: `attention === "permission"` -> "!" and
/// `attention === "question"` -> "?" (`component/session-tabs.tsx:172-174`);
/// `attention` is the caller-supplied kind (permission outranks question).
/// Busy -> TAB_SPINNERS.dots frames[0] (session-tabs.tsx:57-60,
/// spinner-frames.ts:1); the promoted Home tab always labels "+"; an
/// activity-unread idle tab paints TAB_UNREAD_MARKERS["small-dot"] "•"
/// (`session-tabs.tsx:63-70,176-178`); everything else idleLabel "".
#[must_use]
pub fn indicator_label(
    flags: &TabFlags,
    attention: Option<TabAttention>,
    is_home: bool,
    unread: bool,
) -> &'static str {
    indicator_label_frame(flags, attention, is_home, unread, None)
}

/// TAB-04: [`indicator_label`] with the live spinner frame. `spinner_frame`
/// is `None` for reduced motion / no live clock, which keeps the static
/// `TAB_SPINNERS.dots.frames[0]` glyph `⠋`; `Some(n)` paints
/// `TAB_SPINNER_DOTS[n % len]`, matching the upstream
/// `<spinner frames interval={80}>` renderable
/// (`component/session-tabs.tsx:57-58,198`) that replaces the static label
/// whenever `runs() && animations && !numbers`.
#[must_use]
pub fn indicator_label_frame(
    flags: &TabFlags,
    attention: Option<TabAttention>,
    is_home: bool,
    unread: bool,
    spinner_frame: Option<u64>,
) -> &'static str {
    if is_home {
        return "+";
    }
    match attention {
        Some(TabAttention::Permission) => return "!",
        Some(TabAttention::Question) => return "?",
        None => {}
    }
    if flags.needs_attention {
        return "!";
    }
    if flags.busy {
        return spinner_frame_glyph(spinner_frame);
    }
    if unread {
        // `TAB_UNREAD_MARKERS["small-dot"]` (`session-tabs.tsx:63-70`), the
        // default `config.tabs.unread_marker`.
        return "\u{2022}";
    }
    ""
}

/// TAB-04: the busy spinner glyph for one live frame. `None` (reduced motion or
/// no live clock) keeps `TAB_SPINNER_DOTS[0]` = `⠋`; `Some(n)` advances to
/// `TAB_SPINNER_DOTS[n % len]`. Shared by the horizontal strip
/// ([`indicator_label_frame`]) and the vertical rail so both orientations
/// animate from the same frame.
#[must_use]
pub fn spinner_frame_glyph(frame: Option<u64>) -> &'static str {
    match frame {
        Some(n) => TAB_SPINNER_DOTS[(n as usize) % TAB_SPINNER_DOTS.len()],
        None => TAB_SPINNER_DOTS[0],
    }
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
fn horizontal_row(input: &TabInput, slot_width: u16, gauge: usize, hovered: bool) -> (String, Option<TitleFade>) {
    // TAB-04: `config.tabs.indicators`. `Numbers` keeps the numeric label in
    // the indicator column (`session-tabs.tsx:173` `if (props.numbers) return
    // props.label`), so a status change never moves the title start cell.
    // `Status` keeps the existing status-glyph precedence unchanged.
    let status_glyph = indicator_label_frame(
        &input.flags,
        input.attention,
        false,
        input.unread,
        input.spinner_frame,
    );
    let glyph = if input.indicators == TabIndicators::Numbers {
        input.number.as_str()
    } else {
        status_glyph
    };
    horizontal_composed_row(
        glyph,
        &input.title,
        input.marquee_offset,
        slot_width,
        gauge,
        hovered,
    )
}

/// The promoted Home row: "+" indicator glyph and the upstream
/// NEW_SESSION_TAB_TITLE text, with the same gauge/title/close geometry.
fn horizontal_row_home(slot_width: u16, gauge: usize, hovered: bool) -> String {
    horizontal_composed_row("+", TabReservation::HOME_TITLE, 0, slot_width, gauge, hovered).0
}

/// Indicator column width clamped so a slot always keeps a title cell and the
/// close overlay, mirroring upstream MIN width 8.
fn horizontal_indicator_cells(slot_width: u16, gauge: usize) -> usize {
    let w = usize::from(slot_width);
    gauge.min(w.saturating_sub(2)).max(if w >= 3 { 1 } else { w })
}

/// Title cell budget for one horizontal slot. `hovered` reserves the
/// two-cell close child (`✕ `) that `horizontal_composed_row` writes there;
/// the marquee's overflow test must use this exact budget.
fn horizontal_title_cells(slot_width: u16, gauge: usize, hovered: bool) -> u16 {
    let w = usize::from(slot_width);
    let col = horizontal_indicator_cells(slot_width, gauge);
    if hovered && w >= col + 2 {
        (w - col - 2) as u16
    } else {
        w.saturating_sub(col) as u16
    }
}

fn horizontal_composed_row(
    glyph: &str,
    title: &str,
    offset: usize,
    slot_width: u16,
    gauge: usize,
    hovered: bool,
) -> (String, Option<TitleFade>) {
    let w = usize::from(slot_width);
    let col = horizontal_indicator_cells(slot_width, gauge);
    let budget = horizontal_title_cells(slot_width, gauge, hovered);
    let overflows = marquee_overflows(title, budget);
    let displayed = if offset > 0 {
        marquee_text(title, budget, offset)
    } else {
        take_width(title, budget)
    };
    let close = hovered && w >= col + 2;
    let title_cells = if close { w - col - 2 } else { w - col };
    let mut row = format_indicator_column(glyph, col);
    row.push_str(&fit(&displayed, title_cells as u16));
    if close {
        row.push_str("\u{2715} ");
    }
    // `session-tabs.tsx:764-770`: the fade tracks the full title's overflow,
    // so a marquee window keeps the edge ramps of its visible clusters.
    let fade = (overflows && usize::from(budget) > FADE_WIDTH).then(|| TitleFade {
        // The title segment is painted from its own first cluster, so the
        // window starts at cluster 0 of the strip's `displayed` text.
        start: 0,
        length: graphemes(&displayed).len().min(usize::from(u16::MAX)) as u16,
        leading: offset > 0,
    });
    (row, fade)
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

// ---------------------------------------------------------------------------
// Tab indicator/status model.
//
// Upstream `packages/tui/src/component/session-tabs.tsx:132-200` (TabIndicator)
// chooses one glyph from the tab status, and
// `packages/tui/src/context/session-tabs.tsx:143-173` builds that status from
// the root session plus its family. The models below keep those rules pure so
// the paint loop never re-derives glyph precedence.
// ---------------------------------------------------------------------------

/// `packages/tui/src/component/session-tabs.tsx:63` `TAB_SPINNERS.dots.frames`
/// comes from `spinner-frames.ts:1`.
pub const TAB_SPINNER_DOTS: [&str; 10] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠧", "⠇", "⠏"];

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabSpinner {
    Dots,
    Arcs,
    Quadrants,
    Line,
}

impl TabSpinner {
    /// `session-tabs.tsx:63-68`.
    #[must_use]
    pub const fn frames(self) -> &'static [&'static str] {
        match self {
            Self::Dots => &TAB_SPINNER_DOTS,
            Self::Arcs => &["◜", "◝", "◞", "◟"],
            Self::Quadrants => &["◴", "◷", "◶", "◵"],
            Self::Line => &["|", "/", "-", "\\"],
        }
    }

    /// `session-tabs.tsx:63-68` interval in milliseconds.
    #[must_use]
    pub const fn interval_ms(self) -> u64 {
        match self {
            Self::Dots => 80,
            Self::Arcs | Self::Quadrants | Self::Line => 120,
        }
    }
}

/// `session-tabs.tsx:65-70` `TAB_UNREAD_MARKERS`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabUnreadMarker {
    SmallDot,
    Dot,
    Square,
    LargeSquare,
}

impl TabUnreadMarker {
    #[must_use]
    pub const fn glyph(self) -> &'static str {
        match self {
            Self::SmallDot => "•",
            Self::Dot => "●",
            Self::Square => "▪",
            Self::LargeSquare => "■",
        }
    }
}

/// `session-tabs.tsx:88-92` / `config.tabs.indicators`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabIndicators {
    Status,
    Numbers,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabUnread {
    Activity,
    Error,
}

/// Upstream `attention` precedence: permission list first, then forms.
/// `context/session-tabs.tsx:163-167`.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabAttention {
    Permission,
    Question,
}

/// One tab's derived status. Mirrors the object returned by
/// `context/session-tabs.tsx:143-173` plus `promptPulse` from
/// `session-tabs.tsx:316-329` and the `renaming` title state.
#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct TabStatus {
    pub unread: Option<TabUnread>,
    pub prompt_pulse: u64,
    pub attention: Option<TabAttention>,
    pub busy: bool,
    pub renaming: bool,
}

/// `EMPTY_SESSION_TAB_STATUS` at `session-tabs.tsx:83-89`.
pub const EMPTY_TAB_STATUS: TabStatus = TabStatus {
    unread: None,
    prompt_pulse: 0,
    attention: None,
    busy: false,
    renaming: false,
};

impl TabStatus {
    /// `session-tabs.tsx:150` `runs = busy && !attention`.
    #[must_use]
    pub const fn runs(self) -> bool {
        self.busy && self.attention.is_none()
    }

    /// `session-tabs.tsx:151` `pending = busy || Boolean(attention)`.
    #[must_use]
    pub const fn pending(self) -> bool {
        self.busy || self.attention.is_some()
    }

    /// `session-tabs-model.ts:29-31` `sessionTabComplete`.
    #[must_use]
    pub const fn complete(self) -> bool {
        matches!(self.unread, Some(TabUnread::Activity)) && !self.busy
    }

    /// `session-tabs.tsx:153` `unread && !pending`.
    #[must_use]
    pub const fn unread_active(self) -> bool {
        self.unread.is_some() && !self.pending()
    }

    /// `session-tabs.tsx:157-163`: a tab glows when it needs attention, or when
    /// it is not the active tab, is not busy, and carries any unread fact.
    #[must_use]
    pub const fn glows(self, active: bool) -> bool {
        self.attention.is_some() || (!active && !self.busy && self.unread.is_some())
    }

    /// Root/family aggregation from `context/session-tabs.tsx:143-173`: unread
    /// and renaming read the root only; attention scans every family member
    /// with permission outranking question; any member busy marks the root
    /// busy. `root` must not also appear in `members`.
    #[must_use]
    pub fn family(root: Self, members: &[Self]) -> Self {
        let mut attention = root.attention;
        let mut busy = root.busy;
        for member in members {
            attention = match (attention, member.attention) {
                (Some(TabAttention::Permission), _) | (_, Some(TabAttention::Permission)) => {
                    Some(TabAttention::Permission)
                }
                (_, Some(TabAttention::Question)) => Some(TabAttention::Question),
                (current, None) => current,
            };
            if member.busy {
                busy = true;
            }
        }
        Self {
            unread: root.unread,
            prompt_pulse: root.prompt_pulse,
            attention,
            busy,
            renaming: root.renaming,
        }
    }

    /// `context/session-tabs.tsx:355-361`: only a `user` enqueue for another
    /// open tab bumps the pulse. A busy tab still flashes, and tabs that are
    /// not open (or the current route) are ignored.
    pub fn enqueue_user_prompt(&mut self, is_open: bool, is_current: bool) {
        if is_open && !is_current {
            self.prompt_pulse = self.prompt_pulse.saturating_add(1);
        }
    }
}

/// Pending work kinds. `context/session-tabs.tsx:167-171` counts every pending
/// item except `synthetic` as busy; parked synthetic context never wakes a tab.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum PendingKind {
    User,
    Synthetic,
}

#[must_use]
pub fn pending_marks_busy(pending: &[PendingKind]) -> bool {
    pending.iter().any(|kind| *kind != PendingKind::Synthetic)
}

/// Busy composition for a family member: daemon `running` state or any
/// non-synthetic pending item (`context/session-tabs.tsx:167-171`).
#[must_use]
pub fn member_busy(running: bool, pending: &[PendingKind]) -> bool {
    running || pending_marks_busy(pending)
}

/// TAB-06: merge one tab's live status feed into a strip input
/// (`context/session-tabs.tsx:143-176` status()). `status` carries the
/// per-session `promptPulse` from the enqueue map the queue seam feeds;
/// `pending` lists the session's pending items, of which every non-synthetic
/// kind marks the tab busy through `member_busy` (`:167-171`) on top of the
/// daemon running fact `from_navigation` already rolled into `flags.busy`.
/// TAB-05: `status.busy` is the live family roll-up (a running descendant
/// marks the root busy) and ORs into `flags.busy` the same way.
pub fn apply_live_status(
    input: &mut TabInput,
    status: Option<&TabStatus>,
    pending: &[PendingKind],
) {
    let live_busy = status.map_or(false, |status| status.busy);
    if let Some(status) = status {
        input.prompt_pulse = status.prompt_pulse;
    }
    input.flags.busy = member_busy(input.flags.busy || live_busy, pending);
}

/// TAB-06: the live queue-seam enqueue feed. `context/session-tabs.tsx:334-338`
/// subscribes to `session.inbox.enqueued` for `user` items, skips the current
/// route and sessions with no open tab, and bumps that session's pulse.
/// The native daemon emits no such server event yet, so the oc2 seam is the
/// local `queued_drafts.push_back` feed in `native_loop`, which calls this.
pub fn record_enqueue(
    pulses: &mut std::collections::HashMap<SessionRef, TabStatus>,
    session: &SessionRef,
    is_open: bool,
    is_current: bool,
) {
    pulses
        .entry(session.clone())
        .or_default()
        .enqueue_user_prompt(is_open, is_current);
}

/// `session-tabs.tsx:169-187` TabIndicator `label()` in evaluation order.
/// Numbers mode always keeps the numeric label, which is what makes status
/// changes unable to move the title start cell.
#[must_use]
pub fn indicator_text(
    number: &str,
    status: TabStatus,
    indicators: TabIndicators,
    is_home: bool,
    spinner: TabSpinner,
    marker: TabUnreadMarker,
    idle: &str,
) -> String {
    if indicators == TabIndicators::Numbers {
        return number.to_owned();
    }
    match status.attention {
        Some(TabAttention::Permission) => return "!".to_owned(),
        Some(TabAttention::Question) => return "?".to_owned(),
        None => {}
    }
    if status.runs() {
        return spinner.frames()[0].to_owned();
    }
    if is_home {
        return "+".to_owned();
    }
    if status.unread_active() {
        return marker.glyph().to_owned();
    }
    idle.to_owned()
}

/// `session-tabs.tsx:1033-1041` / `1863-1871`: a renaming title is DIM while
/// animations are disabled; otherwise the selected tab is BOLD.
#[must_use]
pub fn title_attributes(selected: bool, renaming: bool, animations: bool) -> u32 {
    use crate::native_ui_style::{ATTR_BOLD, ATTR_DIM};
    if renaming && !animations {
        ATTR_DIM
    } else if selected {
        ATTR_BOLD
    } else {
        0
    }
}

/// `component/title-shimmer.tsx:33,50-55`: the shimmer renderable reports
/// `live = animating`, whose steady state is `shimmering = rename.pending`
/// while motion is enabled, so a pending tab title keeps requesting frames.
/// oc2 has no shimmer painter yet, but this predicate is the one seam that
/// lets the loop's animation clock stay live for a pending title exactly as
/// upstream does (`enabled && shimmering`); with motion off the title is the
/// static DIM frame from [`title_attributes`] and no clock is needed.
#[must_use]
pub const fn title_shimmer_runs(renaming: bool, animations: bool) -> bool {
    renaming && animations
}

/// The full pending-title fact for one session: an in-flight user rename OR an
/// in-flight generated/auto title. Upstream keys a SINGLE client-local map,
/// `generatingTitles[sessionID] === true` (`context/data.tsx:35`), and sets it
/// from BOTH the rename dialog (`dialog-session-rename.tsx:15-25`) and the
/// generated-title path `data.session.title.generate` (`context/data.tsx:37-46`,
/// reachable from `routes/session/index.tsx:1045`). oc2 currently owns only the
/// rename source (`RenameInFlight`); this names the second source so the shimmer
/// clock keys the same union the moment a generate request exists.
#[must_use]
pub const fn title_pending(renaming: bool, generating: bool) -> bool {
    renaming || generating
}

/// Port of `session-tabs.tsx:125-129` `tabFeedbackColor`: dialog attention
/// paints the accent hue, an error unread paints `text.feedback.error.base`,
/// otherwise no override. Upstream feeds this into both the indicator number
/// color and `unreadColor` (`session-tabs.tsx:959-963,1048`). The legacy
/// palette-only fallback keeps the recorded roles used elsewhere in this
/// module (attention -> Warning, error -> Error).
#[must_use]
pub fn tab_feedback_color(
    attention: Option<TabAttention>,
    unread: Option<TabUnread>,
    palette: &crate::native_theme_runtime::Palette,
    semantic: Option<&crate::native_theme_runtime::SemanticPalette>,
) -> Option<[u8; 4]> {
    use crate::native_theme_runtime::Slot;
    if attention.is_some() {
        return Some(semantic.map_or_else(|| palette.get(Slot::Warning), |theme| theme.tab_attention.rgba));
    }
    if matches!(unread, Some(TabUnread::Error)) {
        return Some(semantic.map_or_else(|| palette.get(Slot::Error), |theme| theme.text_feedback_error.rgba));
    }
    None
}

// ---------------------------------------------------------------------------
// Title fade and marquee overflow.
//
// Upstream `session-tabs.tsx:73-77` fixes FADE_WIDTH = 4, and
// `util/marquee.ts:1-51` defines the loop/offset arithmetic. Reduced-motion
// mode only disables the animation clock; the marquee text projection is
// identical.
// ---------------------------------------------------------------------------

/// `session-tabs.tsx:74`.
pub const FADE_WIDTH: usize = 4;
/// `session-tabs.tsx:75`.
pub const ADD_TAB_WIDTH: u16 = 3;
/// `session-tabs.tsx:77-78`.
pub const MARQUEE_DELAY_MS: u64 = 600;
pub const MARQUEE_INTERVAL_MS: u64 = 80;
/// `util/marquee.ts:4`.
pub const MARQUEE_GAP: &str = " · ";
/// `session-tabs.tsx:79`.
pub const CONTEXT_MENU_WIDTH: u16 = 20;
/// `session-tabs.tsx:80-83`.
pub const MOUSE_CLOSE_HOLD_MS: u64 = 5_000;

/// Opacity toward the tab background for one title cell. Port of
/// `session-tabs.tsx:221-228` `fadeTitleColor`: 0 keeps the text color, 1 is
/// fully dissolved into the background. The first and last FADE_WIDTH cells
/// ramp toward the background; the middle stays at 0.
#[must_use]
pub fn title_fade_weight(index: usize, length: usize, leading: f32) -> f32 {
    let fade = |position: isize| -> f32 {
        if position <= 0 {
            0.0
        } else {
            let span = FADE_WIDTH.saturating_sub(1).max(1) as f32;
            0.2 + 0.72 * ((position as f32 - 1.0) / span)
        }
    };
    let index_isize = index as isize;
    let start = if index < FADE_WIDTH {
        (FADE_WIDTH - index) as isize
    } else {
        0
    };
    let end = index_isize - (length as isize - FADE_WIDTH as isize) + 1;
    (fade(start) * leading).max(fade(end))
}

/// Sanitized grapheme clusters, matching the widths used by `display_width`.
fn graphemes(value: &str) -> Vec<String> {
    let mut out = Vec::new();
    visit_unpadded_cells(value, 4096, |glyph, _| out.push(glyph.to_owned()));
    out
}

pub(crate) fn take_width(value: &str, width: u16) -> String {
    let mut out = String::new();
    let mut used = 0usize;
    visit_unpadded_cells(value, usize::from(width), |glyph, cells| {
        if used + cells <= usize::from(width) {
            out.push_str(glyph);
            used += cells;
        }
    });
    out
}

/// `util/marquee.ts:9-11`.
#[must_use]
pub fn marquee_cycle_width(value: &str) -> usize {
    display_width(&format!("{value}{MARQUEE_GAP}"))
}

/// `util/marquee.ts:13-15`.
#[must_use]
pub fn marquee_overflows(value: &str, width: u16) -> bool {
    display_width(value) > usize::from(width)
}

/// `util/marquee.ts:5-8`.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct MarqueePart {
    pub value: String,
    pub separator: bool,
}

/// Direct port of `util/marquee.ts:21-50` `marqueeTextParts`.
#[must_use]
pub fn marquee_text_parts(value: &str, width: u16, offset: usize) -> Vec<MarqueePart> {
    if width == 0 {
        return Vec::new();
    }
    if !marquee_overflows(value, width) || offset == 0 {
        return graphemes(&take_width(value, width))
            .into_iter()
            .map(|value| MarqueePart {
                value,
                separator: false,
            })
            .collect();
    }
    let mut parts: Vec<MarqueePart> = graphemes(value)
        .into_iter()
        .map(|value| MarqueePart {
            value,
            separator: false,
        })
        .collect();
    parts.extend(graphemes(MARQUEE_GAP).into_iter().map(|value| MarqueePart {
        separator: value == "·",
        value,
    }));
    let cursor = offset % marquee_cycle_width(value).max(1);
    let looped: Vec<MarqueePart> = parts.iter().chain(parts.iter()).cloned().collect();
    let mut start = 0usize;
    let mut start_width = 0usize;
    for (index, part) in looped.iter().enumerate() {
        if start_width >= cursor {
            break;
        }
        start = index + 1;
        start_width += display_width(&part.value);
    }
    let mut visible = 0usize;
    let mut used = 0usize;
    for part in &looped[start..] {
        let cells = display_width(&part.value);
        if used + cells > usize::from(width) {
            break;
        }
        used += cells;
        visible += 1;
    }
    looped[start..start + visible].to_vec()
}

/// `util/marquee.ts:17-20` convenience joining of the parts.
#[must_use]
pub fn marquee_text(value: &str, width: u16, offset: usize) -> String {
    marquee_text_parts(value, width, offset)
        .into_iter()
        .map(|part| part.value)
        .collect()
}

/// `session-tabs.tsx:247-330` `createMarquee`, driven by an explicit clock so
/// reduced-motion/off-screen rendering can be replayed deterministically.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct Marquee {
    active: Option<String>,
    offset: usize,
    cycle_width: usize,
    started_at: Option<u64>,
    next_tick: Option<u64>,
}

impl Marquee {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn active(&self) -> Option<&str> {
        self.active.as_deref()
    }

    #[must_use]
    pub const fn offset(&self) -> usize {
        self.offset
    }

    /// The leading fade is fully on while a cycle is running.
    #[must_use]
    pub const fn leading(&self) -> bool {
        self.offset > 0
    }

    /// Next deterministic clock instant at which `advance` can change the
    /// visible frame: the 600 ms delay expiry, then each 80 ms step. The live
    /// loop wakes exactly here instead of rounding to its 80 ms animation tick,
    /// so the first frame lands on the upstream timer boundary.
    #[must_use]
    pub fn next_wake(&self) -> Option<u64> {
        if self.active.is_none() {
            return None;
        }
        if self.offset == 0 {
            return self.started_at.map(|started| started.saturating_add(MARQUEE_DELAY_MS));
        }
        self.next_tick
    }

    /// `session-tabs.tsx:272-290`: a fitting title resets; re-entering the
    /// active tab keeps its running cycle; otherwise wait 600 ms from zero.
    pub fn enter(&mut self, session: &str, title: &str, width: u16, now_ms: u64) {
        if !marquee_overflows(title, width) {
            self.reset();
            return;
        }
        if self.active() == Some(session) {
            return;
        }
        self.reset();
        self.cycle_width = marquee_cycle_width(title);
        self.active = Some(session.to_owned());
        self.started_at = Some(now_ms);
    }

    /// `session-tabs.tsx:291-294`.
    pub fn leave(&mut self, session: &str) {
        if self.active() == Some(session) {
            self.reset();
        }
    }

    /// `session-tabs.tsx:295-301`.
    pub fn reset(&mut self) {
        self.active = None;
        self.offset = 0;
        self.cycle_width = 0;
        self.started_at = None;
        self.next_tick = None;
    }

    /// Advance the deterministic clock. Returns true when the visible frame
    /// changed (offset or active identity).
    pub fn advance(&mut self, now_ms: u64) -> bool {
        let before = (self.active.clone(), self.offset);
        if self.active.is_none() {
            return false;
        }
        if self.offset == 0 {
            let Some(started) = self.started_at else {
                return false;
            };
            if now_ms < started.saturating_add(MARQUEE_DELAY_MS) {
                return false;
            }
            self.offset = 1;
            self.next_tick = Some(started + MARQUEE_DELAY_MS + MARQUEE_INTERVAL_MS);
        }
        while let Some(tick) = self.next_tick {
            if now_ms < tick {
                break;
            }
            if self.offset + 1 < self.cycle_width {
                self.offset += 1;
                self.next_tick = Some(tick + MARQUEE_INTERVAL_MS);
            } else {
                // One full cycle: clear the interval and drop the leading fade.
                self.reset();
                break;
            }
        }
        before != (self.active.clone(), self.offset)
    }
}

/// `session-tabs.tsx:303-326` `createTabMarquee`: hover identity plus the
/// zero-delay leave clear that a re-enter can cancel.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct TabMarquee {
    hovered: Option<String>,
    pending_leave: Option<(String, u64)>,
    marquee: Marquee,
}

impl TabMarquee {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn hovered(&self) -> Option<&str> {
        self.hovered.as_deref()
    }

    #[must_use]
    pub fn marquee(&self) -> &Marquee {
        &self.marquee
    }

    pub fn enter(&mut self, session: &str, title: &str, width: u16, now_ms: u64) {
        self.pending_leave = None;
        self.hovered = Some(session.to_owned());
        self.marquee.enter(session, title, width, now_ms);
    }

    /// `session-tabs.tsx:315-322`: schedule a zero-delay clear so re-entering
    /// the same tab before the next tick keeps the marquee alive.
    pub fn leave(&mut self, session: &str, now_ms: u64) {
        if self.hovered() == Some(session) {
            self.pending_leave = Some((session.to_owned(), now_ms));
        }
    }

    /// Leave whichever tab is hovered (`session-tabs.tsx:323-326`).
    pub fn leave_hovered(&mut self, now_ms: u64) {
        if let Some(session) = self.hovered.clone() {
            self.leave(&session, now_ms);
        }
    }

    pub fn reset(&mut self) {
        self.hovered = None;
        self.pending_leave = None;
        self.marquee.reset();
    }

    /// Apply the pending zero-delay leave at `now_ms`.
    pub fn advance(&mut self, now_ms: u64) -> bool {
        if let Some((session, at)) = self.pending_leave.clone() {
            if now_ms >= at {
                self.pending_leave = None;
                if self.hovered() == Some(&session) {
                    self.hovered = None;
                    self.marquee.leave(&session);
                }
            }
        }
        self.marquee.advance(now_ms)
    }
}

// ---------------------------------------------------------------------------
// Pointer interactions: drag preview, close-cell hold and gesture gating.
// Upstream `session-tabs.tsx:615-660` (drag/release), `session-tabs.tsx:868-900`
// (close hold), `session-tabs.tsx:986-1024` (selection release over add).
// ---------------------------------------------------------------------------

/// A pointer down/up target. A release only activates the control that also
/// received the press, so a transcript selection that ends over the plus,
/// search or close cell is inert.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum TabPointerTarget {
    Transcript,
    Session,
    Close,
    Add,
    Search,
    Overflow,
}

#[derive(Clone, Copy, Debug, Default, Eq, PartialEq)]
pub struct PointerGesture {
    down: Option<TabPointerTarget>,
    did_drag: bool,
}

impl PointerGesture {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    pub fn press(&mut self, target: TabPointerTarget) {
        self.down = Some(target);
        self.did_drag = false;
    }

    pub fn drag(&mut self) {
        if self.down.is_some() {
            self.did_drag = true;
        }
    }

    /// Release action, if any. Only session/close/add clicks that started on
    /// the same control (and did not drag) activate.
    pub fn release(&mut self, up: TabPointerTarget) -> Option<TabPointerTarget> {
        let down = self.down.take();
        let activate = !self.did_drag
            && down == Some(up)
            && matches!(
                up,
                TabPointerTarget::Session
                    | TabPointerTarget::Close
                    | TabPointerTarget::Add
                    | TabPointerTarget::Search
            );
        self.did_drag = false;
        if activate {
            Some(up)
        } else {
            None
        }
    }
}

/// Upstream drag preview (`session-tabs.tsx:598-614,1362-1382`): one local move
/// preview per drag, kept after release until the store reflects the move, and
/// a captured drag suppresses the synthetic release click.
#[derive(Clone, Debug, Default, Eq, PartialEq)]
pub struct DragState {
    source: Option<SessionRef>,
    source_index: usize,
    preview: Option<(SessionRef, usize)>,
    did_drag: bool,
    suppress_click: bool,
}

impl DragState {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    #[must_use]
    pub fn preview(&self) -> Option<(&SessionRef, usize)> {
        self.preview.as_ref().map(|(session, index)| (session, *index))
    }

    #[must_use]
    pub fn dragging(&self) -> Option<&SessionRef> {
        self.source.as_ref()
    }

    #[must_use]
    pub const fn suppress_click(&self) -> bool {
        self.suppress_click
    }

    pub fn press(&mut self, session: &SessionRef, index: usize) {
        self.source = Some(session.clone());
        self.source_index = index;
        self.did_drag = false;
    }

    /// Update the preview from a slot under the pointer. Moving onto the
    /// source slot clears it; repeated reports of one target keep one preview.
    pub fn drag_to(&mut self, slot: usize, tab_len: usize) {
        let Some(source) = self.source.clone() else {
            return;
        };
        if tab_len == 0 {
            return;
        }
        self.did_drag = true;
        let target = slot.min(tab_len - 1);
        if target == self.source_index {
            self.preview = None;
        } else if self.preview.as_ref() != Some(&(source.clone(), target)) {
            self.preview = Some((source, target));
        }
    }

    /// `release()` at `session-tabs.tsx:650-660`: persist at most one move,
    /// select the source, and suppress the synthetic click after a drag.
    pub fn release(&mut self) -> DragRelease {
        let source = self.source.take();
        let move_to = self
            .preview
            .as_ref()
            .filter(|(session, _)| Some(session) == source.as_ref())
            .map(|(_, index)| *index);
        if self.did_drag {
            self.suppress_click = true;
        }
        self.did_drag = false;
        DragRelease {
            select: source,
            move_to,
        }
    }

    /// Clear the preview once the tab store reflects it (`session-tabs.tsx:742-748`).
    pub fn settle(&mut self, tabs: &[SessionRef]) {
        let Some((session, index)) = self.preview.clone() else {
            return;
        };
        let expected = move_id(tabs, &session, index);
        if expected == tabs {
            self.preview = None;
        }
    }

    /// Clear the synthetic-click suppression in the next microtask.
    pub fn clear_suppress(&mut self) {
        self.suppress_click = false;
    }

    /// Local order with the single preview move applied.
    #[must_use]
    pub fn ordered(&self, tabs: &[SessionRef]) -> Vec<SessionRef> {
        match self.preview.clone() {
            Some((session, index)) => move_id(tabs, &session, index),
            None => tabs.to_vec(),
        }
    }

    /// Reorder a rendered input snapshot into the preview order so the paint
    /// and the hit-test snapshot both reflect the pending move
    /// (`session-tabs.tsx:598-602`). A no-op without a live preview.
    pub fn project(&self, inputs: &mut [TabInput], tabs: &[SessionRef]) {
        if self.preview.is_none() {
            return;
        }
        let order = self.ordered(tabs);
        inputs.sort_by_key(|input| {
            order
                .iter()
                .position(|id| *id == input.session)
                .unwrap_or(usize::MAX)
        });
    }
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct DragRelease {
    pub select: Option<SessionRef>,
    pub move_to: Option<usize>,
}

fn move_id(tabs: &[SessionRef], session: &SessionRef, index: usize) -> Vec<SessionRef> {
    let Some(from) = tabs.iter().position(|tab| tab == session) else {
        return tabs.to_vec();
    };
    let to = index.min(tabs.len().saturating_sub(1));
    if from == to {
        return tabs.to_vec();
    }
    let mut next: Vec<SessionRef> = tabs.iter().filter(|tab| *tab != session).cloned().collect();
    next.insert(to.min(next.len()), session.clone());
    next
}

/// Upstream close-cell hold (`session-tabs.tsx:868-900`): after a close the
/// pointer keeps the same physical close cell even when the strip overflows or
/// reflows, until leave or the 5 s timer releases it.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CloseHold {
    pub items: Vec<SessionRef>,
    pub visible: Vec<SessionRef>,
    pub widths: Vec<u16>,
    pub closed: SessionRef,
    pub target: SessionRef,
    pub x: u16,
    expires_at: u64,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct HeldLayout {
    pub tabs: Vec<SessionRef>,
    pub widths: Vec<u16>,
    pub start: usize,
    pub total: u16,
}

impl CloseHold {
    #[must_use]
    pub fn new(
        items: &[SessionRef],
        visible: &[SessionRef],
        widths: &[u16],
        closed: &SessionRef,
        target: &SessionRef,
        x: u16,
        now_ms: u64,
    ) -> Self {
        Self {
            items: items.to_vec(),
            visible: visible.to_vec(),
            widths: widths.to_vec(),
            closed: closed.clone(),
            target: target.clone(),
            x,
            expires_at: now_ms.saturating_add(MOUSE_CLOSE_HOLD_MS),
        }
    }

    #[must_use]
    pub fn expired(&self, now_ms: u64) -> bool {
        now_ms >= self.expires_at
    }

    /// Direct port of `session-tabs.tsx:229-268` `heldSessionTabLayout`.
    #[must_use]
    pub fn layout(&self, tabs: &[SessionRef]) -> Option<HeldLayout> {
        let expected: Vec<&SessionRef> = self.items.iter().filter(|id| **id != self.closed).collect();
        let unchanged = tabs == self.items.as_slice();
        let removed = tabs.len() == expected.len()
            && tabs.iter().zip(expected.iter()).all(|(tab, id)| tab == *id);
        if !unchanged && !removed {
            return None;
        }
        let mut visible: Vec<SessionRef> = self
            .visible
            .iter()
            .filter(|id| tabs.contains(id))
            .cloned()
            .collect();
        if removed && !visible.contains(&self.target) {
            visible.push(self.target.clone());
            visible.sort_by_key(|id| tabs.iter().position(|tab| tab == id).unwrap_or(usize::MAX));
        }
        let positions: Vec<usize> = visible
            .iter()
            .map(|id| tabs.iter().position(|tab| tab == id))
            .collect::<Option<Vec<_>>>()?;
        let start = *positions.first()?;
        if positions
            .iter()
            .enumerate()
            .any(|(index, position)| *position != start + index)
        {
            return None;
        }
        let mut widths: Vec<u16> = visible
            .iter()
            .map(|id| {
                self.widths
                    .get(self.visible.iter().position(|tab| tab == id).unwrap_or(usize::MAX))
                    .copied()
                    .unwrap_or(1)
            })
            .collect();
        if removed {
            let index = visible.iter().position(|id| id == &self.target)?;
            let leading = if start > 0 {
                u16::try_from(overflow_width(start)).unwrap_or(u16::MAX)
            } else {
                0
            };
            let preceding: u16 = widths.iter().take(index).copied().sum();
            // Upstream computes `hold.x - leading - preceding + 2` in signed
            // arithmetic and rejects a sub-1 result (`session-tabs.tsx:263-264`).
            // Saturating subtraction here would clamp the intermediate to 0 and
            // fabricate a two-cell tab instead of refusing the hold.
            let width = i32::from(self.x) - i32::from(leading) - i32::from(preceding) + 2;
            if width < 1 {
                return None;
            }
            widths[index] = u16::try_from(width).ok()?;
        }
        Some(HeldLayout {
            tabs: visible,
            total: widths.iter().copied().sum(),
            widths,
            start,
        })
    }
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

    /// TAB-16 live mapping (`session-tabs.tsx:1468-1489`): after closing the
    /// first visible tab of a 5-tab strip, the pointer column keeps the close
    /// cell of the neighbouring tab. Reproduces the live caller: painted widths
    /// for `visible`, the close x off the closed tab's own close cell, strip
    /// space via `strip_origin_x`.
    #[test]
    fn close_hold_keeps_pointer_column_on_the_neighbouring_close_cell() {
        let ids: Vec<SessionRef> = ["a", "b", "c", "d", "e"]
            .into_iter()
            .map(|id| SessionRef(id.into()))
            .collect();
        let inputs: Vec<_> = ids.iter().map(|id| input(&id.0, "tab")).collect();
        // Narrow strip with trailing overflow, scrolled to the front.
        let strip =
            TabStrip::render_with_hover_and_add(&inputs, TabOrientation::Horizontal, 30, 1, 0, None, false);
        assert!(strip.rects.len() < ids.len(), "the strip overflows");
        let (closed_id, closed_rect) = strip.rects[0].clone();
        assert_eq!(closed_id, ids[0]);
        let origin = strip.strip_origin_x();
        assert_eq!(origin, closed_rect.x, "no leading gutter at scroll 0");
        // The pointer presses the closed tab's own close cell.
        let pointer = closed_rect.x + closed_rect.width - 2;
        let hold = CloseHold::new(
            &ids,
            &strip.rects.iter().map(|(id, _)| id.clone()).collect::<Vec<_>>(),
            &strip.rects.iter().map(|(_, rect)| rect.width).collect::<Vec<_>>(),
            &closed_id,
            &ids[1],
            pointer - origin,
            0,
        );
        let after: Vec<SessionRef> = ids[1..].to_vec();
        let held = hold.layout(&after).expect("held layout applies to the removal");
        let held_inputs: Vec<_> = after.iter().map(|id| input(&id.0, "tab")).collect();
        let held_strip = TabStrip::render_with_held(
            &held_inputs,
            TabOrientation::Horizontal,
            30,
            1,
            0,
            None,
            false,
            false,
            false,
            Some(&held),
        );
        let (target_id, target_rect) = held_strip
            .rects
            .iter()
            .find(|(id, _)| id == &ids[1])
            .expect("target painted")
            .clone();
        assert_eq!(target_id, ids[1]);
        assert_eq!(
            target_rect.x + target_rect.width - 2,
            pointer,
            "the pointer keeps the same physical close cell after the reflow"
        );
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
            attention: None,
            unread: false,
            unread_error: false,
            marquee_offset: 0,
            indicators: TabIndicators::Status,
            number: String::new(),
            prompt_pulse: 0,
            spinner_frame: None,
        }
    }

    /// TAB-15: `TabStrip::drag_slot` maps a pointer column to the absolute tab
    /// slot the way upstream `slotAt` does (`session-tabs.tsx:1490-1500`):
    /// painted edges accumulate from the leading overflow marker, a column past
    /// the first edge resolves to `before + 0`, and no painted strip resolves
    /// to `None`.
    #[test]
    fn tab_strip_drag_slot_matches_slotat() {
        let tabs: Vec<_> = (0..10)
            .map(|i| input(&format!("s{i}"), &format!("title-{i}")))
            .collect();
        // Narrow strip with a mid-list scroll: leading and trailing overflow,
        // so `before > 0` and the first painted rect starts after the `‹N`
        // marker.
        let strip =
            TabStrip::render_with_hover_and_add(&tabs, TabOrientation::Horizontal, 30, 1, 5, None, true);
        assert!(strip.before > 0 && strip.overflow_before, "leading overflow");
        assert!(strip.overflow_after, "trailing overflow");
        let first = strip.rects[0].1;
        // A column inside the leading overflow marker swallows into slot `before`
        // (edge accumulation reaches `before + 0` first, upstream `slotAt`).
        assert_eq!(strip.drag_slot(0, tabs.len()), Some(strip.before));
        // Each painted rect resolves to its own absolute slot.
        for (index, (_, rect)) in strip.rects.iter().enumerate() {
            assert_eq!(
                strip.drag_slot(rect.x, tabs.len()),
                Some(strip.before + index),
                "left edge of painted slot {index}"
            );
            assert_eq!(
                strip.drag_slot(rect.x + rect.width.saturating_sub(1), tabs.len()),
                Some(strip.before + index),
                "right edge of painted slot {index}"
            );
        }
        assert_eq!(
            strip.drag_slot(first.x + 40, tabs.len()),
            Some(strip.before + strip.rects.len() - 1),
            "a column past the last edge clamps to the last painted slot"
        );
        // An empty strip has no slot.
        let empty = TabStrip::render_with_hover_and_add(&[], TabOrientation::Horizontal, 40, 1, 0, None, false);
        assert!(empty.rects.is_empty());
        assert_eq!(empty.drag_slot(3, 0), None);
        assert_eq!(empty.drag_slot(3, 5), None);
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
            attention: None,
            unread: false,
            unread_error: false,
            marquee_offset: 0,
            indicators: TabIndicators::Status,
            number: String::new(),
            prompt_pulse: 0,
            spinner_frame: None,
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
        assert_eq!(indicator_label(&tab.flags, tab.attention, false, tab.unread), "!");
        tab.flags.needs_attention = false;
        assert_eq!(indicator_label(&tab.flags, tab.attention, false, tab.unread), "\u{280b}", "busy paints the dots spinner frame 0");
        tab.flags.busy = false;
        assert_eq!(indicator_label(&tab.flags, tab.attention, false, tab.unread), "", "idle status column is blank in status mode");
        assert_eq!(indicator_label(&tab.flags, tab.attention, false, true), "\u{2022}", "activity-unread paints TAB_UNREAD_MARKERS[\"small-dot\"]");
        assert_eq!(indicator_label(&tab.flags, tab.attention, true, tab.unread), "+", "promoted Home always labels +");
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
                indicator_label(&tab.flags, tab.attention, false, tab.unread)
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
                indicator_label(&tab.flags, tab.attention, false, tab.unread)
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
        // idle bg=background.base fg=text.muted; hovered bg=raised.base
        // (action.primary.hovered), dragged/selected bg=raised.high fg=text.base
        // (selected bold).
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

    /// `session-tabs.tsx:1648-1658`: the indicator number is
    /// `tabFeedbackColor(attention, unread) ?? busyNumber ?? ...`, so dialog
    /// attention outranks a simultaneously-running tab. Before this the code
    /// tested `busy` first and a tab that was both busy and asking kept the
    /// running hue instead of the accent `!`/`?`.
    #[test]
    fn attention_feedback_outranks_busy_for_the_indicator_number() {
        use crate::native_theme_runtime::{Palette, RuntimeTheme, ThemeMode};
        let theme = RuntimeTheme::builtin_default(ThemeMode::Dark);
        let palette = Palette::opencode_dark();
        let semantic = theme.semantic.as_ref().expect("built-in V2 semantic palette");
        let mut both = input("both", "running and asking");
        both.flags = TabFlags { active: false, busy: true, needs_attention: true };
        let strip = TabStrip::render(&[both], TabOrientation::Horizontal, 40, 1, 0)
            .with_row_colors_semantic(&palette, theme.semantic.as_ref());
        assert_eq!(
            strip.row_paints[0].number_fg,
            semantic.tab_attention.rgba,
            "attention feedback outranks the busy hue"
        );
        assert_ne!(strip.row_paints[0].number_fg, semantic.tab_interactive.rgba);
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

        // Hovered add control: bg=raised.base (upstream session-tabs.tsx:1769-1770
        // lifts `theme.background.action.primary.hovered`) and both fg=Text.
        let add_hover = add_strip.with_add_hovered(true, &palette);
        assert!(add_hover.add_hovered());
        let paint = add_hover.add_paint.expect("add paint after builder");
        assert_eq!(paint.color, TabCellColor::AddHovered);
        assert_eq!(paint.background, palette.get(Slot::BackgroundPanel));
        assert_eq!(paint.plus_fg, palette.get(Slot::Text));
        assert_eq!(paint.new_text_fg, palette.get(Slot::Text));

        // Disabling the fact again restores the idle style.
        let back = add_hover.with_add_hovered(false, &palette);
        assert_eq!(
            back.add_paint.expect("idle restore").color,
            TabCellColor::AddIdle
        );
    }

    // -- TAB-01..TAB-24: status, marquee and interaction parity -------------

    #[test]
    fn tab_01_adaptive_fixture_allocations_and_sticky_window() {
        // fixtures.json tab-allocation-7-3-76 / 5-4-54 / 3-1-100 / 4-1-100 /
        // 5-2-100 / 1-0-100, model_available_width already excludes the plus.
        let fixtures = [
            (7usize, 3usize, 76usize, vec![9, 9, 9, 22, 9, 9, 9]),
            (5, 4, 54, vec![8, 8, 8, 8, 22]),
            (3, 1, 100, vec![32, 32, 32]),
            (4, 1, 100, vec![25, 25, 25, 25]),
            (5, 2, 100, vec![19, 19, 24, 19, 19]),
            (1, 0, 100, vec![32]),
        ];
        for (len, active, available, widths) in fixtures {
            let got = adaptive_layout(len, Some(active), available, 0);
            assert_eq!((got.start, got.count), (0, len), "len={len} all fit");
            assert_eq!(got.widths, widths, "len={len} active={active}");
            assert!(
                got.widths.iter().sum::<usize>() <= available,
                "never exceeds the model width"
            );
        }
        // Sticky overflow window: an active tab outside the window pulls the
        // start only as far as needed; a retained previous start sticks.
        let sticky = adaptive_layout(12, Some(7), 40, 5);
        assert_eq!((sticky.start, sticky.count), (6, 2));
        assert!(sticky.start <= 7 && 7 < sticky.start + sticky.count);
        assert_eq!(adaptive_layout(12, Some(7), 40, 8).start, 7);
        assert_eq!(adaptive_layout(12, Some(1), 40, 8).start, 1);
        assert_eq!(adaptive_layout(12, Some(10), 40, 2).start, 9);
    }

    #[test]
    fn tab_02_idle_plus_consumes_three_cells_and_active_new_replaces_it() {
        // session-tabs.tsx:75 ADD_TAB_WIDTH = 3; session-tabs.tsx:1436-1448
        // showPlus() is false while the promoted new-session tab is active.
        let tabs = vec![input("a", "alpha")];
        let idle = TabStrip::render_with_hover_and_add(
            &tabs,
            TabOrientation::Horizontal,
            20,
            1,
            0,
            None,
            true,
        );
        assert_eq!(idle.add_rect.unwrap().width, ADD_TAB_WIDTH);
        assert_eq!(idle.add_rect.unwrap().x + ADD_TAB_WIDTH, 20);
        let promoted = TabStrip::render_with_hover_add_home(
            &tabs,
            TabOrientation::Horizontal,
            20,
            1,
            0,
            None,
            true,
            true,
            false,
        );
        assert!(promoted.add_rect.is_none(), "idle plus is not duplicated");
        assert!(promoted.home_rect.is_some(), "the active new tab occupies the slot");
        let home = promoted.home_rect.unwrap();
        assert_eq!(home.x + home.width, 20, "promoted slot uses the reserved strip width");
    }

    #[test]
    fn tab_03_status_switch_never_shifts_title_start_and_number_width_grows() {
        // session-tabs.tsx:1005-1013 numberWidth = max(2, digits(items.len()));
        // TabIndicator owns a fixed numberWidth+1 column.
        assert_eq!(number_width(9), 2);
        assert_eq!(number_width(99), 2);
        assert_eq!(number_width(100), 3);
        assert_eq!(number_width(1000), 4);
        let cases = [
            (TabStatus::default(), "1"),
            (
                TabStatus {
                    busy: true,
                    ..TabStatus::default()
                },
                "1",
            ),
            (
                TabStatus {
                    attention: Some(TabAttention::Permission),
                    ..TabStatus::default()
                },
                "1",
            ),
            (
                TabStatus {
                    attention: Some(TabAttention::Question),
                    ..TabStatus::default()
                },
                "1",
            ),
            (
                TabStatus {
                    unread: Some(TabUnread::Activity),
                    ..TabStatus::default()
                },
                "1",
            ),
        ];
        for (status, number) in cases {
            for indicators in [TabIndicators::Status, TabIndicators::Numbers] {
                let glyph = indicator_text(
                    number,
                    status,
                    indicators,
                    false,
                    TabSpinner::Dots,
                    TabUnreadMarker::SmallDot,
                    "",
                );
                let column = format_indicator_column(&glyph, 3);
                assert_eq!(display_width(&column), 3, "indicator column width is fixed");
            }
        }
        // The title region always begins after the same numberWidth+1+pad
        // cells; 100 tabs grow the column but every status keeps its start.
        let strip = TabStrip::render(&[input("a", "alpha")], TabOrientation::Horizontal, 40, 1, 0);
        assert_eq!(strip.indicator_cells, 3);
    }

    #[test]
    fn tab_04_busy_permission_question_glyphs_and_numbers_mode() {
        // session-tabs.tsx:169-187 label() precedence.
        let busy = TabStatus {
            busy: true,
            ..TabStatus::default()
        };
        let permission = TabStatus {
            busy: true,
            attention: Some(TabAttention::Permission),
            ..TabStatus::default()
        };
        let question = TabStatus {
            busy: true,
            attention: Some(TabAttention::Question),
            ..TabStatus::default()
        };
        let text = |status| {
            indicator_text(
                "4",
                status,
                TabIndicators::Status,
                false,
                TabSpinner::Dots,
                TabUnreadMarker::SmallDot,
                "",
            )
        };
        assert_eq!(text(busy), "⠋", "dots frame 0 while running");
        assert_eq!(text(permission), "!", "permission pauses and replaces the spinner");
        assert_eq!(text(question), "?", "question pauses and replaces the spinner");
        for status in [busy, permission, question] {
            assert_eq!(
                indicator_text(
                    "4",
                    status,
                    TabIndicators::Numbers,
                    false,
                    TabSpinner::Dots,
                    TabUnreadMarker::SmallDot,
                    ""
                ),
                "4",
                "numbers mode never changes glyph"
            );
        }
        assert_eq!(TabSpinner::Dots.interval_ms(), 80);
        assert_eq!(TabSpinner::Arcs.frames()[0], "◜");
        assert_eq!(TabSpinner::Arcs.interval_ms(), 120);
        assert_eq!(
            indicator_text(
                "1",
                TabStatus {
                    unread: Some(TabUnread::Error),
                    ..TabStatus::default()
                },
                TabIndicators::Status,
                false,
                TabSpinner::Dots,
                TabUnreadMarker::Dot,
                ""
            ),
            "●"
        );
        assert_eq!(
            indicator_text(
                "1",
                TabStatus {
                    unread: Some(TabUnread::Activity),
                    busy: true,
                    ..TabStatus::default()
                },
                TabIndicators::Status,
                false,
                TabSpinner::Dots,
                TabUnreadMarker::SmallDot,
                ""
            ),
            "⠋",
            "pending work outranks the unread marker"
        );
    }

    // TAB-04: the live horizontal row honours `config.tabs.indicators`. Numbers
    // mode keeps the numeric label in the indicator column
    // (`session-tabs.tsx:525,173`), so a status change never moves the title
    // start cell; status mode keeps the existing glyph precedence.
    #[test]
    fn tab_04_numbers_mode_paints_the_number_in_the_live_horizontal_row() {
        let numbers = TabInput {
            flags: TabFlags {
                busy: true,
                ..TabFlags::default()
            },
            number: "3".into(),
            indicators: TabIndicators::Numbers,
            ..input("s", "title")
        };
        let strip = TabStrip::render(&[numbers], TabOrientation::Horizontal, 20, 1, 0);
        let row = &strip.rows[0];
        assert!(
            row.starts_with(" 3 "),
            "numbers mode paints the tab number in the indicator column: {row:?}"
        );
        assert!(
            !row.contains('\u{280b}'),
            "numbers mode suppresses the busy spinner: {row:?}"
        );

        let status = TabInput {
            flags: TabFlags {
                busy: true,
                ..TabFlags::default()
            },
            number: "3".into(),
            indicators: TabIndicators::Status,
            ..input("s", "title")
        };
        let strip = TabStrip::render(&[status], TabOrientation::Horizontal, 20, 1, 0);
        assert!(
            strip.rows[0].contains('\u{280b}'),
            "status mode paints the busy spinner glyph: {:?}",
            strip.rows[0]
        );
    }

    // TAB-04: the live busy-tab spinner animates through `TAB_SPINNER_DOTS`
    // from the frame the live loop feeds (`now_ms / 80`). The static default
    // (`None`) and frame 0 both paint `⠋`; a different frame paints a different
    // glyph, so this fails if `horizontal_row` ignores `spinner_frame`.
    #[test]
    fn tab_04_busy_spinner_animates_from_the_live_frame() {
        let painted = |frame: Option<u64>| {
            let tab = TabInput {
                flags: TabFlags {
                    busy: true,
                    ..TabFlags::default()
                },
                spinner_frame: frame,
                ..input("s", "title")
            };
            TabStrip::render(&[tab], TabOrientation::Horizontal, 20, 1, 0).rows[0].clone()
        };
        assert!(painted(None).contains('⠋'), "reduced motion keeps frame 0");
        assert!(painted(Some(0)).contains('⠋'), "frame 0 is the first dot");
        assert!(
            painted(Some(1)).contains('⠙'),
            "frame 1 advances to the second dot: {:?}",
            painted(Some(1))
        );
        assert_ne!(painted(Some(0)), painted(Some(1)), "frames must differ");
        assert_eq!(painted(Some(10)), painted(Some(0)), "the frame wraps modulo the table");
        // A non-busy tab ignores the frame entirely.
        let idle = TabInput {
            spinner_frame: Some(3),
            ..input("s", "title")
        };
        assert!(
            !TabStrip::render(&[idle], TabOrientation::Horizontal, 20, 1, 0).rows[0].contains('⠙'),
            "an idle tab never paints a spinner frame"
        );
    }

    // TAB-04: `any_running` keeps the live loop's 80 ms tick alive for a busy
    // tab even when it is not the active session (so the active worker alone
    // cannot be the gate).
    #[test]
    fn tab_04_any_running_covers_background_and_family_tabs() {
        use crate::native_navigation::{DaemonEvent, NavigationState, WorkState};
        let mut nav = NavigationState::new();
        assert!(!any_running(&nav), "no tabs is not running");
        nav.open_tab(SessionRef("a".into()));
        assert!(!any_running(&nav), "an idle tab is not running");
        nav.apply(DaemonEvent::WorkState {
            session: SessionRef("a".into()),
            state: WorkState::Running,
        });
        assert!(any_running(&nav), "a busy tab is running");
        // A running descendant (opened via `open_forked`) still wakes the
        // parent tab's spinner, mirroring the `from_navigation` family roll-up.
        let mut family = NavigationState::new();
        family.open_tab(SessionRef("parent".into()));
        family.open_forked(SessionRef("child".into()), SessionRef("parent".into()));
        family.apply(DaemonEvent::WorkState {
            session: SessionRef("child".into()),
            state: WorkState::Running,
        });
        assert!(any_running(&family), "a running descendant keeps the tick alive");
    }

    #[test]
    fn tab_05_family_status_aggregation() {
        // context/session-tabs.tsx:143-173: unread reads the root only;
        // attention scans the family with permission first.
        let root_unread = TabStatus {
            unread: Some(TabUnread::Activity),
            ..TabStatus::default()
        };
        let child_only_unread = TabStatus {
            unread: Some(TabUnread::Error),
            ..TabStatus::default()
        };
        let aggregate = TabStatus::family(root_unread, &[child_only_unread]);
        assert_eq!(aggregate.unread, Some(TabUnread::Activity));
        assert_eq!(aggregate.attention, None, "child-only unread never wakes attention");

        let question = TabStatus {
            attention: Some(TabAttention::Question),
            ..TabStatus::default()
        };
        let permission = TabStatus {
            attention: Some(TabAttention::Permission),
            ..TabStatus::default()
        };
        let busy_child = TabStatus {
            busy: true,
            ..TabStatus::default()
        };
        assert_eq!(
            TabStatus::family(TabStatus::default(), &[question, permission]).attention,
            Some(TabAttention::Permission),
            "permission outranks question regardless of order"
        );
        assert_eq!(
            TabStatus::family(TabStatus::default(), &[question]).attention,
            Some(TabAttention::Question)
        );
        let busy = TabStatus::family(TabStatus::default(), &[busy_child]);
        assert!(busy.busy, "a child's work marks the root busy");
        assert!(!busy.complete(), "busy never completes");
        assert!(!busy.glows(false), "busy tabs do not glow");
        assert!(
            TabStatus::family(root_unread, &[]).glows(false),
            "an idle unread root glows"
        );
    }

    /// TAB-05 live path: the `/children` listing feed (the `native_loop`
    /// roll-up for the active route session) marks the root tab busy through
    /// `apply_live_status` exactly like upstream's family `busy` scan
    /// (`context/session-tabs.tsx:143-173`). The member builder filters the
    /// root row out, so a root-only "running" entry never self-marks.
    #[test]
    fn tab_05_live_child_busy_rolls_into_root_flags() {
        use crate::native_children::live;
        use serde_json::json;
        // Session ids must be valid `SessionId`s (UUIDs) for the listing to
        // parse, exactly like the live `/children` payload.
        const ROOT: &str = "11111111-1111-1111-1111-111111111111";
        const CHILD: &str = "22222222-2222-2222-2222-222222222222";
        const IDLE: &str = "33333333-3333-3333-3333-333333333333";
        let mut listing = live::Status::default();
        listing
            .apply(ROOT, &json!({"children": [
                {"session_id": ROOT, "state": "running"},
                {"session_id": CHILD, "state": "running"},
                {"session_id": IDLE, "state": "idle"},
            ]}))
            .unwrap();

        let build = || {
            let root = EMPTY_TAB_STATUS;
            let members: Vec<TabStatus> = listing
                .rows()
                .iter()
                .filter(|row| row.session_id != ROOT)
                .map(|row| TabStatus {
                    busy: row.is_running(),
                    ..TabStatus::default()
                })
                .collect();
            TabStatus::family(root, &members)
        };
        assert!(build().busy, "any running child marks the root busy");

        let mut root_input = input(ROOT, "root session");
        crate::native_tabs::apply_live_status(&mut root_input, Some(&build()), &[]);
        assert!(root_input.flags.busy, "the live family roll-up reaches the strip flags");

        // A root-only listing (all other rows idle) does not self-mark.
        let mut alone = live::Status::default();
        alone
            .apply(ROOT, &json!({"children": [
                {"session_id": ROOT, "state": "running"},
                {"session_id": IDLE, "state": "idle"},
            ]}))
            .unwrap();
        let idle_members: Vec<TabStatus> = alone
            .rows()
            .iter()
            .filter(|row| row.session_id != ROOT)
            .map(|row| TabStatus {
                busy: row.is_running(),
                ..TabStatus::default()
            })
            .collect();
        assert!(!TabStatus::family(EMPTY_TAB_STATUS, &idle_members).busy);
        let mut idle_input = input(ROOT, "root session");
        crate::native_tabs::apply_live_status(&mut idle_input, None, &[]);
        assert!(!idle_input.flags.busy, "a no-op live feed never fabricates busy");
    }

    #[test]
    fn tab_06_background_enqueue_flash_and_synthetic_parking() {
        // context/session-tabs.tsx:355-361 enqueue flash; :167-171 pending busy.
        let mut status = TabStatus {
            busy: true,
            ..TabStatus::default()
        };
        status.enqueue_user_prompt(true, false);
        assert_eq!(status.prompt_pulse, 1, "a busy tab still flashes");
        status.enqueue_user_prompt(true, true);
        assert_eq!(status.prompt_pulse, 1, "the visible tab is not flashed");
        status.enqueue_user_prompt(false, false);
        assert_eq!(status.prompt_pulse, 1, "closed tabs are not flashed");

        let synthetic_only = [PendingKind::Synthetic, PendingKind::Synthetic];
        assert!(!pending_marks_busy(&synthetic_only));
        assert!(!member_busy(false, &synthetic_only));
        assert!(member_busy(false, &[PendingKind::User]));
        assert!(member_busy(true, &[]));
        assert!(member_busy(false, &[PendingKind::Synthetic, PendingKind::User]));
    }

    /// TAB-06 live seam: `context/session-tabs.tsx:334-338` bumps the pulse
    /// only for a `user` enqueue on another open tab. `record_enqueue` is the
    /// exact function the oc2 `queued_drafts.push_back` seam calls
    /// (`tui_entry.rs` `SubmitPlan::Queue`).
    #[test]
    fn tab06_live_seam_record_enqueue_matches_upstream_rules() {
        let target = SessionRef("bg-session".into());
        let mut pulses = std::collections::HashMap::new();

        // Open, not current: the flash fires.
        record_enqueue(&mut pulses, &target, true, false);
        assert_eq!(pulses[&target].prompt_pulse, 1);
        // Closed tabs never flash.
        let closed = SessionRef("closed".into());
        record_enqueue(&mut pulses, &closed, false, false);
        assert_eq!(pulses[&closed].prompt_pulse, 0);
        // The current route never flashes.
        record_enqueue(&mut pulses, &target, true, true);
        assert_eq!(pulses[&target].prompt_pulse, 1);
        // Pulses accumulate across enqueues.
        record_enqueue(&mut pulses, &target, true, false);
        assert_eq!(pulses[&target].prompt_pulse, 2);
    }

    /// TAB-06 live path: the queue seam's pulse map fed through
    /// `apply_live_status` (the `native_loop` feed after `from_navigation`)
    /// reaches the strip input and ignites the row's number color; draining
    /// the flash restores the idle color for the next frame.
    #[test]
    fn tab06_live_status_feed_ignites_strip_number_then_consumes() {
        use crate::native_theme_runtime::{Palette, Slot};
        let palette = Palette::opencode_dark();
        let idle = input("bg", "background tab");
        let mut flashing = input("bg", "background tab");
        crate::native_tabs::apply_live_status(
            &mut flashing,
            Some(&TabStatus { prompt_pulse: 1, ..TabStatus::default() }),
            &[],
        );
        assert_eq!(flashing.prompt_pulse, 1, "the pulse feeds the live TabInput");
        // Pending user items mark the tab busy through `member_busy`.
        let mut queued = input("bg", "background tab");
        crate::native_tabs::apply_live_status(
            &mut queued,
            None,
            &[PendingKind::User],
        );
        assert!(queued.flags.busy, "a queued user item marks the tab busy");

        let flash_strip = TabStrip::render_with_hover(
            &[flashing.clone()],
            TabOrientation::Horizontal,
            60,
            1,
            0,
            None,
        )
        .with_row_colors(&palette);
        let idle_strip = TabStrip::render_with_hover(
            &[idle],
            TabOrientation::Horizontal,
            60,
            1,
            0,
            None,
        )
        .with_row_colors(&palette);
        let ignited = flash_strip.row_paints[0].number_fg;
        let steady = idle_strip.row_paints[0].number_fg;
        assert_ne!(ignited, steady, "a nonzero pulse ignites the number color");
        let [r, g, b, a] = steady;
        let [ir, ig, ib, ia] = ignited;
        // Upstream ignition tints the number toward `theme.text.base`
        // (`component/session-tabs.tsx:786-792`), so every opaque channel
        // moves toward text.base, never away.
        let text = palette.get(Slot::Text);
        let toward = |steady: u8, ignited: u8, base: u8| -> bool {
            if steady == base { ignited == base } else {
                (ignited as i32 - base as i32).abs() < (steady as i32 - base as i32).abs()
            }
        };
        assert!(toward(r, ir, text[0]) && toward(g, ig, text[1]) && toward(b, ib, text[2]),
            "the ignition tint moves the number toward text.base");
        assert_eq!(ia, a, "alpha untouched");
    }

    #[test]
    fn tab_07_selected_hovered_and_dragged_paint_attributes() {
        // session-tabs.tsx:1522-1523,1567: selected/dragged are BOLD, hovered
        // only lifts the background; the existing paint tests cover colors.
        assert_eq!(
            title_attributes(true, false, true),
            crate::native_ui_style::ATTR_BOLD
        );
        assert_eq!(title_attributes(false, false, true), 0);
        assert_eq!(
            title_attributes(false, false, false),
            0,
            "drag boldness is supplied by the selected flag"
        );
    }

    /// TAB-07 fixture matrix (`fixtures.json` tab-state-*): the seven tab
    /// states resolve the upstream feedback color and title attribute.
    /// idle/busy/unread keep no override; permission and question paint the
    /// accent hue (`session-tabs.tsx:126`); error_unread paints
    /// `text.feedback.error.base` (`:127`); renaming is DIM only with motion
    /// off (`:1033-1041`), otherwise the selected BOLD rule applies.
    #[test]
    fn tab_07_state_fixture_color_and_attribute_matrix() {
        use crate::native_theme_runtime::{Palette, RuntimeTheme, Slot, ThemeMode};
        let palette = Palette::opencode_dark();
        let theme = RuntimeTheme::builtin_default(ThemeMode::Dark);
        let sem = theme.semantic.as_ref();
        let accent = sem.map_or_else(|| palette.get(Slot::Warning), |t| t.tab_attention.rgba);
        let error = sem.map_or_else(|| palette.get(Slot::Error), |t| t.text_feedback_error.rgba);

        // tab-state-idle / tab-state-busy / tab-state-unread: no feedback hue.
        assert_eq!(tab_feedback_color(None, None, &palette, sem), None);
        assert_eq!(tab_feedback_color(None, Some(TabUnread::Activity), &palette, sem), None);
        // tab-state-permission / tab-state-question: attention accent.
        assert_eq!(tab_feedback_color(Some(TabAttention::Permission), None, &palette, sem), Some(accent));
        assert_eq!(tab_feedback_color(Some(TabAttention::Question), None, &palette, sem), Some(accent));
        // tab-state-error_unread: error hue, and attention still outranks it.
        assert_eq!(tab_feedback_color(None, Some(TabUnread::Error), &palette, sem), Some(error));
        assert_ne!(accent, error, "attention accent and error hue differ");
        assert_eq!(
            tab_feedback_color(Some(TabAttention::Permission), Some(TabUnread::Error), &palette, sem),
            Some(accent),
            "attention outranks an unread error"
        );
        // tab-state-renaming: DIM only while motion is off.
        assert_eq!(title_attributes(false, true, false), crate::native_ui_style::ATTR_DIM);
        assert_eq!(title_attributes(true, true, true), crate::native_ui_style::ATTR_BOLD);
        assert_eq!(title_attributes(false, false, true), 0);
    }

    /// TAB-07: the hovered (non-dragged) horizontal tab paints at
    /// `action.primary.hovered` == `raised.base` — both bundled themes pin
    /// `action.primary.$hovered` to `$hue.neutral.700` (`opencode.json:63`,
    /// `opencode2.json:65`), the same value as `raised.base`
    /// (`opencode.json:56`, `opencode2.json:58`) — NOT at the selected
    /// elevation. A dragged tab lifts to `raised.high` and turns BOLD
    /// (`session-tabs.tsx:1568-1573,1642`), the selected tab wins over the
    /// drag, and releasing the drag restores the hovered/idle roles.
    #[test]
    fn tab_07_hovered_paints_raised_base_and_dragged_lifts_to_selected_elevation() {
        use crate::native_theme_runtime::{Palette, Slot};
        let palette = Palette::opencode_dark();
        let mut hovered_tab = input("hov", "hovered");
        hovered_tab.flags = TabFlags { active: false, busy: false, needs_attention: false };
        let mut selected_tab = input("sel", "selected");
        selected_tab.flags = TabFlags { active: true, busy: false, needs_attention: false };
        let render = |dragged: Option<&SessionRef>| {
            TabStrip::render_with_hover(
                &[
                    input("idle", "idle"),
                    hovered_tab.clone(),
                    selected_tab.clone(),
                ],
                TabOrientation::Horizontal,
                120,
                1,
                0,
                Some(&SessionRef("hov".into())),
            )
            .with_dragged(dragged)
            .with_row_colors(&palette)
        };
        let hovered = render(None);
        assert_eq!(
            hovered.row_paints[1].background,
            palette.get(Slot::BackgroundPanel),
            "hovered bg = action.primary.hovered = raised.base (session-tabs.tsx:1568-1573)"
        );
        assert_eq!(hovered.row_paints[1].title_fg, palette.get(Slot::Text));
        assert_eq!(hovered.row_paints[1].title_attr, 0, "hovered alone is not bold");
        assert_eq!(
            hovered.row_paints[1].number_fg,
            palette.get(Slot::Text),
            "hovered number base = foreground() = text.base (:1635-1636)"
        );
        assert_eq!(
            hovered.row_paints[2].background,
            palette.get(Slot::BackgroundElement),
            "selected keeps raised.high = decrease(raised.base)"
        );
        let dragged = render(Some(&SessionRef("hov".into())));
        assert_eq!(
            dragged.row_paints[1].color,
            TabCellColor::DraggedHovered,
            "dragging the hovered tab keeps the hover fact for the number color"
        );
        assert_eq!(
            dragged.row_paints[1].background,
            palette.get(Slot::BackgroundElement),
            "dragged lifts toward decrease(raised.base) by 1 == raised.high"
        );
        assert_eq!(dragged.row_paints[1].title_fg, palette.get(Slot::Text));
        assert_eq!(dragged.row_paints[1].title_attr, 1, "dragged is BOLD (:1642)");
        assert_eq!(
            dragged.row_paints[1].number_fg,
            palette.get(Slot::Text),
            "dragged+hovered number base = foreground() = text.base (:1632-1641)"
        );
        assert_eq!(
            dragged.row_paints[2].color,
            TabCellColor::Selected,
            "selected wins over the drag fact"
        );
        assert_eq!(dragged.row_paints[2].title_attr, 1);
        // Dragging a tab the pointer is NOT over keeps the idle number formula:
        // upstream `numberColor` only takes `foreground()` when
        // `hovered() && !selected()` (`session-tabs.tsx:1632-1641`), independent
        // of the drag fact. The dragged background/bold still apply.
        let dragged_away = render(Some(&SessionRef("idle".into())));
        assert_eq!(
            dragged_away.row_paints[0].color,
            TabCellColor::Dragged,
            "dragged but not hovered is Dragged, not DraggedHovered"
        );
        assert_eq!(
            dragged_away.row_paints[0].background,
            palette.get(Slot::BackgroundElement),
            "dragged-away still lifts to raised.high"
        );
        assert_eq!(
            dragged_away.row_paints[0].title_attr, 1,
            "dragged-away is still BOLD (:1642)"
        );
        assert_eq!(
            dragged_away.row_paints[0].title_fg,
            palette.get(Slot::TextMuted),
            "dragged-away title fg = foreground() = tint(text.muted, text.base, 0) = text.muted (:1609-1612)"
        );
        assert_ne!(
            dragged_away.row_paints[0].number_fg,
            palette.get(Slot::Text),
            "dragged-but-not-hovered number is the idle formula, NOT text.base"
        );
        // A frame with the drag released repaints the source as plain hovered.
        let released = render(None);
        assert_eq!(released.row_paints[1].color, TabCellColor::Hovered);
        assert_eq!(released.row_paints[1].title_attr, 0);
        // A dragged title also stays BOLD when the TAB-24 renaming stamp runs.
        let renamed = TabStrip::render_with_hover(
            &[
                input("idle", "idle"),
                hovered_tab.clone(),
                selected_tab.clone(),
            ],
            TabOrientation::Horizontal,
            120,
            1,
            0,
            Some(&SessionRef("hov".into())),
        )
        .with_dragged(Some(&SessionRef("hov".into())))
        .with_row_colors(&palette)
        .with_title_renaming(&|_| false, true);
        assert_eq!(renamed.row_paints[1].title_attr, 1, "renaming stamp keeps drag BOLD");
    }

    /// TAB-08 fixture (`marquee-*` long title): the exact fixture title with
    /// CJK and a skin-tone ZWJ emoji stays grapheme-aware, and the FADE_WIDTH=4
    /// window ramps both edges of a 10-cluster title.
    #[test]
    fn tab_08_fixture_long_title_is_grapheme_aware() {
        let title = "A very long session title with 中文 and 👩🏽\u{200d}💻";
        assert_eq!(display_width("中文"), 4, "two CJK glyphs are four cells");
        assert!(marquee_overflows(title, 17));
        // Every 4-cell window consumes <= 4 cells and never emits an empty part.
        for offset in 0..marquee_cycle_width(title) {
            let parts = marquee_text_parts(title, 4, offset);
            assert!(parts.iter().map(|p| display_width(&p.value)).sum::<usize>() <= 4);
            assert!(parts.iter().all(|p| !p.value.is_empty()));
        }
        // Four-cell fade ramps both edges and keeps the middle opaque.
        let weights: Vec<f32> = (0..10).map(|i| title_fade_weight(i, 10, 1.0)).collect();
        assert!((weights[0] - 0.92).abs() < 1e-6);
        assert!(weights[4].abs() < 1e-6);
        assert!((weights[9] - 0.92).abs() < 1e-6);
    }

    /// TAB-09 fixture phases (`marquee-0/599/600/680`): nothing before the
    /// 600 ms delay, the first cell at exactly 600, then one 80 ms step at 680.
    #[test]
    fn tab_09_fixture_clock_phases_wait_then_step() {
        let title = "A very long session title with 中文 and 👩🏽\u{200d}💻";
        let mut marquee = Marquee::new();
        marquee.enter("s", title, 17, 0);
        assert_eq!(marquee.offset(), 0, "clock_ms 0");
        assert!(!marquee.advance(599), "clock_ms 599 still waits");
        assert_eq!(marquee.offset(), 0);
        assert!(marquee.advance(600), "clock_ms 600 starts the cycle");
        assert_eq!(marquee.offset(), 1);
        assert!(marquee.advance(680), "clock_ms 680 is one 80 ms step");
        assert_eq!(marquee.offset(), 2);
    }

    #[test]
    fn tab_08_four_cell_fade_and_grapheme_marquee() {
        // session-tabs.tsx:74 FADE_WIDTH = 4 and :221-228 fadeTitleColor.
        // A 10-cell title keeps its middle opaque and ramps both 4-cell edges
        // toward the background.
        let weights: Vec<f32> = (0..10).map(|i| title_fade_weight(i, 10, 1.0)).collect();
        assert!((weights[0] - 0.92).abs() < 0.001);
        assert!((weights[1] - 0.68).abs() < 0.001);
        assert!((weights[2] - 0.44).abs() < 0.001);
        assert!((weights[3] - 0.20).abs() < 0.001);
        assert!((weights[4] - 0.0).abs() < 0.001);
        assert!((weights[9] - 0.92).abs() < 0.001);
        assert_eq!(FADE_WIDTH, 4);
        // The leading fade scales the left edge while the marquee runs; the
        // trailing ramp follows the title end independently.
        assert!((title_fade_weight(0, 10, 0.0) - 0.0).abs() < 0.001);
        assert!((title_fade_weight(0, 10, 0.5) - 0.46).abs() < 0.001);

        // util/marquee.ts:21-50 is grapheme-aware for CJK, emoji ZWJ,
        // combining accents, Tamil and long paths.
        assert_eq!(marquee_cycle_width("東京abc"), 10, "7 text cells + 3 gap");
        assert_eq!(marquee_text("東京abc", 4, 0), "東京");
        assert_eq!(marquee_text("東京abc", 4, 1), "京ab");
        let combining = "e\u{301}x";
        assert_eq!(marquee_cycle_width(combining), 5, "e+accent is one cluster");
        let emoji = "👩‍💻x";
        assert_eq!(marquee_cycle_width(emoji), 6, "ZWJ cluster counts once");
        for value in ["東京abc", combining, emoji, "தமிழ்", "a/very/long/path/name"] {
            for offset in 0..marquee_cycle_width(value) {
                let parts = marquee_text_parts(value, 4, offset);
                assert!(
                    parts
                        .iter()
                        .map(|part| display_width(&part.value))
                        .sum::<usize>()
                        <= 4
                );
                assert!(parts.iter().all(|part| !part.value.is_empty()));
            }
        }
        assert!(marquee_overflows("東京abc", 6));
        assert!(!marquee_overflows("東京abc", 7));
    }

    #[test]
    fn tab_09b_marquee_wakes_on_the_delay_then_eighty_millisecond_steps() {
        // session-tabs.tsx:79-80 MARQUEE_DELAY = 600, MARQUEE_INTERVAL = 80.
        let mut marquee = Marquee::new();
        marquee.enter("s", "0123456789abcdefghij", 4, 1_000);
        assert_eq!(marquee.next_wake(), Some(1_600), "delay boundary, not an 80 ms multiple");
        assert!(!marquee.advance(1_599));
        assert_eq!(marquee.offset(), 0);
        assert!(marquee.advance(1_600));
        assert_eq!(marquee.offset(), 1);
        assert!(marquee.leading(), "the leading fade turns on with the first step");
        assert_eq!(marquee.next_wake(), Some(1_680));
        assert!(marquee.advance(1_680));
        assert_eq!(marquee.offset(), 2);
        // cycleWidth = 20 title cells + " · " = 23; the cycle stops after 22.
        assert!(marquee.advance(3_360));
        assert_eq!(marquee.active(), None, "exactly one cycle");
        assert_eq!(marquee.offset(), 0);
        assert_eq!(marquee.next_wake(), None);

        // Leaving mid-cycle resets immediately; re-entering restarts the delay.
        marquee.enter("s", "0123456789abcdefghij", 4, 1_000);
        assert!(marquee.advance(1_600));
        marquee.leave("s");
        assert_eq!(marquee.active(), None);
        assert_eq!(marquee.next_wake(), None);
    }

    #[test]
    fn tab_09c_strip_reports_the_exact_title_budget_it_paints() {
        let inputs = vec![input("a", "alpha"), input("b", "beta")];
        let strip = TabStrip::render_with_hover(
            &inputs,
            TabOrientation::Horizontal,
            90,
            1,
            0,
            Some(&inputs[0].session),
        );
        let slot = strip
            .rects
            .iter()
            .find(|(id, _)| *id == inputs[0].session)
            .unwrap()
            .1
            .width;
        let gauge = strip.indicator_cells;
        assert_eq!(gauge, number_width(2) + 1);
        assert_eq!(
            strip.title_width(&inputs[0].session, false),
            Some(slot - gauge),
            "resting titles keep the whole slot after the indicator column"
        );
        assert_eq!(
            strip.title_width(&inputs[0].session, true),
            Some(slot - gauge - 2),
            "the hovered close child reserves two cells"
        );
        assert_eq!(
            strip.title_width(&SessionRef("gone".into()), true),
            None,
            "a tab outside the painted window has no budget"
        );
    }

    #[test]
    fn tab_08b_rows_report_the_exact_fade_window() {
        // session-tabs.tsx:764,870: the fade only exists while the title
        // overflows the exact painted title budget, and its window is the
        // visible cluster count, not the padded slot width.
        let mut long = input("a", "abcdefghijklmnopqrst");
        let inputs = vec![long.clone(), input("b", "beta")];
        let strip = TabStrip::render(&inputs, TabOrientation::Horizontal, 40, 1, 0);
        let fade = strip.row_fades[0].expect("20 cells overflow a 17-cell budget");
        assert_eq!((fade.start, fade.length), (0, 17));
        assert!(!fade.leading);
        assert_eq!(fade.weight(0), 0.0, "at rest the leading ramp is disabled");
        assert_eq!(fade.weight(12), 0.0, "the middle of the title stays opaque");
        assert!((fade.weight(13) - 0.2).abs() < 1e-6);
        assert!((fade.weight(16) - 0.92).abs() < 1e-6);
        assert_eq!(strip.row_fades[1], None, "a fitting title has no fade");

        let hovered = TabStrip::render_with_hover(
            &inputs,
            TabOrientation::Horizontal,
            40,
            1,
            0,
            Some(&inputs[0].session),
        );
        let fade = hovered.row_fades[0].expect("hovered close shrinks the budget");
        assert_eq!((fade.start, fade.length), (0, 15));

        // A scrolling marquee keeps the visible-cluster window and turns the
        // leading ramp on, so both edges fade.
        long.marquee_offset = 5;
        let scrolling = TabStrip::render(&[long, input("b", "beta")], TabOrientation::Horizontal, 40, 1, 0);
        let fade = scrolling.row_fades[0].expect("the marquee window still fades");
        assert_eq!((fade.start, fade.length), (0, 17));
        assert!(fade.leading);
        assert!((fade.weight(0) - 0.92).abs() < 1e-6);
        assert_eq!(fade.weight(4), 0.0);
        assert!((fade.weight(16) - 0.92).abs() < 1e-6);
    }

    #[test]
    fn tab_09_marquee_clock_waits_advances_and_stops_after_one_cycle() {
        // session-tabs.tsx:77-78 MARQUEE_DELAY = 600, MARQUEE_INTERVAL = 80;
        // :255-270 waits, advances by one cell per interval, then clears.
        let title = "abcdef"; // 6 cells + 3 gap = 9-cell cycle
        assert_eq!(marquee_cycle_width(title), 9);
        let mut marquee = Marquee::new();
        marquee.enter("a", title, 4, 0);
        assert_eq!(marquee.active(), Some("a"));
        assert_eq!(marquee.offset(), 0);
        assert!(!marquee.advance(599));
        assert_eq!(marquee.offset(), 0, "nothing moves before 600 ms");
        assert!(marquee.advance(600));
        assert_eq!(marquee.offset(), 1);
        assert!(marquee.leading());
        assert!(marquee.advance(680));
        assert_eq!(marquee.offset(), 2, "80 ms per cell");
        assert!(!marquee.advance(681), "no motion between ticks");
        let mut now = 680;
        let mut max = marquee.offset();
        while marquee.active().is_some() && now < 10_000 {
            now += MARQUEE_INTERVAL_MS;
            marquee.advance(now);
            max = max.max(marquee.offset());
        }
        assert_eq!(max, 8, "one cycle ends at cycleWidth - 1");
        assert_eq!(marquee.active(), None, "stops after one cycle");
        assert_eq!(marquee.offset(), 0);

        // Re-entering the running tab keeps its offset; leaving resets; a
        // fitting title never starts.
        marquee.enter("a", title, 4, 0);
        marquee.advance(600);
        marquee.enter("a", title, 4, 700);
        assert_eq!(marquee.offset(), 1);
        marquee.leave("b");
        assert_eq!(marquee.active(), Some("a"));
        marquee.leave("a");
        assert_eq!(marquee.active(), None);
        marquee.enter("a", "abc", 4, 0);
        marquee.advance(2_000);
        assert_eq!(marquee.active(), None, "a fitting title resets");

        // TabMarquee cancels a leave scheduled for the same tick when the
        // pointer re-enters, matching the hover-clear timeout.
        let mut hover = TabMarquee::new();
        hover.enter("a", title, 4, 0);
        hover.leave("a", 0);
        hover.enter("a", title, 4, 0);
        assert_eq!(hover.hovered(), Some("a"));
        hover.advance(0);
        assert_eq!(hover.marquee().active(), Some("a"));
        hover.leave_hovered(5);
        hover.advance(5);
        assert_eq!(hover.hovered(), None);
        assert_eq!(hover.marquee().active(), None);
    }

    #[test]
    fn tab_13_pointer_press_release_controls_and_hidden_close_cell() {
        // session-tabs.tsx:1589-1604: close is hover-gated; 923-947 middle and
        // right press are separate controls.
        let tabs = vec![input("a", "alpha")];
        let strip = TabStrip::render(&tabs, TabOrientation::Horizontal, 20, 1, 0);
        let close_cell = strip.close_rects[0].1;
        let action = strip
            .mouse_action(close_cell.x, close_cell.y)
            .expect("session target");
        assert!(!action.close, "an unhovered close cell selects the tab");
        let hovered = TabStrip::render_with_hover(
            &tabs,
            TabOrientation::Horizontal,
            20,
            1,
            0,
            Some(&SessionRef("a".into())),
        );
        assert!(
            hovered
                .mouse_action(close_cell.x, close_cell.y)
                .expect("session target")
                .close,
            "the visible close cell closes"
        );
        let mut gesture = PointerGesture::new();
        gesture.press(TabPointerTarget::Session);
        assert_eq!(
            gesture.release(TabPointerTarget::Session),
            Some(TabPointerTarget::Session)
        );
        gesture.press(TabPointerTarget::Close);
        assert_eq!(
            gesture.release(TabPointerTarget::Close),
            Some(TabPointerTarget::Close)
        );
        gesture.press(TabPointerTarget::Session);
        gesture.drag();
        assert_eq!(gesture.release(TabPointerTarget::Session), None);
    }

    #[test]
    fn tab_14_selection_release_over_add_search_and_close_is_inert() {
        // A transcript selection press starts on Transcript; releasing over
        // the plus/search/close never activates that control.
        for up in [
            TabPointerTarget::Add,
            TabPointerTarget::Search,
            TabPointerTarget::Close,
            TabPointerTarget::Overflow,
        ] {
            let mut gesture = PointerGesture::new();
            gesture.press(TabPointerTarget::Transcript);
            assert_eq!(gesture.release(up), None, "release over {up:?}");
        }
        let tabs = vec![input("a", "alpha")];
        let strip = TabStrip::render_with_hover_and_add(
            &tabs,
            TabOrientation::Horizontal,
            20,
            1,
            0,
            None,
            true,
        );
        let add = strip.add_rect.unwrap();
        assert!(strip.mouse_action(add.x + 1, 0).is_none());
        assert!(strip.hit(add.x + 1, 0).is_none());
    }

    #[test]
    fn tab_15_drag_preview_persists_once_and_suppresses_synthetic_release() {
        // session-tabs.tsx:598-614,650-660: one preview move, persisted on
        // release, with the captured-drag synthetic click suppressed.
        let ids: Vec<SessionRef> = ["a", "b", "c"]
            .into_iter()
            .map(|id| SessionRef(id.into()))
            .collect();
        let mut drag = DragState::new();
        drag.press(&ids[0], 0);
        drag.drag_to(2, ids.len());
        drag.drag_to(2, ids.len());
        assert_eq!(
            drag.preview().map(|(session, index)| (session.0.as_str(), index)),
            Some(("a", 2)),
            "repeated reports keep one preview"
        );
        let order: Vec<String> = drag.ordered(&ids).iter().map(|s| s.0.clone()).collect();
        assert_eq!(order, ["b", "c", "a"]);
        let release = drag.release();
        assert_eq!(release.select, Some(SessionRef("a".into())));
        assert_eq!(release.move_to, Some(2), "exactly one persisted move");
        assert!(drag.suppress_click());
        // The preview holds until the store reflects the move.
        assert_eq!(
            drag.preview().map(|(session, index)| (session.0.as_str(), index)),
            Some(("a", 2))
        );
        drag.settle(&ids);
        assert_eq!(
            drag.preview().map(|(session, index)| (session.0.as_str(), index)),
            Some(("a", 2))
        );
        drag.settle(&[SessionRef("b".into()), SessionRef("c".into()), SessionRef("a".into())]);
        assert_eq!(drag.preview(), None);
        drag.clear_suppress();
        assert!(!drag.suppress_click());
        // A second release cannot move again.
        assert_eq!(drag.release().move_to, None);
    }

    #[test]
    fn tab_15b_drag_preview_projects_into_the_rendered_input_order() {
        let ids: Vec<SessionRef> = ["a", "b", "c"]
            .into_iter()
            .map(|id| SessionRef(id.into()))
            .collect();
        let mut inputs = vec![input("a", "alpha"), input("b", "beta"), input("c", "gamma")];
        let mut drag = DragState::new();
        drag.project(&mut inputs, &ids);
        let order = |inputs: &[TabInput]| {
            inputs.iter().map(|tab| tab.session.0.clone()).collect::<Vec<_>>()
        };
        assert_eq!(order(&inputs), ["a", "b", "c"], "no preview leaves the snapshot alone");
        drag.press(&ids[0], 0);
        drag.drag_to(2, ids.len());
        drag.project(&mut inputs, &ids);
        assert_eq!(
            order(&inputs),
            ["b", "c", "a"],
            "the painted snapshot follows the pending move",
        );
    }

    #[test]
    fn tab_16_close_hold_keeps_pointer_close_cell_until_leave_or_timeout() {
        // session-tabs.tsx:229-268 heldSessionTabLayout: the pointer keeps its
        // physical close cell through the overflow change; leave or 5 s ends it.
        let ids: Vec<SessionRef> = ["a", "b", "c"]
            .into_iter()
            .map(|id| SessionRef(id.into()))
            .collect();
        let hold = CloseHold::new(
            &ids,
            &ids,
            &[10, 10, 10],
            &SessionRef("b".into()),
            &SessionRef("c".into()),
            18, // c starts at 10, its close cell is start + width - 2 = 18
            0,
        );
        let after: Vec<SessionRef> = [SessionRef("a".into()), SessionRef("c".into())].to_vec();
        let held = hold.layout(&after).expect("held layout");
        assert_eq!(held.tabs, after);
        assert_eq!(held.widths, vec![10, 10], "the close cell stays under the pointer");
        assert_eq!(held.total, 20);
        assert_eq!(held.start, 0);
        assert!(!hold.expired(4_999));
        assert!(hold.expired(MOUSE_CLOSE_HOLD_MS));
        // A layout change that is neither the unchanged list nor the exact
        // removal invalidates the hold.
        assert_eq!(
            hold.layout(&[SessionRef("a".into()), SessionRef("x".into())]),
            None
        );
        // The pre-close list (no removal yet) keeps the original widths.
        assert_eq!(hold.layout(&ids).unwrap().widths, vec![10, 10, 10]);
        // A hold whose signed target width is below one is refused, not
        // saturated into a fabricated two-cell tab (`session-tabs.tsx:263-264`).
        let impossible = CloseHold::new(
            &ids,
            &ids,
            &[10, 10, 10],
            &ids[1],
            &ids[2],
            5, // x far left of a's two leading cells: width = 5 - 10 + 2 < 1
            0,
        );
        assert_eq!(impossible.layout(&after), None, "sub-one width refuses the hold");
        // Render consumption (TAB-16): the held layout replaces the adaptive
        // width solve, so the painted slots keep the held widths and the
        // target's close cell stays at the pointer column x=18.
        let inputs: Vec<TabInput> = after.iter().map(|id| input(&id.0, "tab")).collect();
        let strip = TabStrip::render_with_held(
            &inputs,
            TabOrientation::Horizontal,
            80,
            1,
            0,
            None,
            false,
            false,
            false,
            Some(&held),
        );
        assert_eq!(
            strip.rects.iter().map(|(_, rect)| rect.width).collect::<Vec<_>>(),
            vec![10, 10],
            "painted slots take the held widths"
        );
        assert_eq!(strip.before, 0);
        let target_x = strip
            .rects
            .iter()
            .find(|(id, _)| id == &SessionRef("c".into()))
            .map(|(_, rect)| rect.x)
            .expect("target painted");
        assert_eq!(target_x + 10 - 2, 18, "target close cell held under the pointer");
    }

    #[test]
    fn tab_24_reduced_animation_preserves_status_and_dims_renaming_title() {
        // session-tabs.tsx:1030-1041: renaming + animations=false -> DIM;
        // reduced motion does not change status facts or attention glow.
        let status = TabStatus {
            attention: Some(TabAttention::Permission),
            busy: true,
            unread: Some(TabUnread::Error),
            renaming: true,
            prompt_pulse: 3,
        };
        assert!(status.glows(false));
        assert_eq!(
            indicator_text(
                "2",
                status,
                TabIndicators::Status,
                false,
                TabSpinner::Dots,
                TabUnreadMarker::SmallDot,
                ""
            ),
            "!",
            "status is preserved under reduced motion"
        );
        assert_eq!(
            title_attributes(true, true, false),
            crate::native_ui_style::ATTR_DIM
        );
        assert_eq!(
            title_attributes(true, false, false),
            crate::native_ui_style::ATTR_BOLD
        );
        assert_eq!(title_attributes(false, true, true), 0);
        // The deterministic clock can replay every full-motion phase.
        let mut marquee = Marquee::new();
        marquee.enter("a", "abcdef", 4, 0);
        let mut frames = Vec::new();
        for tick in 0..=9 {
            marquee.advance(600 + tick * MARQUEE_INTERVAL_MS);
            frames.push(marquee.offset());
        }
        assert_eq!(frames, vec![1, 2, 3, 4, 5, 6, 7, 8, 0, 0]);
    }

    #[test]
    fn tab_24_strip_applies_renaming_dim_only_with_animations_off() {
        // session-tabs.tsx:1061-1074: the renaming title is DIM only while
        // animations are disabled; a selected non-renaming tab is BOLD.
        use crate::native_theme_runtime::Palette;
        let palette = Palette::opencode_dark();
        let selected = || {
            let mut tab = input("s", "selected");
            tab.flags = TabFlags { active: true, busy: false, needs_attention: false };
            tab
        };
        let base = TabStrip::render_with_hover(
            &[selected(), input("i", "idle")],
            TabOrientation::Horizontal,
            80,
            1,
            0,
            None,
        )
        .with_row_colors(&palette);
        assert_eq!(base.row_paints[0].title_attr, crate::native_ui_style::ATTR_BOLD);
        assert_eq!(base.row_paints[1].title_attr, 0);

        // Renaming the selected tab with animations OFF dims it; the selected
        // non-renaming tab stays BOLD.
        let dim = TabStrip::render_with_hover(
            &[selected(), input("i", "idle")],
            TabOrientation::Horizontal,
            80,
            1,
            0,
            None,
        )
        .with_row_colors(&palette)
        .with_title_renaming(&|session: &SessionRef| session == &SessionRef("s".into()), false);
        assert_eq!(dim.row_paints[0].title_attr, crate::native_ui_style::ATTR_DIM);
        assert_eq!(dim.row_paints[1].title_attr, 0);

        // Renaming with animations ON is NOT dimmed: the selected tab keeps
        // BOLD (title_shimmer consumes `renaming && !animations`).
        let bold = TabStrip::render_with_hover(
            &[selected(), input("i", "idle")],
            TabOrientation::Horizontal,
            80,
            1,
            0,
            None,
        )
        .with_row_colors(&palette)
        .with_title_renaming(&|session: &SessionRef| session == &SessionRef("s".into()), true);
        assert_eq!(bold.row_paints[0].title_attr, crate::native_ui_style::ATTR_BOLD);
    }

    #[test]
    fn tab_24_pending_title_keeps_the_animation_clock_live_only_with_motion() {
        // component/title-shimmer.tsx:50-55: `live = animating` and
        // `shimmering = rename.pending` while `enabled`, so the render loop
        // keeps ticking for a pending title only when motion is enabled.
        assert!(title_shimmer_runs(true, true), "pending + motion keeps the clock live");
        assert!(!title_shimmer_runs(true, false), "pending + reduced motion is a static DIM frame");
        assert!(!title_shimmer_runs(false, true), "an idle title never animates");
        assert!(!title_shimmer_runs(false, false));
    }

    #[test]
    fn tab_24_generated_title_pending_shares_the_same_clock_as_rename() {
        // context/data.tsx:35 keys one pending map for both the explicit rename
        // and the generated-title sources, so either one keeps the frame clock
        // live while motion is enabled (title-shimmer.tsx:50-55).
        assert!(title_shimmer_runs(title_pending(true, false), true));
        assert!(title_shimmer_runs(title_pending(false, true), true));
        assert!(!title_shimmer_runs(title_pending(false, false), true));
        assert!(!title_shimmer_runs(title_pending(true, true), false));
    }

    #[test]
    fn tab_05_running_child_marks_the_parent_tab_busy() {
        use crate::native_navigation::{DaemonEvent, NavigationState, WorkState};
        let mut nav = NavigationState::new();
        nav.open_tab(SessionRef("parent".into()));
        nav.open_forked(SessionRef("child".into()), SessionRef("parent".into()));
        // The child runs; the parent has no work state of its own.
        nav.apply(DaemonEvent::WorkState {
            session: SessionRef("child".into()),
            state: WorkState::Running,
        });
        let inputs = TabStrip::from_navigation(&nav, &[]);
        let parent = inputs.iter().find(|i| i.session == SessionRef("parent".into())).unwrap();
        let child = inputs.iter().find(|i| i.session == SessionRef("child".into())).unwrap();
        assert!(parent.flags.busy, "a running family member marks the root tab busy");
        assert!(child.flags.busy);
        // An unrelated tab is unaffected.
        nav.open_tab(SessionRef("other".into()));
        let inputs = TabStrip::from_navigation(&nav, &[]);
        let other = inputs.iter().find(|i| i.session == SessionRef("other".into())).unwrap();
        assert!(!other.flags.busy);
    }

    /// TAB-05: attention aggregates across the whole family
    /// (`context/session-tabs.tsx:163-171` scans every `family(session)`
    /// member). A pending permission/question on a descendant wakes the root
    /// tab's indicator, and permission outranks question across the family.
    #[test]
    fn tab_05_child_attention_wakes_the_root_tab() {
        use crate::native_navigation::NavigationState;
        let mut nav = NavigationState::new();
        nav.open_tab(SessionRef("parent".into()));
        nav.open_forked(SessionRef("child".into()), SessionRef("parent".into()));

        let inputs = TabStrip::from_navigation(
            &nav,
            &[Attention {
                session: SessionRef("child".into()),
                needs_attention: true,
                kind: TabAttention::Question,
            }],
        );
        let parent = inputs.iter().find(|i| i.session == SessionRef("parent".into())).unwrap();
        let child = inputs.iter().find(|i| i.session == SessionRef("child".into())).unwrap();
        assert!(parent.flags.needs_attention, "a descendant's attention wakes the root tab");
        assert_eq!(parent.attention, Some(TabAttention::Question));
        assert_eq!(child.attention, Some(TabAttention::Question));

        // Permission on the child outranks a question anywhere in the family.
        let inputs = TabStrip::from_navigation(
            &nav,
            &[
                Attention {
                    session: SessionRef("parent".into()),
                    needs_attention: true,
                    kind: TabAttention::Question,
                },
                Attention {
                    session: SessionRef("child".into()),
                    needs_attention: true,
                    kind: TabAttention::Permission,
                },
            ],
        );
        let parent = inputs.iter().find(|i| i.session == SessionRef("parent".into())).unwrap();
        assert_eq!(
            parent.attention,
            Some(TabAttention::Permission),
            "permission outranks question across the family"
        );

        // An unrelated tab stays idle.
        nav.open_tab(SessionRef("other".into()));
        let inputs = TabStrip::from_navigation(
            &nav,
            &[Attention {
                session: SessionRef("child".into()),
                needs_attention: true,
                kind: TabAttention::Question,
            }],
        );
        let other = inputs.iter().find(|i| i.session == SessionRef("other".into())).unwrap();
        assert!(!other.flags.needs_attention);
        assert_eq!(other.attention, None);
    }

    /// TAB-04: the live strip paints the attention *kind* — `!` for a
    /// permission and `?` for a question (`component/session-tabs.tsx:172-174`),
    /// with permission outranking question for the same tab
    /// (`context/session-tabs.tsx:165-169`). Before the kind was carried on the
    /// input, only `!` was reachable.
    #[test]
    fn tab_04_live_strip_question_glyph() {
        use crate::native_navigation::NavigationState;
        let mut nav = NavigationState::new();
        nav.open_tab(SessionRef("a".into()));
        let session = SessionRef("a".into());

        let question = TabStrip::from_navigation(
            &nav,
            &[Attention {
                session: session.clone(),
                needs_attention: true,
                kind: TabAttention::Question,
            }],
        );
        assert_eq!(question[0].attention, Some(TabAttention::Question));
        assert_eq!(
            indicator_label(&question[0].flags, question[0].attention, false, false),
            "?"
        );
        let strip = TabStrip::render_with_hover_and_add(
            &question,
            TabOrientation::Horizontal,
            24,
            1,
            0,
            None,
            true,
        );
        assert!(strip.rows[0].contains('?'), "question paints ?: {:?}", strip.rows[0]);

        let permission = TabStrip::from_navigation(
            &nav,
            &[Attention {
                session: session.clone(),
                needs_attention: true,
                kind: TabAttention::Permission,
            }],
        );
        assert_eq!(permission[0].attention, Some(TabAttention::Permission));
        assert_eq!(
            indicator_label(&permission[0].flags, permission[0].attention, false, false),
            "!"
        );

        // Permission outranks question when both facts name the same tab.
        let both = TabStrip::from_navigation(
            &nav,
            &[
                Attention {
                    session: session.clone(),
                    needs_attention: true,
                    kind: TabAttention::Question,
                },
                Attention {
                    session: session.clone(),
                    needs_attention: true,
                    kind: TabAttention::Permission,
                },
            ],
        );
        assert_eq!(both[0].attention, Some(TabAttention::Permission));

        // No attention fact: the tab carries no kind and paints idle.
        let idle = TabStrip::from_navigation(&nav, &[]);
        assert_eq!(idle[0].attention, None);
        assert_eq!(indicator_label(&idle[0].flags, idle[0].attention, false, false), "");
    }

    /// TAB-07: a background completion marked unread on the live
    /// `NavigationState` projects into the strip as the accent small-dot marker
    /// (`session-tabs.tsx:63-70,125-129,176-178,958-963`): the live
    /// `from_navigation` -> `with_row_colors_semantic` path paints "•" in the
    /// indicator column and colors it the accent `unreadColor`, not idleNumber.
    #[test]
    fn tab_07_live_unread_projects_marker_and_accent() {
        use crate::native_navigation::NavigationState;
        use crate::native_theme_runtime::{Palette, Slot};
        let palette = Palette::opencode_dark();
        let mut nav = NavigationState::new();
        nav.open_tab(SessionRef("a".into()));
        nav.open_tab(SessionRef("b".into()));
        nav.mark_unread(&SessionRef("b".into()));
        // Focusing a route views it (`context/session-tabs.tsx:229-267`), so
        // re-open "a" to make "b" an unread, non-selected background tab.
        nav.open_tab(SessionRef("a".into()));

        let inputs = TabStrip::from_navigation(&nav, &[]);
        let unread = inputs.iter().find(|i| i.session == SessionRef("b".into())).unwrap();
        assert!(unread.unread, "live unread fact projects onto the tab input");
        assert!(!inputs.iter().find(|i| i.session == SessionRef("a".into())).unwrap().unread);

        let strip = TabStrip::render_with_hover(&inputs, TabOrientation::Horizontal, 40, 1, 0, None)
            .with_row_colors_semantic(&palette, None);
        let index = strip.rects.iter().position(|(id, _)| id == &SessionRef("b".into())).unwrap();
        assert!(
            strip.rows[index].contains('\u{2022}'),
            "unread tab paints the small-dot marker: {:?}",
            strip.rows[index]
        );
        assert_eq!(
            strip.row_paints[index].number_fg,
            palette.get(Slot::Warning),
            "activity unread uses the accent unreadColor fallback, not idleNumber()"
        );
        assert_ne!(
            strip.row_paints[index].number_fg,
            crate::native_tab_rail::idle_number(&palette)
        );
    }

    /// TAB-07 live-path color: when a built-in V2 semantic palette is present,
    /// the unread activity marker resolves to `semantic.tab_attention` (the
    /// `hue.accent[200]` `unreadColor`, `session-tabs.tsx:125-129,565`).
    ///
    /// Coverage note: this exercises the `from_navigation` -> strip builder
    /// (`with_row_colors_semantic`) color resolution only. The live painter
    /// (`tui_entry.rs` tab arm) calls the same builder with
    /// `ctx.semantic.as_ref()`, but the bridge `Renderer` exposes no styled-cell
    /// readback, so a revert of that call site itself cannot be observed here.
    #[test]
    fn tab_07_live_unread_uses_semantic_tab_attention() {
        use crate::native_navigation::NavigationState;
        use crate::native_theme_runtime::{Palette, RuntimeTheme, ThemeMode, Slot};
        let theme = RuntimeTheme::builtin_default(ThemeMode::Dark);
        let palette = Palette::opencode_dark();
        let semantic = theme.semantic.as_ref().expect("built-in V2 semantic palette");
        let mut nav = NavigationState::new();
        nav.open_tab(SessionRef("a".into()));
        nav.open_tab(SessionRef("b".into()));
        nav.mark_unread(&SessionRef("b".into()));
        nav.open_tab(SessionRef("a".into()));
        let inputs = TabStrip::from_navigation(&nav, &[]);
        let strip = TabStrip::render_with_hover(&inputs, TabOrientation::Horizontal, 40, 1, 0, None)
            .with_row_colors_semantic(&palette, theme.semantic.as_ref());
        let index = strip.rects.iter().position(|(id, _)| id == &SessionRef("b".into())).unwrap();
        assert_eq!(
            strip.row_paints[index].number_fg,
            semantic.tab_attention.rgba,
            "live semantic path paints the unread marker with hue.accent[200]"
        );
        assert_ne!(strip.row_paints[index].number_fg, palette.get(Slot::Warning));
    }

    /// TAB-07 live-path color: an errored turn (`outcome === "failed"`,
    /// `context/session-tabs.tsx:160-162`) paints the unread indicator
    /// `theme.text.feedback.error.base` (`session-tabs.tsx:125-130`), not the
    /// activity accent. Plain activity-unread keeps the accent, and a
    /// busy/attention tab never takes the unread tint at all.
    #[test]
    fn tab_07_error_unread_paints_feedback_error_not_activity_accent() {
        use crate::native_navigation::NavigationState;
        use crate::native_theme_runtime::{Palette, RuntimeTheme, ThemeMode, Slot};
        let theme = RuntimeTheme::builtin_default(ThemeMode::Dark);
        let palette = Palette::opencode_dark();
        let semantic = theme.semantic.as_ref().expect("built-in V2 semantic palette");
        let mut nav = NavigationState::new();
        nav.open_tab(SessionRef("a".into()));
        nav.open_tab(SessionRef("b".into()));
        nav.open_tab(SessionRef("c".into()));
        nav.mark_unread_failed(&SessionRef("b".into()));
        nav.mark_unread(&SessionRef("c".into()));
        // Focus "a" so "b" and "c" are unviewed background tabs.
        nav.open_tab(SessionRef("a".into()));

        let inputs = TabStrip::from_navigation(&nav, &[]);
        let failed = inputs.iter().find(|i| i.session == SessionRef("b".into())).unwrap();
        assert!(failed.unread_error && failed.unread, "failed turn carries both marks");
        let activity = inputs.iter().find(|i| i.session == SessionRef("c".into())).unwrap();
        assert!(!activity.unread_error && activity.unread, "activity turn has no failed mark");

        let strip = TabStrip::render_with_hover(&inputs, TabOrientation::Horizontal, 60, 1, 0, None)
            .with_row_colors_semantic(&palette, theme.semantic.as_ref());
        let failed_index = strip.rects.iter().position(|(id, _)| id == &SessionRef("b".into())).unwrap();
        let activity_index = strip.rects.iter().position(|(id, _)| id == &SessionRef("c".into())).unwrap();
        assert_eq!(
            strip.row_paints[failed_index].number_fg,
            semantic.text_feedback_error.rgba,
            "failed outcome paints text.feedback.error.base"
        );
        assert_ne!(strip.row_paints[failed_index].number_fg, semantic.tab_attention.rgba);
        assert_eq!(
            strip.row_paints[activity_index].number_fg,
            semantic.tab_attention.rgba,
            "activity-unread keeps the accent unreadColor"
        );
        // Legacy palette fallback: Slot::Error, not Warning.
        let legacy = TabStrip::render_with_hover(&inputs, TabOrientation::Horizontal, 60, 1, 0, None)
            .with_row_colors_semantic(&palette, None);
        assert_eq!(
            legacy.row_paints[failed_index].number_fg,
            palette.get(Slot::Error),
            "legacy fallback resolves the error slot"
        );
    }
}
