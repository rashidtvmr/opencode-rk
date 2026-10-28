#![forbid(unsafe_code)]
//! Bounded vertical session rail geometry and paint projection.
//! Upstream references: packages/tui/src/component/session-tabs.tsx:542-740
//! and packages/tui/src/component/session-tabs-rail.tsx:8-92.

use crate::native_tabs::TabFlags;
use crate::native_view_layout::{fit, visit_unpadded_cells};

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum RailStyle { Idle, Active, Hovered, Busy, Attention, NewControl, Empty }

pub const RAIL_WIDTH: u16 = 42;
pub const TOP_PADDING: u16 = 1;
pub const TAB_HEIGHT: u16 = 2;
pub const TAB_GAP: u16 = 1;
pub const ADD_HEIGHT: u16 = 1;
pub const TAB_STRIDE: u16 = TAB_HEIGHT + TAB_GAP;

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailInput {
    pub session: crate::native_navigation::SessionRef,
    pub title: String,
    pub detail: String,
    pub flags: TabFlags,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct RailRect { pub x: u16, pub y: u16, pub width: u16, pub height: u16 }

impl RailRect {
    pub fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x && y >= self.y && x < self.x.saturating_add(self.width) && y < self.y.saturating_add(self.height)
    }
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RailHit { Session(crate::native_navigation::SessionRef), New }

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum RailRowKind { Padding, Session(crate::native_navigation::SessionRef), Gap, New, Empty }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailRow { pub kind: RailRowKind, pub text: String, pub rect: RailRect, pub style: RailStyle }

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct RailFrame {
    pub width: u16,
    pub height: u16,
    pub scroll_start: usize,
    pub scroll_cells: usize,
    pub rows: Vec<RailRow>,
    pub new_rect: Option<RailRect>,
    visible_close: Option<(crate::native_navigation::SessionRef, RailRect)>,
}

impl RailFrame {
    pub fn render(inputs: &[RailInput], width: u16, height: u16, scroll_start: usize, hovered: Option<&crate::native_navigation::SessionRef>) -> Self {
        Self::render_with_active_new(inputs, width, height, scroll_start, hovered, false)
    }

    pub fn render_with_active_new(inputs: &[RailInput], width: u16, height: u16, scroll_start: usize, hovered: Option<&crate::native_navigation::SessionRef>, active_new: bool) -> Self {
        Self::render_cells(inputs, width, height, scroll_start.saturating_mul(usize::from(TAB_STRIDE)), hovered, active_new)
    }

    /// Scroll and clip in physical cells, including partially visible sessions.
    /// Upstream session-tabs.tsx:658-667 reveals the selected child's exact bottom.
    pub fn render_cells(inputs: &[RailInput], width: u16, height: u16, requested_scroll: usize, hovered: Option<&crate::native_navigation::SessionRef>, active_new: bool) -> Self {
        let mut frame = Self { width, height, scroll_start: 0, scroll_cells: 0, rows: Vec::new(), new_rect: None, visible_close: None };
        if width == 0 || height == 0 { return frame; }
        let top = TOP_PADDING.min(height);
        let usable = usize::from(height.saturating_sub(top));
        let stride = usize::from(TAB_STRIDE);
        let new_top = inputs.len().saturating_mul(stride);
        let maximum = new_top.saturating_add(usize::from(ADD_HEIGHT)).saturating_sub(usable);
        let mut scroll = requested_scroll.min(maximum);
        let reveal = if active_new { Some((new_top, usize::from(ADD_HEIGHT))) }
            else { inputs.iter().position(|input| input.flags.active).map(|index| (index.saturating_mul(stride), usize::from(TAB_HEIGHT))) };
        if usable > 0 {
            if let Some((target, target_height)) = reveal {
                if target < scroll { scroll = target; }
                else if target.saturating_add(target_height) > scroll.saturating_add(usable) {
                    scroll = target.saturating_add(target_height).saturating_sub(usable);
                }
            }
        }
        frame.scroll_cells = scroll.min(maximum);
        frame.scroll_start = frame.scroll_cells / stride;
        // Upstream reserves max(2, decimal tab-count width) indicator cells
        // followed by a one-cell gap; both title and detail share that inset.
        let indicator_width = inputs.len().to_string().len().max(2);
        let pad = width.min(indicator_width.saturating_add(1).min(usize::from(u16::MAX)) as u16);
        for y in 0..height {
            let rect = RailRect { x: 0, y, width, height: 1 };
            if y < top {
                frame.rows.push(RailRow { kind: RailRowKind::Padding, text: fit("", width), rect, style: RailStyle::Empty });
                continue;
            }
            let logical = frame.scroll_cells.saturating_add(usize::from(y - top));
            let index = logical / stride;
            let part = logical % stride;
            if let Some(input) = inputs.get(index) {
                if part < usize::from(TAB_HEIGHT) {
                    let style = if input.flags.needs_attention { RailStyle::Attention } else if input.flags.busy { RailStyle::Busy } else if hovered == Some(&input.session) { RailStyle::Hovered } else if input.flags.active { RailStyle::Active } else { RailStyle::Idle };
                    let close = part == 0 && hovered == Some(&input.session) && width >= 4;
                    // The close child occupies the penultimate cell. An idle
                    // title reserves the right padding; details reserve two.
                    let tail = if close || part != 0 { 2 } else { 1 };
                    let row_pad = pad.min(width.saturating_sub(tail));
                    let indicator = if part == 0 && input.flags.busy && !input.flags.needs_attention { "⠋" } else { "" };
                    let prefix = fit(&format!("{indicator:>indicator_width$} "), row_pad);
                    let available = width.saturating_sub(row_pad).saturating_sub(tail);
                    let text = fit(if part == 0 { &input.title } else { &input.detail }, available);
                    let text = if close {
                        frame.visible_close = Some((input.session.clone(), RailRect { x: width - 2, y, width: 1, height: 1 }));
                        format!("{prefix}{text}✕ ")
                    } else { fit(&format!("{prefix}{text}"), width) };
                    frame.rows.push(RailRow { kind: RailRowKind::Session(input.session.clone()), text, rect, style });
                } else {
                    frame.rows.push(RailRow { kind: RailRowKind::Gap, text: fit("", width), rect, style: RailStyle::Empty });
                }
            } else if logical == new_top {
                frame.new_rect = Some(rect);
                frame.rows.push(RailRow { kind: RailRowKind::New, text: fit(" + New session", width), rect, style: RailStyle::NewControl });
            } else {
                frame.rows.push(RailRow { kind: RailRowKind::Empty, text: fit("", width), rect, style: RailStyle::Empty });
            }
        }
        frame
    }

    pub fn hit(&self, x: u16, y: u16) -> Option<RailHit> {
        self.rows.iter().find(|row| row.rect.contains(x, y)).and_then(|row| match &row.kind { RailRowKind::Session(id) => Some(RailHit::Session(id.clone())), RailRowKind::New => Some(RailHit::New), _ => None })
    }

    /// Resolve the exact painted session row for a captured mouse press.
    pub fn session_press_target(&self, x: u16, y: u16) -> Option<(crate::native_navigation::SessionRef, RailRect, bool)> {
        let RailHit::Session(session) = self.hit(x, y)? else { return None };
        self.rows.iter().find(|row| {
            row.rect.contains(x, y) && matches!(&row.kind, RailRowKind::Session(id) if id == &session)
        }).map(|row| (session.clone(), row.rect, self.close_hit(x, y) == Some(&session)))
    }

    pub fn close_hit(&self, x: u16, y: u16) -> Option<&crate::native_navigation::SessionRef> {
        self.visible_close.as_ref().filter(|(_, rect)| rect.contains(x, y)).map(|(session, _)| session)
    }

    pub fn display_width(text: &str) -> usize { visit_unpadded_cells(text, usize::MAX, |_, _| {}) }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn rail_text_and_busy_indicator_match_pinned_reference_frame() {
        // Pinned OpenTUI component, 42x40, three sessions, active New,
        // animations=false: rail-status-home.txt (2026-10-28 capture).
        let mut inputs = sessions(3);
        for (index, input) in inputs.iter_mut().enumerate() {
            input.title = format!("Session {}", index + 1);
            input.flags.active = false;
        }
        inputs[1].flags.busy = true;
        let frame = RailFrame::render_cells(&inputs, 42, 40, 0, None, true);
        assert_eq!(frame.rows[1].text.trim_end(), "   Session 1");
        assert_eq!(frame.rows[2].text.trim_end(), "   project");
        assert_eq!(frame.rows[4].text.trim_end(), " ⠋ Session 2");
        assert_eq!(frame.rows[10].text.trim_end(), " + New session");
    }

    #[test]
    fn close_child_is_visible_and_actionable_only_on_hovered_title_row() {
        let inputs = sessions(2);
        let resting = RailFrame::render(&inputs, 42, 10, 0, None);
        assert_eq!(resting.close_hit(40, 1), None);
        assert!(!resting.rows.iter().any(|row| row.text.contains('✕')));
        let hovered = RailFrame::render(&inputs, 42, 10, 0, Some(&inputs[0].session));
        assert_eq!(hovered.close_hit(40, 1), Some(&inputs[0].session));
        assert_eq!(hovered.close_hit(40, 2), None, "detail row never closes");
        assert_eq!(hovered.close_hit(41, 1), None, "right padding never closes");
        let title = hovered.rows.iter().find(|row| row.kind == RailRowKind::Session(inputs[0].session.clone())).unwrap();
        assert_eq!(crate::native_view_layout::slice_cells(title.text.lines().next().unwrap(), 40, 1), "✕");
        let moved = RailFrame::render(&inputs, 42, 10, 0, Some(&inputs[1].session));
        assert_eq!(moved.close_hit(40, 1), None, "old hover loses close authority");
        for width in 0..4 {
            let tiny = RailFrame::render(&inputs, width, 10, 0, Some(&inputs[0].session));
            assert!(tiny.visible_close.is_none());
        }
    }

    #[test]
    fn hovered_close_fits_narrow_rails_without_overrunning() {
        let inputs = sessions(1);
        for width in 0..=6 {
            let frame = RailFrame::render(&inputs, width, 6, 0, Some(&inputs[0].session));
            for row in &frame.rows {
                assert!(RailFrame::display_width(&row.text) <= usize::from(width));
            }
            if width >= 4 {
                assert_eq!(frame.close_hit(width - 2, 1), Some(&inputs[0].session));
            }
        }
    }

    #[test]
    fn mouse_press_captures_title_detail_and_clipped_detail_rows() {
        let inputs = sessions(2);
        let frame = RailFrame::render(&inputs, 42, 10, 0, None);
        for y in [1, 2] {
            let (session, rect, close) = frame.session_press_target(0, y).expect("painted session row is actionable");
            assert_eq!(session, inputs[0].session);
            assert!(rect.contains(0, y), "captured row must contain the press coordinate");
            assert!(!close);
        }

        let mut many = sessions(32);
        for input in &mut many { input.flags.active = false; }
        let clipped = RailFrame::render_cells(&many, 42, 40, 0, None, true);
        assert_eq!(clipped.scroll_cells, 58);
        let (session, rect, close) = clipped.session_press_target(0, 1).expect("clipped detail remains selectable");
        assert_eq!(session, many[19].session);
        assert!(rect.contains(0, 1), "clipped-detail press must capture its painted row");
        assert!(!close, "clipped detail never exposes close action");
    }

    fn sessions(count: usize) -> Vec<RailInput> {
        (0..count).map(|index| RailInput {
            session: crate::native_navigation::SessionRef(format!("tab-{index}")),
            title: "界面😀 long title".into(), detail: "project".into(),
            flags: TabFlags { active: index + 1 == count, busy: false, needs_attention: false },
        }).collect()
    }

    #[test]
    fn every_painted_cell_is_bounded_and_unambiguous() {
        for width in [0, 1, 2, 42] {
            for height in [0, 1, 2, 3, 6, 20] {
                for count in [0, 1, 32] {
                    let frame = RailFrame::render(&sessions(count), width, height, usize::MAX, None);
                    if width == 0 || height == 0 { assert!(frame.rows.is_empty()); continue; }
                    let mut covered = vec![false; usize::from(height)];
                    for row in &frame.rows {
                        assert!(row.rect.y + row.rect.height <= height);
                        for line in row.text.split('\n') { assert!(RailFrame::display_width(line) <= usize::from(width)); }
                        for y in row.rect.y..row.rect.y + row.rect.height {
                            assert!(!covered[usize::from(y)], "overlapping row {y}");
                            covered[usize::from(y)] = true;
                        }
                    }
                    assert!(covered.into_iter().all(|cell| cell));
                    if let Some(rect) = frame.new_rect { assert_eq!(frame.hit(rect.x, rect.y), Some(RailHit::New)); }
                }
            }
        }
    }

    #[test]
    fn active_tail_reveals_new_when_complete_logical_tail_fits() {
        let inputs = sessions(32);
        let frame = RailFrame::render(&inputs, 42, 11, 0, None);
        assert_eq!(frame.scroll_cells, 85, "active session follows upstream exact-cell reveal");
        assert_eq!(frame.scroll_start, 28, "item offset is display metadata only");
        assert!(frame.rows.iter().any(|row| row.kind == RailRowKind::Session(inputs[31].session.clone()) && row.rect.y == 9));
        assert_eq!(frame.hit(0, 9), Some(RailHit::Session(inputs[31].session.clone())));
        assert_eq!(frame.hit(0, 10), Some(RailHit::Session(inputs[31].session.clone())));
        assert!(frame.new_rect.is_none(), "inactive New child remains below the viewport");
    }

    #[test]
    fn active_home_reveals_new_after_retained_sessions() {
        let mut inputs = sessions(32);
        for input in &mut inputs { input.flags.active = false; }
        let inactive = RailFrame::render_with_active_new(&inputs, 42, 6, 0, None, false);
        assert_eq!(inactive.scroll_start, 0);
        assert!(inactive.new_rect.is_none());
        let active = RailFrame::render_with_active_new(&inputs, 42, 6, 0, None, true);
        let rect = active.new_rect.expect("active Home reveals final New child");
        assert_eq!(active.hit(rect.x, rect.y), Some(RailHit::New));
        assert!(active.scroll_start > 0);
    }

    #[test]
    fn active_home_keeps_cell_scroll_and_clipped_detail_at_exact_bottom() {
        let mut inputs = sessions(32);
        for input in &mut inputs { input.flags.active = false; }
        let frame = RailFrame::render_cells(&inputs, 42, 40, 0, Some(&inputs[19].session), true);
        assert_eq!(frame.scroll_cells, 58);
        assert_eq!(frame.new_rect, Some(RailRect { x: 0, y: 39, width: 42, height: 1 }));
        assert_eq!(frame.hit(0, 1), Some(RailHit::Session(inputs[19].session.clone())));
        assert_eq!(frame.rows[1].text.trim(), "project");
        assert_eq!(frame.close_hit(40, 1), None, "clipped title has no close authority");
        assert_eq!(frame.hit(0, 39), Some(RailHit::New));
        let repaint = RailFrame::render_cells(&inputs, 42, 40, frame.scroll_cells, None, true);
        assert_eq!(repaint.scroll_cells, 58);
        assert_eq!(repaint.new_rect, frame.new_rect);
    }

    #[test]
    fn active_home_reveal_is_bounded_for_short_viewports() {
        let mut inputs = sessions(8);
        for input in &mut inputs { input.flags.active = false; }
        for height in 0..=12 {
            let frame = RailFrame::render_with_active_new(&inputs, 42, height, 0, None, true);
            assert!(frame.rows.iter().all(|row| row.rect.y.saturating_add(row.rect.height) <= height));
            if height > TOP_PADDING {
                let rect = frame.new_rect.expect("New fits after fixed top padding");
                assert_eq!(frame.hit(rect.x, rect.y), Some(RailHit::New));
            } else {
                assert!(frame.new_rect.is_none());
            }
        }
    }

    #[test]
    fn active_reveal_does_not_pin_new_when_logical_tail_is_offscreen() {
        let inputs = sessions(32);
        let frame = RailFrame::render(&inputs, 42, 6, 0, None);
        assert_eq!(frame.scroll_start, 30, "active session follows upstream reveal rule");
        assert!(frame.rows.iter().any(|row| row.kind == RailRowKind::Session(inputs[31].session.clone()) && row.rect.y == 4));
        assert_eq!(frame.hit(0, 4), Some(RailHit::Session(inputs[31].session.clone())));
        assert!(frame.new_rect.is_none());
        assert_eq!(frame.hit(0, 6), None, "outside viewport must not hit");
    }

    #[test]
    fn new_uses_complete_logical_list_not_visible_slice() {
        let mut inputs = sessions(8);
        for input in &mut inputs { input.flags.active = false; }
        let top = RailFrame::render(&inputs, 42, 6, 0, None);
        assert!(top.new_rect.is_none(), "top window must not append New after visible slice");
        let bottom = RailFrame::render(&inputs, 42, 6, 7, None);
        assert_eq!(bottom.scroll_cells, 20);
        assert_eq!(bottom.scroll_start, 6);
        assert!(bottom.rows.iter().any(|row| row.kind == RailRowKind::Session(inputs[7].session.clone()) && row.rect.y == 2));
        assert_eq!(bottom.hit(0, 2), Some(RailHit::Session(inputs[7].session.clone())));
        assert_eq!(bottom.hit(0, 3), Some(RailHit::Session(inputs[7].session.clone())));
        assert!(bottom.rows.iter().any(|row| row.kind == RailRowKind::Gap && row.rect.y == 4));
        assert_eq!(bottom.new_rect, Some(RailRect { x: 0, y: 5, width: 42, height: 1 }));
        assert_eq!(bottom.hit(0, 5), Some(RailHit::New));
    }

    #[test]
    fn requested_max_scroll_reveals_new_without_hiding_active_session() {
        let inputs = sessions(8);
        let frame = RailFrame::render(&inputs, 42, 6, 7, None);
        assert_eq!(frame.scroll_cells, 20);
        assert_eq!(frame.scroll_start, 6);
        assert!(frame.rows.iter().any(|row| row.kind == RailRowKind::Session(inputs[7].session.clone()) && row.rect.y == 2));
        assert_eq!(frame.hit(0, 2), Some(RailHit::Session(inputs[7].session.clone())));
        assert_eq!(frame.hit(0, 3), Some(RailHit::Session(inputs[7].session.clone())));
        assert!(frame.rows.iter().any(|row| row.kind == RailRowKind::Gap && row.rect.y == 4));
        assert_eq!(frame.new_rect, Some(RailRect { x: 0, y: 5, width: 42, height: 1 }));
        assert_eq!(frame.hit(0, 5), Some(RailHit::New));
    }

    #[test]
    fn zero_items_new_is_after_padding_and_short_heights_are_bounded() {
        let empty = RailFrame::render(&[], 42, 6, 0, None);
        assert_eq!(empty.new_rect, Some(RailRect { x: 0, y: 1, width: 42, height: 1 }));
        assert_eq!(empty.hit(41, 1), Some(RailHit::New));
        for height in 0..=4 {
            let frame = RailFrame::render(&[], 42, height, usize::MAX, None);
            assert!(frame.rows.iter().all(|row| row.rect.y.saturating_add(row.rect.height) <= height));
            match frame.new_rect {
                Some(rect) => {
                    assert_eq!(frame.hit(rect.x, rect.y), Some(RailHit::New));
                    assert!(rect.y < height);
                    assert!(frame.hit(0, height).is_none(), "outside viewport must not hit");
                }
                None => {
                    assert!(frame.hit(0, height.saturating_sub(1)).is_none());
                }
            }
        }
    }

    #[test]
    fn new_exact_geometry_for_short_list_and_gap() {
        let inputs = sessions(2);
        let frame = RailFrame::render(&inputs, 42, 10, 0, None);
        assert_eq!(frame.new_rect, Some(RailRect { x: 0, y: 7, width: 42, height: 1 }));
        assert_eq!(frame.hit(41, 7), Some(RailHit::New));
        assert_eq!(frame.hit(41, 8), None);
    }

    #[test]
    fn short_heights_hit_only_painted_controls() {
        let inputs = sessions(8);
        for height in 0..=4 {
            let frame = RailFrame::render(&inputs, 42, height, 0, None);
            assert!(frame.rows.iter().all(|row| row.rect.y.saturating_add(row.rect.height) <= height));
            for y in 0..height {
                let painted = frame.rows.iter().find(|row| row.rect.contains(0, y)).and_then(|row| match &row.kind {
                    RailRowKind::Session(id) => Some(RailHit::Session(id.clone())),
                    RailRowKind::New => Some(RailHit::New),
                    _ => None,
                });
                assert_eq!(frame.hit(0, y), painted, "height={height} y={y}");
                assert_eq!(frame.hit(42, y), None, "outside rail width must not hit");
            }
            assert_eq!(frame.hit(0, height), None, "outside viewport must not hit");
        }
    }

}
