# LANE-RULES — scratchpad

Session: ses_worker_rules
Task: LANE-RULES
Owned file: crates/server/src/rules_loader.rs (NEW)

## Claim
AGENTS.md / CLAUDE.md / rules-folder loader for system prompt injection.
Pure-state + fs-read module. Parity gap: OpenCode rule loading.

## Evidence
- File: crates/server/src/rules_loader.rs
- Frozen sha256: be2442cdf1c116a571dd6904860ea82a862472514314eb010ab9a92d91487e88
- Commit: 26928c6 (pushed to origin/main)
- Tests: 6/6 GREEN x3 stable
  - T01 AGENTS.md discovered
  - T02 frontmatter glob parsed
  - T03 oversize file skipped
  - T04 symlink escape rejected
  - T05 deterministic ordering (alphabetical)
  - T06 empty workspace -> empty snapshot
- Verify: `cd crates/server && rustc --edition 2021 --test src/rules_loader.rs`

## Design decisions
- std-only, forbid(unsafe_code)
- parse_frontmatter: YAML `---` delimited, single `globs:` key
- canonicalize_reject_escape: follows symlinks, rejects if resolved path leaves workspace
- Deterministic ordering: sort by full path (BTreeMap not needed, Vec::sort suffices)
- Bounds: MAX_FILES=64, MAX_FILE_BYTES=64KiB, total 512KiB
- Oversized files silently skipped (still count toward file limit)
- Budget exceeded returns error before loading the offending file

## Remaining unknowns
- Integrator must wire `pub mod rules_loader` into server/lib.rs
- No CLI/server entrypoint wiring in this lane
