#![forbid(unsafe_code)]
//! Widget state -> [`crate::world::PaintCall`] lists. Pure, bounded, no IO.
//!
//! TS truth: `feature-plugins/sidebar/*.tsx` rows, `system/which-key.tsx`
//! menu, `component/todo-item.tsx` card-ish rows. Divergence: text-free
//! paint calls (region+rect only; text drawn by caller via `draw_text`).

use crate::layout::Rect;
use crate::world::{PaintCall, Region};

/// Progress bar 0-100.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Bar {
    pub percent: u8,
}

/// Sparkline points (bounded slice).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Spark<'a> {
    pub points: &'a [u8],
}

/// Menu items + selected index.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Menu<'a> {
    pub items: &'a [&'a str],
    pub selected: usize,
}

/// Bordered card: title row + body rows.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Card<'a> {
    pub title: &'a str,
    pub body_rows: u32,
}

fn empty(r: Rect) -> bool {
    r.w == 0 || r.h == 0
}

fn clip(r: Rect, within: Rect) -> Option<Rect> {
    let x = r.x.max(within.x);
    let y = r.y.max(within.y);
    let ex = (r.x.saturating_add(r.w)).min(within.x.saturating_add(within.w));
    let ey = (r.y.saturating_add(r.h)).min(within.y.saturating_add(within.h));
    if ex <= x || ey <= y {
        return None;
    }
    Some(Rect {
        x,
        y,
        w: ex - x,
        h: ey - y,
    })
}

/// Bar fill rect clipped to `within`.
#[must_use]
pub fn bar_calls(bar: Bar, within: Rect) -> Vec<PaintCall> {
    if empty(within) {
        return Vec::new();
    }
    let pct = bar.percent.min(100);
    let w = within.w * u32::from(pct) / 100;
    if w == 0 {
        return Vec::new();
    }
    let r = Rect {
        x: within.x,
        y: within.y,
        w,
        h: 1.min(within.h),
    };
    match clip(r, within) {
        Some(c) => vec![PaintCall {
            region: Region::Status,
            rect: c,
        }],
        None => Vec::new(),
    }
}

/// One call per point column, height scaled to `within.h`.
#[must_use]
pub fn spark_calls(spark: Spark<'_>, within: Rect) -> Vec<PaintCall> {
    if empty(within) || spark.points.is_empty() {
        return Vec::new();
    }
    let max = spark.points.iter().copied().max().unwrap_or(0).max(1);
    let mut out = Vec::new();
    for (i, &p) in spark.points.iter().enumerate() {
        if u32::try_from(i).unwrap_or(u32::MAX) >= within.w {
            break;
        }
        let h = (u32::from(p) * within.h) / u32::from(max);
        if h == 0 {
            continue;
        }
        let r = Rect {
            x: within.x + u32::try_from(i).unwrap_or(u32::MAX),
            y: within.y + within.h - h,
            w: 1,
            h,
        };
        if let Some(c) = clip(r, within) {
            out.push(PaintCall {
                region: Region::Status,
                rect: c,
            });
        }
    }
    out
}

/// One call per visible item row; selected row kept (caller highlights).
#[must_use]
pub fn menu_calls(menu: Menu<'_>, within: Rect) -> Vec<PaintCall> {
    if empty(within) || menu.items.is_empty() {
        return Vec::new();
    }
    let mut out = Vec::new();
    for (i, _) in menu.items.iter().enumerate() {
        let io = u32::try_from(i).unwrap_or(u32::MAX);
        if io >= within.h {
            break;
        }
        let r = Rect {
            x: within.x,
            y: within.y + io,
            w: within.w,
            h: 1,
        };
        if let Some(c) = clip(r, within) {
            let _ = (c, menu.selected);
            out.push(PaintCall {
                region: Region::Sidebar,
                rect: c,
            });
        }
    }
    out
}

/// Border title row + body rows clipped.
#[must_use]
pub fn card_calls(card: Card<'_>, within: Rect) -> Vec<PaintCall> {
    if empty(within) || card.title.is_empty() {
        return Vec::new();
    }
    let rows = (u32::from(card.body_rows) + 1).min(within.h);
    if rows == 0 {
        return Vec::new();
    }
    let r = Rect {
        x: within.x,
        y: within.y,
        w: within.w,
        h: rows,
    };
    match clip(r, within) {
        Some(c) => vec![PaintCall {
            region: Region::Composer,
            rect: c,
        }],
        None => Vec::new(),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn area(w: u32, h: u32) -> Rect {
        Rect { x: 0, y: 0, w, h }
    }

    #[test]
    fn bar_levels() {
        assert!(bar_calls(Bar { percent: 0 }, area(10, 1)).is_empty());
        assert_eq!(bar_calls(Bar { percent: 50 }, area(10, 1))[0].rect.w, 5);
        assert_eq!(bar_calls(Bar { percent: 100 }, area(10, 1))[0].rect.w, 10);
    }

    #[test]
    fn spark_flat_peak() {
        let flat = spark_calls(Spark { points: &[5, 5, 5] }, area(3, 4));
        assert_eq!(flat.len(), 3);
        let peak = spark_calls(Spark { points: &[0, 10] }, area(2, 4));
        assert_eq!(peak.len(), 1);
        assert_eq!(peak[0].rect.h, 4);
    }

    #[test]
    fn menu_oob_selected_kept() {
        let m = menu_calls(
            Menu {
                items: &["a", "b"],
                selected: 99,
            },
            area(10, 5),
        );
        assert_eq!(m.len(), 2);
    }

    #[test]
    fn card_overflow_clip() {
        let c = card_calls(
            Card {
                title: "t",
                body_rows: 99,
            },
            area(10, 3),
        );
        assert_eq!(c[0].rect.h, 3);
        assert!(card_calls(
            Card {
                title: "",
                body_rows: 1
            },
            area(10, 3)
        )
        .is_empty());
    }

    #[test]
    fn zero_rect_empty() {
        assert!(bar_calls(Bar { percent: 50 }, area(0, 0)).is_empty());
        assert!(menu_calls(
            Menu {
                items: &["a"],
                selected: 0
            },
            area(0, 0)
        )
        .is_empty());
    }
}
