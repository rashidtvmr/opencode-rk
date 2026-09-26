# BRIDGE-056 toast_view scratchpad

Claim: toast view slot mirroring packages/tui/src/ui/toast.tsx:15-102.
Source evidence:
- opencode packages/tui/src/ui/toast.tsx:7-12 ToastOptions, :23-38 border variant colors + top2/right2 + width min(60,width-6), :54-67 single currentToast replace-on-new + timeout, :69-79 error(err) helper, :89-102 ToastProvider/useToast + must-be-within error.
- opencode-rk crates/opentui-bridge/src/toast.rs: ToastOptions/effective_duration_ms/toast_width/ToastQueue reused, not redefined.
Target boundary: ONLY crates/opentui-bridge/src/toast_view.rs. No lib.rs edit, no cargo run.
Tests: 7 tests in-file (error_message x2, border_color, position, replace-on-new, show_error, use_toast). Logically green by inspection, not compiled.
Decisions: empty String = non-Error fallback; Position unit struct with TOP/RIGHT consts; ToastProvider single Option slot; queue divergence documented in module doc.
Unknowns: lib.rs lacks `pub mod toast_view;` so module is unwired until integrator adds it. That edit is out of scope for this lane.
