# LANE-MAIN-ONCE2 scratchpad

Claim: LANE-MAIN-ONCE2 in-progress.

Evidence:
- main.rs Cli adds non-global --once; NativeTui arm forwards cli.once into TuiArgs.
- Tui arm resolves --data-dir then sets OPENCODE_RK_HOME (until tui_entry typed channel lands).
- check: cargo check -p opencode-rk-cli --bin opencode-rk green.
- runtime: tui --once + --data-dir tui --once both render frame exit 0.
