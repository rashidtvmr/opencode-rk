# PHASE1-WAVE1-DISPATCH-MAP-CORRECTION

## Claim
- Task: `PHASE1-WAVE1-DISPATCH-MAP-CORRECTION`, type implementation (manifest correction only).
- Session: `ses_f2d73329affeBljDY1VFANGgTW`; branch `plan/phase1-wave1-dispatch`.
- Owned file: `worklog/PHASE1-WAVE1-DISPATCH-MAP.md` only.
- Role: minimal execution-manifest correction author. No lanes, no Cargo, no implementation.

## Source evidence
- Manifest: `worklog/PHASE1-WAVE1-DISPATCH-MAP.md` (candidate `0e6675b`).
- Verifier: `worklog/PHASE1-WAVE1-DISPATCH-MAP-VERIFY.md` at `0bba574`, verdict ACCEPT WITH CORRECTIONS, 6 required corrections.
- SEC-RED-OS blocked evidence: `bd8ddc9` (macOS host unsupported; status blocked; needs Linux runner + linked syscall backend).
- Gate anchored observation: `CONVERGENCE BLOCKED total=93` (this revision, after correction claim; verifier observed 92 at `0bba574`; count drifts with ledger).

## Observed scenario
Manifest stale vs verifier: Lane 1 shell RED presented as launch-now "new" file but already authored/verified on `origin/lane/TOOL-RED-SHELL` (`2cd9ca9`, hash `2853960b...`, ACCEPT WITH SPLIT `8c514e0`); all 4 commands carry macOS-absent `timeout 180` (exit 127); Lane 2 uses truncated `5d66683...` placeholder rejected by `crates/cli/build.rs` valid_revision; gate says 90 and 91 vs current 92/93; `free -h` Linux-only; MSVC exclusion unstated; SEC-RED-OS labeled active though blocked at `bd8ddc9`.

## Target boundary
Apply only the 7 listed corrections. Preserve parent-open warnings, queued blockers, existing cited hashes. No new lanes, research, Cargo runs, or acceptance.

## Tests (contract validation)
- `rtk git diff --check` clean.
- Stale-token parser: no `timeout`, no `5d66683...`, no `free -h`, no `SEC-RED-OS is active` / `SEC-RED-OS active`; explicit MSVC exclusion present; no duplicate owned files in launch table.

## Decisions
- Remove Lane 1 from launch-next (already-authored observation, not launchable); renumber to 3 verification lanes; breadth 4/14 -> 3/14.
- Lane 2 env uses full 40-hex revision (deterministic) instead of omitting.
- Resource schedule `Timeout` column renamed `Bound` to avoid stale `timeout` token; `free -h` replaced with `vm_stat`.
- Gate stated as anchored observation (93 current, 92 at verifier) with drift note.

## Remaining unknowns
- None for this correction. Pending later independent verifier confirmation.
