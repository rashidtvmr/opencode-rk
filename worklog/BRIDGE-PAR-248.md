# BRIDGE-PAR-248 scratchpad

- claim: BRIDGE-PAR-248 via cc.claim, session ses_par248, scratchpad worklog/BRIDGE-PAR-248.md, ok
- source: crates/opentui-bridge/src/debounce_full.rs:1-44 (Debounced last-push-wins + fire, Fade 160ms smoothstep)
- target: crates/opentui-bridge/src/debounce_full2.rs only, no lib.rs/Cargo.toml/debounce_full.rs edits
- design: DebounceFull { pending Option<String> 4KiB, ticks u32, wait u32 }; push(&str) truncates char-boundary + ticks=0; poll ticks.saturating_add(1), fire when >= wait (wait 0 = immediate), reset ticks, take pending; set_wait bool = changed
- tests: 6 covering fire-after-wait, pending-none, last-push-wins reset, 4KiB cap, set_wait bool, wait-zero
- verify: rustfmt --check only, no cargo/commit
