# BRIDGE-PAR-289 scratchpad (ses_par289)

Claim: in-progress via completion_claims (ses_par289, worklog/BRIDGE-PAR-289.md).
Source evidence:
- TS truth: /home/rashid/projects/opencode/packages/tui/src/util/model.ts:3-6 `parse` splits on first `/` (provider=first seg, model=rest joined).
- Sibling: crates/opentui-bridge/src/util_model.rs:13-18 `split_model` first-`/` split; strict `is_valid_model` there (both parts non-empty) — this lane is the SHORT/display variant, different fn set, no conflict (separate module, lib.rs untouched).
Target boundary: ONE new file crates/opentui-bridge/src/model_util_full.rs. No lib.rs, no Cargo.toml, no cargo, no commit.
Tests: 5 in-file (short slash, short colon/bare/empty, short 64-cap, provider cases, valid alnum). Written before verification.
Decisions:
- `model_short`: rsplit on ['/',':'] tail, chars().take(64). Bare id passes through; empty stays empty.
- `provider_of`: split head, empty/absent -> "default" const, chars().take(32).
- `is_valid_model`: non-empty + any ascii alnum (looser than util_model.rs strict form, per card).
- std-only, forbid(unsafe_code), <100 lines.
Remaining: rustfmt --check, ledger flip.
