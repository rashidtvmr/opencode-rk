# BRIDGE-PAR-203 scratchpad

claim: BRIDGE-PAR-203 via ses_par203 ok.
evidence:
- crates/opentui-bridge/src/run_runtime_stdin.rs:11-46 (StdinProbe, resolve_probe, probe_label, labels tty/piped/closed)
- crates/opentui-bridge/src/native_frame.rs:19 (OFFLINE_TITLE="OpenCode RK — offline"), :59 (title None renders offline banner)
target: crates/opentui-bridge/src/offline_banner.rs only. No lib.rs/Cargo.toml edits.
tests: title format+cap, seed format, is_offline both ways.
decisions: ASCII "--" per style rules; char-count cap 256 via chars().take.
