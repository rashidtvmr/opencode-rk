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
    pub const ADD_TAB_WIDTH: u16 = 3;

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
        match orientation {
            TabOrientation::Horizontal if rows > 0 => Self {
                orientation: Some(TabOrientation::Horizontal),
                width: cols,
                height: 1,
                ..Self::default()
            },
            TabOrientation::Vertical
                if rows > 0 && cols >= Self::VERTICAL_WIDTH + Self::CONTENT_PREFERRED_WIDTH =>
            {
                Self {
                    orientation: Some(TabOrientation::Vertical),
                    width: Self::VERTICAL_WIDTH,
                    height: rows,
                    ..Self::default()
                }
            }
            TabOrientation::Vertical => Self::for_view(TabOrientation::Horizontal, cols, rows),
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

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct TabStrip {
    /// Painted add-session control, independent of session hit rectangles.
    pub add_rect: Option<TabRect>,
    pub orientation: TabOrientation,
    pub available_width: u16,
    pub rows: Vec<String>,
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
        if let Some(rect) = &mut self.add_rect {
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
    /// The tab layout receives only the space left of the always-visible button.
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
        let mut out = Self {
            orientation,
            add_rect: None,
            available_width: width,
            rows: Vec::new(),
            rects: Vec::new(),
            close_rects: Vec::new(),
            overflow_before: false,
            overflow_after: false,
            scroll: 0,
            hovered: hovered.cloned(),
            visible_close_session: None,
        };
        if width == 0 || height == 0 {
            return out;
        }
        let horizontal = matches!(orientation, TabOrientation::Horizontal);
        let available = if horizontal && show_add {
            width.saturating_sub(TabReservation::ADD_TAB_WIDTH)
        } else {
            match orientation { TabOrientation::Horizontal => width, TabOrientation::Vertical => height }
        };
        let active = inputs.iter().position(|tab| tab.flags.active);
        // The adaptive model governs the horizontal SessionTabs strip.  A
        // rail is a one-row-per-tab list: its extent is height, while its
        // cell width remains the complete rail width.
        let layout_available = available;
        let solved = if horizontal {
            adaptive_layout(inputs.len(), active, usize::from(layout_available), scroll)
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
        if horizontal && show_add && width > 0 {
            out.add_rect = Some(TabRect { x: width.saturating_sub(TabReservation::ADD_TAB_WIDTH), y: 0, width: TabReservation::ADD_TAB_WIDTH.min(width), height: height.min(1) });
        }
        let visible = &inputs[solved.start..solved.start + solved.count];
        let before_cells = if solved.before > 0 {
            overflow_width(solved.before)
        } else {
            0
        };
        // Keep tiny viewports in bounds.  Normal terminals have room for both
        // markers; when they do not, the tab children still cannot escape the
        // viewport (the upstream box layout has the same one-cell minimum).
        let marker_left = before_cells.min(usize::from(available));
        let mut x_or_y = marker_left as u16;
        for (position, input) in visible.iter().enumerate() {
            let label = format_label(input);
            let item_width = if horizontal {
                solved.widths[position] as u16
            } else {
                width
            };
            let (x, y, w, h) = if horizontal {
                (x_or_y, 0, item_width, height.min(1))
            } else {
                (0, x_or_y, width, 1)
            };
            let rect = TabRect {
                x,
                y,
                width: w,
                height: h,
            };
            let close = if w >= 2 {
                Some(TabRect {
                    x: x + w - 2,
                    y,
                    width: 1,
                    height: h,
                })
            } else {
                None
            };
            let label_width = if close.is_some() { w - 2 } else { w };
            // This is the same structure as SessionTabs: title, then an
            // absolute right=1,width=1 close child, then its trailing gap.
            let rendered = if close.is_some() {
                let mut row = fit(&label, label_width);
                row.push_str(if hovered == Some(&input.session) {
                    "✕ "
                } else {
                    "  "
                });
                row
            } else {
                fit(&label, w)
            };
            out.rows.push(rendered);
            out.rects.push((input.session.clone(), rect));
            if let Some(close) = close {
                out.close_rects.push((input.session.clone(), close));
            }
            x_or_y = x_or_y.saturating_add(if horizontal { item_width } else { 1 });
        }
        out.visible_close_session = out
            .hovered
            .as_ref()
            .filter(|session| out.close_rects.iter().any(|(id, _)| id == *session))
            .cloned();
        out
    }

    /// Return the one close child visible in this frame, if any.
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

fn format_label(input: &TabInput) -> String {
    let mut suffix = String::new();
    if input.flags.active {
        suffix.push('•');
    }
    if input.flags.busy {
        suffix.push('⟳');
    }
    if input.flags.needs_attention {
        suffix.push('!');
    }
    if suffix.is_empty() {
        input.title.clone()
    } else {
        format!("{} {}", input.title, suffix)
    }
}

fn display_width(s: &str) -> usize {
    visit_unpadded_cells(s, 4096, |_, _| {}).min(4096)
}

#[cfg(test)]
mod tests {
    use super::*;
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
            assert_eq!(add, TabRect { x: 77, y: 0, width: 3, height: 1 });
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
}
