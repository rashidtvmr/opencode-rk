# FIX-02 scratchpad

Claim: compose_calls + frame_line in paint_compose_full.rs.
Evidence: layout.rs:11-32 Rect(u32)+is_empty; world.rs:143-156 Region+PAINT_ORDER; text.rs:34-36 clip_chars precedent.
Scenario: bridge-side pure compose mirroring world.rs paint_calls but flattened u16 tuples.
Boundary: ONE file paint_compose_full.rs. No edit lib.rs/world.rs/layout.rs.
Tests: skips_empty, all_empty, paint_order, frame_line_clips_and_bounds (4).
Decisions: saturating min(u16::MAX) cast fixes silent truncation; index-aligned rects array removes rect_of helper.
Unknowns: none. rustfmt PASS. No cargo run per scope.
