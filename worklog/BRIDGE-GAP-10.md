# BRIDGE-GAP-10: single-slot toast holder (controller truth)

Claim: `crates/opentui-bridge/src/toast_single.rs` (new). Session `ses_gap10`.

Source evidence:
- `crates/opentui-bridge/src/toast.rs:50-87` `ToastOptions { title?, message, variant, duration_ms }` + `effective_duration_ms` (`None` -> `DEFAULT_DURATION_MS` = 5000, `:14-15`); `ToastQueue` (`:91-125`) bounded FIFO, DIVERGES from TS single-slot.
- `crates/opentui-bridge/src/toast_view.rs:37-71` `ToastProvider` view-layer single slot (show replace / clear / current, no expiry).
- TS truth per brief: `packages/tui/src/ui/toast.tsx:60-67` single `currentToast`, replace-on-new (file absent in repo; brief is authority).

Observed: provider covers view slot but leaves expiry to host; FIFO queue contradicts TS. Gap = controller truth with deterministic expiry.

Target boundary: ONE new file only. No edits to `toast.rs` / `toast_view.rs` / `lib.rs` (integrator prewires `pub mod toast_single;`).

Tests (in-file, frozen at write): replace-on-new, clear, expiry via caller clock, default 5000. All green via standalone harness (lib.rs untouched so crate build excludes file until integrator wires it).

Decisions:
- Caller clock `now_ms: u64` (no `Instant`, deterministic, std-only).
- Expiry `now.saturating_sub(shown_at) >= effective_duration_ms()` (overflow-safe; duration 0 expires immediately; boundary `>=` so deadline instant counts expired).
- `current()` is non-mutating (returns None when expired, keeps state; caller calls `clear()`); documented in rustdoc.
- `is_expired` on empty slot = false.

Remaining: integrator wire `pub mod toast_single;` + run `cargo test -p opencode-rk-opentui-bridge toast_single`.
