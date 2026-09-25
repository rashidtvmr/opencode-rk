# BRIDGE-PAR-181 scratchpad

claim: in-progress via cc.claim ses_par181. status honest.
source: crates/opentui-bridge/src/dialog_stack.rs:15 DialogStack (open/close/top), :26 open dup-to-top, MAX_STACK 8, MAX_ID 64. crates/opentui-bridge/src/timeline_dialog.rs:19 TimelineDialog {entries,cursor,open}, :57 selected.
observed: no dialog_host.rs existed; lib.rs untouched per scope.
target: new file dialog_host.rs only. DialogHost{stack,timeline}+open/close_top/top/timeline_selected. TIMELINE_ID="timeline" syncs open flag. std-only, forbid(unsafe_code).
tests: 6 tests in-file (roundtrip, flag raise/lower, empty none, selected delegates, long-id reject). rustfmt --check clean.
decisions: dup-to-top/full/long-id behavior inherited from DialogStack::open, not re-implemented. close_top only lowers flag when popped id == TIMELINE_ID so stacked "a"+timeline keeps "a".
unknowns: lib.rs wiring left to orchestrator (out of scope).
