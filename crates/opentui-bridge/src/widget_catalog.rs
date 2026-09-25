#![forbid(unsafe_code)]
//! Typed widget callers for the existing bounded widget paint primitives.

use crate::layout::Rect;
use crate::widget_paint::{bar_calls, card_calls, menu_calls, spark_calls, Bar, Card, Menu, Spark};
use crate::world::PaintCall;

const MENU_ITEM_LIMIT: usize = 32;

/// A bounded menu value.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MenuItems {
    pub items: Vec<String>,
    pub selected: usize,
}

/// A bounded card definition.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CardDef {
    pub title: String,
    pub body_rows: u32,
}

/// A widget and its typed state.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum WidgetKind {
    Bar(u8),
    Spark(Vec<u8>),
    Menu(MenuItems),
    Card(CardDef),
}

/// Convert a typed widget to the existing paint-call representation.
#[must_use]
pub fn paint_widget(kind: WidgetKind, rect: Rect) -> Vec<PaintCall> {
    if rect.w == 0 || rect.h == 0 {
        return Vec::new();
    }

    match kind {
        WidgetKind::Bar(percent) => bar_calls(Bar { percent }, rect),
        WidgetKind::Spark(points) => spark_calls(Spark { points: &points }, rect),
        WidgetKind::Menu(menu) => {
            let items: Vec<&str> = menu
                .items
                .iter()
                .take(MENU_ITEM_LIMIT)
                .map(String::as_str)
                .collect();
            menu_calls(
                Menu {
                    items: &items,
                    selected: menu.selected,
                },
                rect,
            )
        }
        WidgetKind::Card(card) => card_calls(
            Card {
                title: &card.title,
                body_rows: card.body_rows,
            },
            rect,
        ),
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::world::Region;

    fn area(w: u32, h: u32) -> Rect {
        Rect { x: 0, y: 0, w, h }
    }

    #[test]
    fn bar_fill_delegates() {
        let calls = paint_widget(WidgetKind::Bar(50), area(10, 1));
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].rect.w, 5);
        assert_eq!(calls[0].region, Region::Status);
    }

    #[test]
    fn spark_peak_delegates() {
        let calls = paint_widget(WidgetKind::Spark(vec![0, 10]), area(2, 4));
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].rect.h, 4);
    }

    #[test]
    fn menu_rows_delegate_and_cap() {
        let items = (0..MENU_ITEM_LIMIT + 3).map(|i| i.to_string()).collect();
        let calls = paint_widget(
            WidgetKind::Menu(MenuItems {
                items,
                selected: 99,
            }),
            area(4, MENU_ITEM_LIMIT as u32 + 3),
        );
        assert_eq!(calls.len(), MENU_ITEM_LIMIT);
    }

    #[test]
    fn card_clip_delegates() {
        let calls = paint_widget(
            WidgetKind::Card(CardDef {
                title: "title".to_string(),
                body_rows: 99,
            }),
            area(10, 3),
        );
        assert_eq!(calls.len(), 1);
        assert_eq!(calls[0].rect.h, 3);
        assert_eq!(calls[0].region, Region::Composer);
    }

    #[test]
    fn empty_rect_paints_nothing() {
        assert!(paint_widget(WidgetKind::Bar(100), area(0, 4)).is_empty());
        assert!(paint_widget(WidgetKind::Spark(vec![1]), area(4, 0)).is_empty());
        assert!(paint_widget(
            WidgetKind::Menu(MenuItems {
                items: vec!["item".to_string()],
                selected: 0,
            }),
            Rect {
                x: 1,
                y: 1,
                w: 0,
                h: 0
            },
        )
        .is_empty());
    }
}
