# BRIDGE-GAP-39

- Claim: ses_gap39 via tools/completion_claims.py, ledger in-progress.
- Evidence: theme_registry.rs luminance split 128, muted extremes 180/75 (muted_text_color); gray extreme branches dark/light.
- Target: crates/opentui-bridge/src/theme_generate.rs only.
- Impl: generate_system 8-step black->bg(idx3)->white, gray_scale linear step*255/7 clamp, muted_text <128->180 else 75; const fns; forbid(unsafe_code); std-only.
- Tests: scale len, endpoints, names echoed, gray endpoints, gray monotonic, muted dark/light (7 tests).
- Unknowns: none.
