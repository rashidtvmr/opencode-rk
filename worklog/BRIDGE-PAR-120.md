# BRIDGE-PAR-120 scratchpad

claim: BRIDGE-PAR-120 via cc.claim session ses_par120 OK
source: /home/rashid/projects/opencode/packages/tui/src/util/signal.ts:1-51
- createDebouncedSignal: set stages next, timer fires once delay_ms after last set, clearTimeout on re-set
- createFadeIn: hidden->alpha 0; !animate||revealed->revealed=true alpha 1; else revealed=true, 160ms smoothstep p*p*(3-2p) polled every 16ms
boundary: ONE file crates/opentui-bridge/src/debounce_full.rs, no lib.rs/Cargo.toml/debounce.rs/debounce_signal.rs edits, no cargo, no commit
target: no-timer port Debounced{value,pending,delay_ms}+push/fire/get; Fade{alpha_x1000,revealed}+fade_step; std-only forbid(unsafe_code) <170 lines; >=5 tests
decision: fire=explicit commit (clears pending, true/false); delay_ms stored metadata only; fade keeps revealed sticky on hidden (matches TS: hidden path never clears revealed); smoothstep via f64 round, min(elapsed/160,1)
tests: fire_commits, fire_none_false, fade_hidden, fade_instant, fade_partial_range, fade_full_revealed_sticky
verify: rustfmt --check only
