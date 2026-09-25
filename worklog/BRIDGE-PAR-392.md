# BRIDGE-PAR-392 (UNCLAIMED - orchestrator owns claims.json, file-only per spawn prompt)

## Claim
Not claimed (per instructions: do NOT touch claims.json). File-only lane.

## Source evidence
- TS truth: /home/rashid/projects/opencode/packages/tui/src/parsers-config.ts (34 filetype entries + aliases udiff/patch->diff, makefile->make; builtins markdown/javascript/typescript need no wasm)
- Rust truth (read-only): crates/opentui-bridge/src/parsers_config.rs (static LANG_PARSERS table, lookup/count; NOT edited)
- Style model: crates/opentui-bridge/src/builtins_config_full.rs:1-41 (bool register, cap check; NOT edited)

## Target boundary
- ONE new file: crates/opentui-bridge/src/parsers_cfg_full.rs
- NOT edited: lib.rs, Cargo.toml, parsers_config.rs
- No cargo, no commit (per scope)

## Tests
- 4 tests in-file: register_and_has, rejects_dupe_empty_long, enforces_cap_16, missing_absent
- Frozen: written before/once with impl, zero edits after green (only impl golfed register guard merge; tests stable)

## Decisions
- ParsersCfg {names: Vec<String>} cap 16 / each 1..=64, register->bool, has/len/is_empty; std-only, forbid(unsafe_code)
- Names-only on purpose: static URL table lives in parsers_config.rs; this file is the bounded mutable registry companion. DIVERGENCE noted in doc comment.
- 78 lines (<80), rustfmt --check PASS

## Remaining
- Needs wiring into lib.rs by orchestrator/integrator (out of scope).
- rustc/cargo test NOT run (out of scope per prompt; only rustfmt --check).
