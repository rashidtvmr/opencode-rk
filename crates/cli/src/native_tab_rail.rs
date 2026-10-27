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
    pub rows: Vec<RailRow>,
    pub new_rect: Option<RailRect>,
    visible_close: Option<(crate::native_navigation::SessionRef, RailRect)>,
}

impl RailFrame {
    pub fn render(inputs: &[RailInput], width: u16, height: u16, scroll_start: usize, hovered: Option<&crate::native_navigation::SessionRef>) -> Self {
        let mut frame = Self { width, height, scroll_start: 0, rows: Vec::new(), new_rect: None, visible_close: None };
        if width == 0 || height == 0 { return frame; }
        // Reserve the bottom control independently of session count/overflow.
        let add_y = height - ADD_HEIGHT;
        let top = TOP_PADDING.min(add_y);
        let usable = usize::from(add_y.saturating_sub(top));
        let capacity = (usable + usize::from(TAB_GAP)) / usize::from(TAB_HEIGHT + TAB_GAP);
        let max_start = inputs.len().saturating_sub(capacity);
        let mut start = scroll_start.min(max_start);
        if capacity > 0 {
            if let Some(active) = inputs.iter().position(|input| input.flags.active) {
                if active < start { start = active; }
                if active >= start + capacity { start = active + 1 - capacity; }
            }
        }
        frame.scroll_start = start;
        let mut y = 0;
        while y < top {
            frame.rows.push(RailRow { kind: RailRowKind::Padding, text: fit("", width), rect: RailRect { x: 0, y, width, height: 1 }, style: RailStyle::Empty });
            y += 1;
        }
        let count = capacity.min(inputs.len().saturating_sub(start));
        for (index, input) in inputs.iter().skip(start).take(count).enumerate() {
            let style = if input.flags.needs_attention { RailStyle::Attention } else if input.flags.busy { RailStyle::Busy } else if hovered == Some(&input.session) { RailStyle::Hovered } else if input.flags.active { RailStyle::Active } else { RailStyle::Idle };
            let pad = width.min(2);
            let prefix = " ".repeat(usize::from(pad));
            let close = hovered == Some(&input.session) && width >= 4;
            let title = fit(&input.title, width - pad - if close { 2 } else { 0 });
            let title = if close {
                frame.visible_close = Some((input.session.clone(), RailRect { x: width - 2, y, width: 1, height: 1 }));
                format!("{title}✕ ")
            } else { title };
            let detail = fit(&input.detail, width - pad);
            let text = format!("{prefix}{title}\n{prefix}{detail}");
            frame.rows.push(RailRow { kind: RailRowKind::Session(input.session.clone()), text, rect: RailRect { x: 0, y, width, height: TAB_HEIGHT }, style });
            y = y.saturating_add(TAB_HEIGHT);
            if index + 1 < count { frame.rows.push(RailRow { kind: RailRowKind::Gap, text: fit("", width), rect: RailRect { x: 0, y, width, height: TAB_GAP }, style: RailStyle::Empty }); y += TAB_GAP; }
        }
        while y < add_y {
            frame.rows.push(RailRow { kind: RailRowKind::Empty, text: fit("", width), rect: RailRect { x: 0, y, width, height: 1 }, style: RailStyle::Empty });
            y += 1;
        }
        let rect = RailRect { x: 0, y: add_y, width, height: ADD_HEIGHT };
        frame.new_rect = Some(rect);
        frame.rows.push(RailRow { kind: RailRowKind::New, text: fit("+ New session", width), rect, style: RailStyle::NewControl });
        frame
    }

    pub fn hit(&self, x: u16, y: u16) -> Option<RailHit> {
        self.rows.iter().find(|row| row.rect.contains(x, y)).and_then(|row| match &row.kind { RailRowKind::Session(id) => Some(RailHit::Session(id.clone())), RailRowKind::New => Some(RailHit::New), _ => None })
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
                    assert_eq!(frame.hit(width - 1, height - 1), Some(RailHit::New));
                }
            }
        }
    }

    #[test]
    fn active_tail_is_visible_without_hiding_new_session() {
        let inputs = sessions(32);
        let frame = RailFrame::render(&inputs, 42, 6, 0, None);
        assert!(frame.rows.iter().any(|row| row.kind == RailRowKind::Session(inputs[31].session.clone())));
        assert_eq!(frame.hit(0, 5), Some(RailHit::New));
    }
}
