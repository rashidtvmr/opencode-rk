# BRIDGE-GAP-34: event_wire.rs loop-level pump
Claim: BRIDGE-GAP-34, session ses_gap34, in-progress (pre-fenced by orchestrator).
Source evidence:
- crates/opentui-bridge/src/loop_events.rs:22 HostAction {Key(u32), Paste(String), Resize, Quit, Page(Page), Submit}; Page at :32.
- crates/opentui-bridge/src/events.rs:29 EventBus (bounded FIFO, cap MAX_EVENTS=256); task prompt said core_events::EventBus but core_events.rs:13 only does `use crate::events::EventKind` - no EventBus there. Real path crate::events::EventBus used.
- lib.rs has `pub mod events;` (`pub mod core_events;`, `pub mod loop_events;` assumed siblings; NOT editing lib.rs per scope).
Target boundary: ONE new file crates/opentui-bridge/src/event_wire.rs, std-only, forbid(unsafe_code), <200 lines, LoopEvent + pump + 6 in-file tests.
Decisions: bare Resize -> Resize(80,24); Quit -> Shutdown; rest -> Input passthrough; VecDeque staging drops oldest past MAX_LOOP_EVENTS=128; pump_into() wires EventBus notify (Shutdown has no EventKind, pushes nothing).
Unknowns: wiring into lib.rs / loop caller is orchestrator's job (out of scope).
