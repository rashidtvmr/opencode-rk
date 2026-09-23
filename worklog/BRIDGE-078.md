# BRIDGE-078: bg-pulse fps pin

Claim: port `bg-pulse.tsx:74-86` fps pin to `bg_pulse.rs` additively.
Evidence: TS `bg-pulse.tsx:80-81` pins targetFps/maxFps=30; `:84-87` onCleanup restores captured pair.
Added: `FPS_PIN=30`, `FpsPin{previous:(u16,u16), pinned:bool}`, `pin()->FpsPin`, `target_fps()/max_fps()`, `restore()->(u16,u16)`; doc note "not ported" removed.
Tests: `fps_pin_pins_both_to_30`, `fps_restore_returns_previous_pair`, `fps_unpinned_reads_previous`; existing 5 untouched.
RED: new tests fail pre-impl (no symbols). GREEN (logical): pin->(30,30), restore->previous+unpinned; no cargo run per scope.
Unknowns: renderer wiring caller out of scope; u16 width matches existing PulseCell.
