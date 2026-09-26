# BRIDGE-PAR-225 scratchpad

Claim: BRIDGE-PAR-225 via ses_par225. Owned file: crates/opentui-bridge/src/render_loop_full.rs.

Source evidence:
- crates/opentui-bridge/src/render_queue.rs:9 RenderQueue {dirty, pending}; :29 mark_dirty; :48 take()->(bool, Vec<String>) resets both; :16 MAX_NOTES 8; :18 MAX_NOTE_LEN 256.
- crates/opentui-bridge/src/frame_assemble.rs:13 assemble_frame(title,status,transcript,draft,toast,width,height)->Vec<String>; toast replaces last line; height.max(8) rows.

Target boundary: RenderLoop {queue, frames} + request (mark_dirty) + render (->None when clean; Some frame + frames bump + take reset). std-only, forbid(unsafe_code), <130 lines, >=5 tests. No edits to lib.rs/Cargo.toml/render_queue/frame_assemble. No cargo/commit.

Tests: new_none; request_some_bump; clean_after_render_none; re_request_renders; note_becomes_toast; height_clamp.

Decisions: first queued note -> toast (ponytail: join policy later). take() before dirty check so notes reset even when clean. frames u64 saturating? plain +=1 sufficient.

Unknowns: none.
