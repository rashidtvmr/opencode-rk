#![forbid(unsafe_code)]
//! Bounded native projection of OpenCode V2's session-tab context menu.
//!
//! This module produces action intents only. The owning TUI loop must route
//! them to its real session/navigation/clipboard facilities; constructing a
//! menu never performs an operation.
//!
//! Upstream pin: `450e7b92412c780a7b5a75226bb8d49aaca24e22`,
//! `packages/tui/src/component/session-tabs.tsx:370-501,892-913,1652-1675`.

use crate::native_navigation::SessionRef;

pub const CONTEXT_MENU_MIN_WIDTH: u16 = 20;
pub const CONTEXT_MENU_MAX_WIDTH: usize = 50;
pub const MAX_RECENTLY_CLOSED: usize = 10;

#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Viewport {
    pub width: u16,
    pub height: u16,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct MenuRect {
    pub x: u16,
    pub y: u16,
    pub width: u16,
    pub height: u16,
}

impl MenuRect {
    #[must_use]
    pub fn contains(self, x: u16, y: u16) -> bool {
        x >= self.x
            && y >= self.y
            && x < self.x.saturating_add(self.width)
            && y < self.y.saturating_add(self.height)
    }
}

/// Recently closed tab descriptor supplied by the caller. The ID remains the
/// action identity; its title is presentation only.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ClosedTab {
    pub session: SessionRef,
    pub title: String,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuTarget {
    Session {
        session: SessionRef,
        title: String,
        can_add: bool,
    },
    /// `None` means the owner has no reopen facility. `Some(empty)` means the
    /// facility exists but has no history, so the upstream empty-state rows show.
    New { recently_closed: Option<Vec<ClosedTab>> },
}

