# APP-001-CLI-PROVIDERS-DEP-W1 scratchpad

Claim: task `APP-001-CLI-PROVIDERS-DEP-W1`, session `ses_prewire_W1`, ledger in-progress confirmed.
Base: exact receipt `f22e0e8` (verify APP-001 File0600 backend at d137a09).
Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/prewire-app001-cli-providers-w1`, branch `prewire/APP-001-CLI-PROVIDERS-W1`.

## Source evidence
- `crates/cli/Cargo.toml:20-30` (at f22e0e8): `[dependencies]` path deps catalog, contracts, opentui-bridge(optional), sessions, server, storage, tools. No providers dep.
- `crates/providers/Cargo.toml:1-2`: `name = "opencode-rk-providers"`, exists at `../providers` relative to cli.
- Adjacent style: `crates/server/Cargo.toml` uses `opencode-rk-providers = { path = "../providers" }` (plain path dep, no features). Followed same form.
- Alphabetical order in cli deps: catalog, contracts, opentui-bridge, providers, sessions, server, storage, tools. Note: pre-existing file already had sessions before server (not alphabetical); inserted providers after opentui-bridge, before sessions, preserving neighbors byte-identical.

## Observed scenario
- Target boundary: add one line `opencode-rk-providers = { path = "../providers" }` to cli `[dependencies]`. No features (no source evidence requires any; server precedent uses none).
- No source/tests/root manifest/Cargo.lock edits. `git status --porcelain` shows only `M crates/cli/Cargo.toml`.

## Verification (no cargo lock mutation per directive)
- Manifest inspection only: cli dep line present; providers package name matches; providers manifest contains no `opencode-rk-cli` reference, so no dependency cycle.
- `tomllib` unavailable in host python3, used literal/regex manifest check instead: `TOML-manifest check ok`.
- No cargo commands run (lock refresh explicitly separate lane). No test suite for manifest-only prewire; downstream lock/main.rs lanes verify build.

## Decisions
- Single-line insertion, adjacent style, alphabetical placement.
- Scratchpad + ledger update committed in main repo dir (not worktree) to avoid clobbering claims.json on stale base.

## Remaining unknowns
- None for this lane. Lock refresh and main.rs setup wiring are separate lanes.
