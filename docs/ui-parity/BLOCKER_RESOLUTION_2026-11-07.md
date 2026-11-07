# UI-parity blocker resolution — 2026-11-07

Base SHA: `7088398873e835bbe303cec43dcd4ad9f3606712`
Upstream oracle: `450e7b92412c780a7b5a75226bb8d49aaca24e22`

This wave took the residual "blocker" set (the obligations still marked
`unverified_rust` after wave-112) and, for each, either proved it already TRUE in
the live app, implemented the missing live wiring, or recorded the exact
producer/consumer seam that blocks it. No obligation is claimed from source
presence alone.

| Lane | Obligation | Outcome | Evidence |
| --- | --- | --- | --- |
| L52 | LAY-11 | **CONFIRMED** | Live render uses `native_view_layout::right_pane` (`native_view.rs:5145-5157`); test `live_render_uses_right_pane_precedence_for_open_dock`. |
| L52 | LAY-10 | **BLOCKED** | `panel_can_split`/`panel_presentation`/`panel_toggle_fullscreen` (`native_pane_resize.rs:551-585`) have no live consumer; the native "panel" slot is the composer dock (`dock_open()`), a bottom dock, not the upstream plugin panel. No 107/108 fullscreen switch surface exists. |
| L53 | LAY-13 | **BLOCKED** | `PaneResize`/`frame_resize_handle`/`pane_resize_handle` (`native_pane_resize.rs`) have zero consumers in `native_view.rs`/`tui_entry.rs`; no live pointer target reaches the resize handle. |
| L54 | SIDE-05 | **BLOCKED** | `StatusView::set_limit` (`native_view_status.rs:1260`) is test-only; no `/api/experimental/usage/limits` route, no core usage-limits mirror, no `UsageLimits` contract. The data sources from the external OpenCode Console API. |
| L55 | DIALOG-07 | **CONFIRMED** | `dialog07_tests` (`native_view_dialogs.rs:11861`): `dialog07_row_footer_paints_right_aligned`, `dialog07_current_row_keeps_its_own_footer_after_selection_moves`, `dialog07_flatten_uses_search_footer_then_category`, `dialog07_long_title_truncates_and_keeps_footer`, plus `dialog05_paints_category_and_detail_rows_in_viewport`. |
| L56 | DIALOG-15 (integration) | **CONFIRMED** | `IntegrationPhase` (`native_palette.rs:2930-2995`) covers EnvironmentOnly/Accounts/Methods/Form/Progress/Connected/Cancelled; test `integration_phases_cover_accounts_methods_progress_and_cancel`; live flow at `tui_entry.rs:10205,11351,16583,17844,17888`; test `dialog15_live_enter_opens_integration_surface`. |
| L56 | DIALOG-15 (MCP auth) | **BLOCKED** | Bundled server never emits `needs_auth`/`needs_client_registration` (`crates/server/src/mcp_http.rs:32`), so the MCP sign-in gate is unreachable live. Model + test exist (`dialog15_mcp_enter_routes_signin_error_and_ignore`). |
| L57 | DIALOG-20 | **IMPLEMENTED** | Live toast hover now pauses/resumes the lifetime (`toast_hover_transition`, called from the pointer loop); crash surface forwards Home/End (`crash_surface_key`). New tests `dialog20_hover_pauses_and_leave_resumes_the_live_toast`, `dialog20_crash_home_and_end_scroll_the_stack`. Commit `fec82026`. |
| L58 | THEME-14 | **CONFIRMED** | `SyntaxStyleArena` (`native_theme_runtime.rs:2471`) retains replaced styles until renderer idle; live at `tui_entry.rs:10629`; tests `theme14_syntax_replacement_is_retained_until_renderer_idle`, `theme14_live_streaming_replacement_retains_style_until_frame_end`. |
| L59 | MEDIA-001 | **CONFIRMED (structural)** | 8 tests in `native_image_preview.rs`/`native_view_chat.rs`/`native_view.rs`; live gate `set_image_preview_enabled` (`tui_entry.rs:10700`) and gallery click `open_image_preview_page_at` (`tui_entry.rs:15354`). Decode half remains BLOCKED (no image decoder; layout/state model only). |
| L60 | FFI-02 | **CONFIRMED** | `Scene` (insert/insert_at/move_child/remove/set_style/set_text/set_focus), `Style`/`Dimension`/`Edges`, `Editor`, text buffers; tests incl. `ffi02_destroy_subtree_stales_handles`, `ffi02_ordered_move_preserves_identity_and_tag`, `ffi05_editor_view_wraps_wide_chars`. |
| L61 | FFI-03 | **CONFIRMED** | `Arena`/`GenHandle`/`SharedResource`; tests `arena_stale_generation_rejected`, `arena_retires_exhausted_generation`, `failed_construction_leaves_no_handle`, `shared_resource_explicit_lifetime`, `arena_borrow_pairs_value_with_handle`, `constructing_panic_leaves_arena_usable`, `failed_construction_on_other_thread_is_wrong_thread`. |
| L62 | FFI-04 | **CONFIRMED** | `FrameController`; tests `host_hooks_must_drain_before_prepare`, `host_hook_queue_is_bounded`, `output_backpressure_blocks_admission_until_drained`, `present_requires_every_output_ticket_to_drain`, `output_failure_is_terminal_for_frames_but_not_for_shutdown`, `shutdown_waits_for_the_in_flight_frame`, `idle_shutdown_is_immediate_and_idempotent`. |
| L63 | FFI-06 | **CONFIRMED** | Tests `ffi06_display_text_conceals_markers`, `frame_text_never_flashes_raw_source`, `styled_runs_cover_visible_text_and_merge`, `styled_runs_are_empty_without_a_committed_result`, `late_result_is_discarded_after_destroy`. |
| L64 | FFI-11 | **CONFIRMED** | Tests `ffi11_focus_cycle_and_restore_after_destroy`, `ffi02_destroy_subtree_stales_handles`, `arena_swap_invalidates_the_previous_handle`, `late_result_is_discarded_after_view_replacement`. |
| L65 | PAR-02 | **IMPLEMENTED** | `docs/ui-parity/PAR-02-bridge-api-inventory.md` maps each capability row to a real Rust symbol. Commit `814dea74`. |
| L66 | PAR-03 | **IMPLEMENTED** | `tools/ui_parity/par03_capture.py` + `tests/test_par03_capture.py` capture the same deterministic fixture through the pinned upstream adapter and real Rust PTY adapter. Residual: byte-exact parity still differs in `rows`/`cursor`/`focus`. Commit `814dea74`. |
| L67 | PAR-06 | **IMPLEMENTED** | `docs/ui-parity/PAR-06-capture-environment.md` + `tools/ui_parity/capture_environment.py`. Commit `eaf812da`. |
| L68 | PAR-09 | **IMPLEMENTED** | Test `par09_partial_fence_body_is_never_blessed_as_blank` (`native_markdown_flow.rs`). Commit `eaf812da`. |

## Remaining blocked seams (need a product surface or an external source)

- **LAY-10 / LAY-13** — require a live plugin-panel / terminal right-pane producer.
  The native app has only the composer dock; wiring `PaneResize` to it would be a
  new product surface, not a parity fix.
- **SIDE-05** — requires the usage-limits producer chain (external Console API).
- **DIALOG-15 MCP-auth half** — requires the server to emit `needs_auth`.
- **MEDIA-001 decode half** — requires an image decoder; the layout/state model is complete.

## Verification

- `cargo test -p opencode-rk-opentui-bridge` → 219 passed (FFI-02/03/04/06/11).
- `cargo test -p opencode-rk-cli --bin oc2 dialog20_` → 6 passed (DIALOG-20).
- Full CLI suite green on the integrated SHA (see the wave-114 ledger entry).