/// Stable-ID intents. Caller performs the real backend/UI operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuAction {
    NewSession,
    Rename {
        session: SessionRef,
        current_title: String,
    },
    CopySessionId {
        session: SessionRef,
    },
    Close {
        session: SessionRef,
    },
    Reopen {
        session: SessionRef,
    },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuItem {
    pub label: String,
    /// `None` marks a disabled section heading or empty-state row.
    pub action: Option<MenuAction>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct VisibleMenuItem {
    pub index: usize,
    pub rect: MenuRect,
    /// Cell-bounded display text, excluding the one-cell horizontal padding.
    pub label: String,
    pub enabled: bool,
    pub hovered: bool,
    pub background: crate::native_theme_runtime::Slot,
    pub foreground: crate::native_theme_runtime::Slot,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MenuFrame {
    pub rect: Option<MenuRect>,
    pub items: Vec<VisibleMenuItem>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MouseButton {
    Left,
    Right,
    Other,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum MenuEvent {
    Escape,
    PointerMove { x: u16, y: u16 },
    PointerDown { button: MouseButton, x: u16, y: u16 },
    PointerUp { button: MouseButton, x: u16, y: u16 },
    Scroll { delta: i16, x: u16, y: u16 },
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub enum MenuOutcome {
    Ignored,
    Captured,
    Closed,
    Invoke(MenuAction),
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct TabContextMenu {
    anchor_x: u16,
    anchor_y: u16,
    items: Vec<MenuItem>,
    open: bool,
    hovered: Option<usize>,
    scroll: usize,
}

impl TabContextMenu {
    #[must_use]
    pub fn new(anchor_x: u16, anchor_y: u16, target: MenuTarget) -> Self {
        let items = match target {
            MenuTarget::Session {
                session,
                title,
                can_add,
            } => [
                can_add.then(|| MenuItem {
                    label: "New session".into(),
                    action: Some(MenuAction::NewSession),
                }),
                Some(MenuItem {
                    label: "Rename".into(),
                    action: Some(MenuAction::Rename {
                        session: session.clone(),
                        current_title: title,
                    }),
                }),
                Some(MenuItem {
                    label: "Copy session ID".into(),
                    action: Some(MenuAction::CopySessionId {
                        session: session.clone(),
                    }),
                }),
                Some(MenuItem {
                    label: "Close".into(),
                    action: Some(MenuAction::Close { session }),
                }),
            ].into_iter().flatten().collect(),
            MenuTarget::New { recently_closed } => {
                let Some(recently_closed) = recently_closed else {
                    return Self {
                        anchor_x, anchor_y, items: Vec::new(), open: true,
                        hovered: None, scroll: 0,
                    };
                };
                let mut items = vec![MenuItem {
                    label: "Recently closed tabs".into(),
                    action: None,
                }];
                let closed: Vec<_> = recently_closed
                    .into_iter()
                    .take(MAX_RECENTLY_CLOSED)
                    .collect();
                if closed.is_empty() {
                    items.push(MenuItem {
                        label: "No recently closed tabs".into(),
                        action: None,
                    });
                } else {
                    items.extend(closed.into_iter().map(|tab| MenuItem {
                        label: if tab.title.is_empty() {
                            "Untitled session".into()
                        } else {
                            tab.title
                        },
                        action: Some(MenuAction::Reopen {
                            session: tab.session,
                        }),
                    }));
                }
                items
            }
        };
        Self {
            anchor_x,
            anchor_y,
            items,
            open: true,
            hovered: None,
            scroll: 0,
        }
    }

    #[must_use]
    pub fn is_open(&self) -> bool {
        self.open
    }

    #[must_use]
    pub fn items(&self) -> &[MenuItem] {
        &self.items
    }

    /// Mirror upstream `width`, `height`, `left`, and `top` calculations.
    /// Zero-sized viewports produce no popup; all coordinate arithmetic clamps.
    #[must_use]
    pub fn frame(&self, viewport: Viewport) -> MenuFrame {
        if !self.open || viewport.width == 0 || viewport.height == 0 || self.items.is_empty() {
            return MenuFrame {
                rect: None,
                items: Vec::new(),
            };
        }
        let content_width = self
            .items
            .iter()
            .map(|item| {
                crate::native_view_layout::visit_unpadded_cells(
                    &item.label,
                    CONTEXT_MENU_MAX_WIDTH.saturating_sub(2),
                    |_, _| {},
                )
            })
            .map(|label_width| label_width.saturating_add(2))
            .max()
            .unwrap_or(0);
        let desired = usize::from(CONTEXT_MENU_MIN_WIDTH).max(content_width);
        let width = viewport
            .width
            .min(u16::try_from(desired).unwrap_or(u16::MAX));
        let height = viewport
            .height
            .min(u16::try_from(self.items.len()).unwrap_or(u16::MAX));
        let rect = MenuRect {
            x: self.anchor_x.min(viewport.width.saturating_sub(width)),
            y: self
                .anchor_y
                .saturating_add(1)
                .min(viewport.height.saturating_sub(height)),
            width,
            height,
        };
        let start = self
            .scroll
            .min(self.items.len().saturating_sub(usize::from(height)));
        let items = self
            .items
            .iter()
            .enumerate()
            .skip(start)
            .take(usize::from(height))
            .map(|(index, item)| {
                let enabled = item.action.is_some();
                let hovered = enabled && self.hovered == Some(index);
                let padding = u16::from(rect.width >= 3);
                let label_width = rect.width.saturating_sub(padding.saturating_mul(2));
                VisibleMenuItem {
                    index,
                    rect: MenuRect {
                        x: rect.x,
                        y: rect.y.saturating_add((index - start) as u16),
                        width: rect.width,
                        height: 1,
                    },
                    label: crate::native_view_layout::fit(&item.label, label_width),
                    enabled,
                    hovered,
                    background: if hovered {
                        crate::native_theme_runtime::Slot::BackgroundElement
                    } else {
                        crate::native_theme_runtime::Slot::BackgroundPanel
                    },
                    foreground: if enabled {
                        crate::native_theme_runtime::Slot::Text
                    } else {
                        crate::native_theme_runtime::Slot::TextMuted
                    },
                }
            })
            .collect();
        MenuFrame {
            rect: Some(rect),
            items,
        }
    }

    /// Match upstream item-level `onMouseUp`: any non-right release over an
    /// enabled visible row invokes that row, irrespective of press origin.
    /// Disabled rows are inert; outside presses close the menu backdrop.
    pub fn handle(&mut self, event: MenuEvent, viewport: Viewport) -> MenuOutcome {
        if !self.open {
            return MenuOutcome::Ignored;
        }
        match event {
            MenuEvent::Escape => self.close(),
            MenuEvent::PointerMove { x, y } => {
                let frame = self.frame(viewport);
                self.hovered = frame
                    .items
                    .iter()
                    .find(|item| item.rect.contains(x, y) && item.enabled)
                    .map(|item| item.index);
                MenuOutcome::Captured
            }
            MenuEvent::PointerDown { button, x, y } => {
                let frame = self.frame(viewport);
                let inside = frame.rect.is_some_and(|rect| rect.contains(x, y));
                if !inside || button == MouseButton::Right {
                    return self.close();
                }
                MenuOutcome::Captured
            }
            MenuEvent::PointerUp { button, x, y } => {
                if button == MouseButton::Right {
                    return MenuOutcome::Captured;
                }
                let frame = self.frame(viewport);
                let Some(item) = frame.items.iter().find(|item| item.rect.contains(x, y)) else {
                    return MenuOutcome::Captured;
                };
                let Some(action) = self
                    .items
                    .get(item.index)
                    .and_then(|item| item.action.clone())
                else {
                    return MenuOutcome::Captured;
                };
                self.open = false;
                self.hovered = None;
                MenuOutcome::Invoke(action)
            }
            MenuEvent::Scroll { delta, x, y } => {
                let Some(rect) = self.frame(viewport).rect else {
                    return MenuOutcome::Captured;
                };
                if !rect.contains(x, y) {
                    return MenuOutcome::Captured;
                }
                let max_scroll = self.items.len().saturating_sub(usize::from(rect.height));
                self.scroll = if delta > 0 {
                    self.scroll
                        .saturating_sub(usize::from(delta.unsigned_abs()))
                } else {
                    self.scroll
                        .saturating_add(usize::from(delta.unsigned_abs()))
                        .min(max_scroll)
                };
                self.hovered = None;
                MenuOutcome::Captured
            }
        }
    }

    fn close(&mut self) -> MenuOutcome {
        self.open = false;
        self.hovered = None;
        MenuOutcome::Closed
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const VIEW: Viewport = Viewport {
        width: 60,
        height: 10,
    };

    fn session_menu(x: u16, y: u16, id: &str) -> TabContextMenu {
        TabContextMenu::new(
            x,
            y,
            MenuTarget::Session {
                session: SessionRef(id.into()),
                title: "Known title".into(),
                can_add: true,
            },
        )
    }

    #[test]
    fn session_rows_match_upstream_order_and_keep_stable_identity() {
        let menu = session_menu(5, 0, "session-uuid");
        assert_eq!(
            menu.items()
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            ["New session", "Rename", "Copy session ID", "Close"]
        );
        assert_eq!(
            menu.items()[1].action,
            Some(MenuAction::Rename {
                session: SessionRef("session-uuid".into()),
                current_title: "Known title".into(),
            })
        );
        assert_eq!(
            menu.items()[2].action,
            Some(MenuAction::CopySessionId {
                session: SessionRef("session-uuid".into())
            })
        );
        assert_eq!(
            menu.items()[3].action,
            Some(MenuAction::Close {
                session: SessionRef("session-uuid".into())
            })
        );
    }

    #[test]
    fn absent_add_facility_omits_only_the_new_session_action() {
        let menu = TabContextMenu::new(5, 0, MenuTarget::Session {
            session: SessionRef("exact-id".into()), title: "title".into(), can_add: false,
        });
        assert_eq!(menu.items().iter().map(|item| item.label.as_str()).collect::<Vec<_>>(),
            ["Rename", "Copy session ID", "Close"]);
        assert_eq!(menu.items()[2].action, Some(MenuAction::Close {
            session: SessionRef("exact-id".into()),
        }));
    }

    #[test]
    fn absent_reopen_facility_does_not_invent_a_recent_history_section() {
        let menu = TabContextMenu::new(5, 0, MenuTarget::New { recently_closed: None });
        assert!(menu.items().is_empty());
        assert!(menu.frame(VIEW).rect.is_none());
        let empty = TabContextMenu::new(5, 0, MenuTarget::New { recently_closed: Some(Vec::new()) });
        assert_eq!(empty.items().len(), 2);
        assert!(empty.items().iter().all(|item| item.action.is_none()));
    }

    #[test]
    fn horizontal_popup_matches_captured_origin_and_rows() {
        let menu = session_menu(5, 0, "id");
        let frame = menu.frame(VIEW);
        assert_eq!(
            frame.rect,
            Some(MenuRect {
                x: 5,
                y: 1,
                width: 20,
                height: 4
            })
        );
        assert_eq!(
            frame
                .items
                .iter()
                .map(|item| item.rect.y)
                .collect::<Vec<_>>(),
            [1, 2, 3, 4]
        );
    }

    #[test]
    fn vertical_edge_popup_clamps_to_viewport() {
        let menu = session_menu(35, 7, "id");
        let rect = menu.frame(VIEW).rect.unwrap();
        assert_eq!(
            rect,
            MenuRect {
                x: 35,
                y: 6,
                width: 20,
                height: 4
            }
        );
        assert!(rect.x + rect.width <= VIEW.width && rect.y + rect.height <= VIEW.height);
    }

    #[test]
    fn new_target_matches_recently_closed_capture_order() {
        let menu = TabContextMenu::new(
            40,
            0,
            MenuTarget::New {
                recently_closed: Some(vec![ClosedTab {
                    session: SessionRef("older-id".into()),
                    title: "Earlier session".into(),
                }]),
            },
        );
        assert_eq!(
            menu.items()
                .iter()
                .map(|item| item.label.as_str())
                .collect::<Vec<_>>(),
            ["Recently closed tabs", "Earlier session"]
        );
        assert_eq!(
            menu.items()[1].action,
            Some(MenuAction::Reopen {
                session: SessionRef("older-id".into())
            })
        );
        let empty = TabContextMenu::new(
            40,
            0,
            MenuTarget::New {
                recently_closed: Some(Vec::new()),
            },
        );
        assert_eq!(empty.items()[1].label, "No recently closed tabs");
        assert!(empty.items()[1].action.is_none());
    }

    #[test]
    fn long_labels_and_tiny_viewports_remain_bounded() {
        let menu = TabContextMenu::new(
            99,
            99,
            MenuTarget::New {
                recently_closed: Some(vec![ClosedTab {
                    session: SessionRef("stable".into()),
                    title: "界".repeat(80),
                }]),
            },
        );
        let frame = menu.frame(Viewport {
            width: 7,
            height: 1,
        });
        assert_eq!(
            frame.rect,
            Some(MenuRect {
                x: 0,
                y: 0,
                width: 7,
                height: 1
            })
        );
        assert_eq!(frame.items.len(), 1);
        assert!(frame.items[0].rect.contains(6, 0));
        assert!(menu.frame(Viewport::default()).rect.is_none());
    }

    #[test]
    fn hover_disabled_rows_outside_click_and_escape_are_truthful() {
        let mut menu = TabContextMenu::new(
            4,
            0,
            MenuTarget::New {
                recently_closed: Some(Vec::new()),
            },
        );
        let frame = menu.frame(VIEW);
        let first = frame.items[0].rect;
        menu.handle(
            MenuEvent::PointerMove {
                x: first.x,
                y: first.y,
            },
            VIEW,
        );
        assert!(
            !menu.frame(VIEW).items[0].hovered,
            "section labels are disabled"
        );
        assert_eq!(
            menu.handle(
                MenuEvent::PointerUp {
                    button: MouseButton::Left,
                    x: first.x,
                    y: first.y
                },
                VIEW
            ),
            MenuOutcome::Captured
        );
        assert!(menu.is_open());
        assert_eq!(
            menu.handle(
                MenuEvent::PointerDown {
                    button: MouseButton::Left,
                    x: 59,
                    y: 9
                },
                VIEW
            ),
            MenuOutcome::Closed
        );
        assert!(!menu.is_open());

        let mut menu = session_menu(5, 0, "id");
        assert_eq!(menu.handle(MenuEvent::Escape, VIEW), MenuOutcome::Closed);
        assert!(!menu.is_open());
    }

    #[test]
    fn menu_hover_and_copy_action_use_same_row_and_identity() {
        let mut menu = session_menu(5, 0, "exact-id");
        let frame = menu.frame(VIEW);
        let row = frame.items[2].rect;
        menu.handle(MenuEvent::PointerMove { x: row.x, y: row.y }, VIEW);
        assert!(menu.frame(VIEW).items[2].hovered);
        assert_eq!(
            menu.handle(
                MenuEvent::PointerUp {
                    button: MouseButton::Left,
                    x: row.x,
                    y: row.y
                },
                VIEW
            ),
            MenuOutcome::Invoke(MenuAction::CopySessionId {
                session: SessionRef("exact-id".into())
            })
        );
        assert!(!menu.is_open());
    }

    /// Proposed test-owner replacement for the retired same-row down/up test.
    /// Pinned-source release semantics are evidenced by `tab-menu-release-contract-main/release_red.rs`.
    #[test]
    fn release_on_close_after_down_on_new_invokes_close() {
        let mut menu = session_menu(5, 0, "target-id");
        let frame = menu.frame(VIEW);
        let start = frame.items[0].rect;
        let close = frame.items[3].rect;
        assert_eq!(
            menu.handle(
                MenuEvent::PointerDown {
                    button: MouseButton::Left,
                    x: start.x + 1,
                    y: start.y,
                },
                VIEW,
            ),
            MenuOutcome::Captured
        );
        assert_eq!(
            menu.handle(
                MenuEvent::PointerUp {
                    button: MouseButton::Left,
                    x: close.x + 1,
                    y: close.y,
                },
                VIEW,
            ),
            MenuOutcome::Invoke(MenuAction::Close {
                session: SessionRef("target-id".into()),
            })
        );
        assert!(!menu.is_open());
    }

    /// Proposed test-owner replacement for the retired same-row down/up test.
    #[test]
    fn release_on_close_without_prior_menu_press_invokes_close() {
        let mut menu = session_menu(5, 0, "target-id");
        let close = menu.frame(VIEW).items[3].rect;
        assert_eq!(
            menu.handle(
                MenuEvent::PointerUp {
                    button: MouseButton::Left,
                    x: close.x + 1,
                    y: close.y,
                },
                VIEW,
            ),
            MenuOutcome::Invoke(MenuAction::Close {
                session: SessionRef("target-id".into()),
            })
        );
        assert!(!menu.is_open());
    }

    #[test]
    fn recently_closed_rows_are_limited_and_reopen_by_id() {
        let closed = (0..MAX_RECENTLY_CLOSED + 2)
            .map(|index| ClosedTab {
                session: SessionRef(format!("id-{index}")),
                title: format!("tab-{index}"),
            })
            .collect();
        let menu = TabContextMenu::new(
            40,
            0,
            MenuTarget::New {
                recently_closed: Some(closed),
            },
        );
        assert_eq!(menu.items().len(), MAX_RECENTLY_CLOSED + 1);
        assert_eq!(
            menu.items()[1].action,
            Some(MenuAction::Reopen {
                session: SessionRef("id-0".into())
            })
        );
        assert_eq!(
            menu.items()[MAX_RECENTLY_CLOSED].action,
            Some(MenuAction::Reopen {
                session: SessionRef("id-9".into())
            })
        );
    }
}
