#![forbid(unsafe_code)]
//! Loop-level event pump over [`EventBus`].
//!
//! Maps decoded [`HostAction`]s (see `loop_events`) into [`LoopEvent`]s the
//! render loop consumes. Pure, bounded, std-only: at most
//! [`MAX_LOOP_EVENTS`] events are kept and the oldest is dropped on overflow.

use std::collections::VecDeque;

use crate::events::{EventBus, EventKind};
use crate::loop_events::HostAction;

/// Max queued [`LoopEvent`]s. Overflow drops the oldest.
pub const MAX_LOOP_EVENTS: usize = 128;
/// Default terminal width for bare [`HostAction::Resize`] (wire carries none).
pub const DEFAULT_COLS: u32 = 80;
/// Default terminal height, see [`DEFAULT_COLS`].
pub const DEFAULT_ROWS: u32 = 24;

/// Loop-level event consumed by the render loop.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum LoopEvent {
    /// Key/paste/page/submit action, passed through untouched.
    Input(HostAction),
    /// Resize with concrete dimensions.
    Resize(u32, u32),
    /// Frame tick.
    Tick,
    /// Terminal shutdown request (from [`HostAction::Quit`]).
    Shutdown,
}

impl From<HostAction> for LoopEvent {
    fn from(action: HostAction) -> Self {
        match action {
            HostAction::Quit => Self::Shutdown,
            HostAction::Resize => Self::Resize(DEFAULT_COLS, DEFAULT_ROWS),
            other => Self::Input(other),
        }
    }
}

/// Map `actions` into [`LoopEvent`]s, oldest-first. Bounded: keeps at most
/// [`MAX_LOOP_EVENTS`], dropping the oldest on overflow.
#[must_use]
pub fn pump(actions: Vec<HostAction>) -> Vec<LoopEvent> {
    let mut out = VecDeque::new();
    for action in actions {
        if out.len() >= MAX_LOOP_EVENTS {
            out.pop_front();
        }
        out.push_back(LoopEvent::from(action));
    }
    out.into_iter().collect()
}

/// [`pump`] plus [`EventBus`] notification: each mapped event also pushes its
/// [`EventKind`] (`Input`/`Resize`/`Tick`). [`LoopEvent::Shutdown`] has no bus
/// kind and is handled by the loop directly, so it pushes nothing.
pub fn pump_into(bus: &mut EventBus, actions: Vec<HostAction>) -> Vec<LoopEvent> {
    let events = pump(actions);
    for event in &events {
        match event {
            LoopEvent::Input(_) => bus.push(EventKind::Input),
            LoopEvent::Resize(..) => bus.push(EventKind::Resize),
            LoopEvent::Tick => bus.push(EventKind::Tick),
            LoopEvent::Shutdown => {}
        }
    }
    events
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::loop_events::Page;

    #[test]
    fn quit_maps_to_shutdown() {
        assert_eq!(pump(vec![HostAction::Quit]), vec![LoopEvent::Shutdown]);
    }

    #[test]
    fn resize_maps_to_default_dims() {
        assert_eq!(
            pump(vec![HostAction::Resize]),
            vec![LoopEvent::Resize(DEFAULT_COLS, DEFAULT_ROWS)]
        );
    }

    #[test]
    fn actions_pass_through_as_input() {
        let actions = vec![
            HostAction::Key(65),
            HostAction::Paste("hi".into()),
            HostAction::Page(Page::Help),
            HostAction::Submit,
        ];
        assert_eq!(
            pump(actions),
            vec![
                LoopEvent::Input(HostAction::Key(65)),
                LoopEvent::Input(HostAction::Paste("hi".into())),
                LoopEvent::Input(HostAction::Page(Page::Help)),
                LoopEvent::Input(HostAction::Submit),
            ]
        );
    }

    #[test]
    fn cap_evicts_oldest() {
        let actions: Vec<HostAction> = (0..(MAX_LOOP_EVENTS + 10) as u32)
            .map(HostAction::Key)
            .collect();
        let events = pump(actions);
        assert_eq!(events.len(), MAX_LOOP_EVENTS);
        assert_eq!(events[0], LoopEvent::Input(HostAction::Key(10)));
        assert_eq!(
            events[MAX_LOOP_EVENTS - 1],
            LoopEvent::Input(HostAction::Key((MAX_LOOP_EVENTS + 9) as u32))
        );
    }

    #[test]
    fn empty_in_empty_out() {
        assert!(pump(vec![]).is_empty());
    }

    #[test]
    fn pump_into_notifies_bus() {
        let mut bus = EventBus::new();
        let events = pump_into(
            &mut bus,
            vec![HostAction::Key(65), HostAction::Resize, HostAction::Quit],
        );
        assert_eq!(events.len(), 3);
        assert_eq!(bus.len(), 2);
        assert_eq!(bus.drain()[0].kind, EventKind::Input);
    }
}
