# APP-012-RUSTIX-WORKSPACE-W1

Claim: session ses_f1dd641fdffeTq0Iq1REw6D2kr, ledger tasks/completion/claims.json, file: Cargo.toml (+ this worklog).
Base: e5eb9b9 origin/prewire/APP-012-RUSTIX-W1, worktree /private/var/folders/.../prewire-app012-rustix-workspace-w1, branch prewire/APP-012-RUSTIX-WORKSPACE-W1.

## Source evidence
- Cargo.toml:25 [workspace.dependencies] alphabetized; rusqlite then serde (line 31/33 pre-edit).
- crates/tools/Cargo.toml:10 `rustix = { workspace = true, features = ["fs"] }` — requires workspace entry (missing before edit).
- Cargo.lock:1296 rustix 1.1.4 checksum bfe4565... single lock package.

## Change
`rustix = "1.1.4"` inserted after `rusqlite` (alphabetical rs<ru<se). No workspace-level features, no default-features key.

## Decisions
- Bare version string = cargo default-features on for rustix; safe (rustix defaults non-destructive, no unsafe broad features). tools adds `fs` only. Workspace level intentionally feature-free so other crates opt in per-crate.
- Exact 1.1.4 matches Cargo.lock already committed in base e5eb9b9 (rustix present via transitive/other path) → lock-compatible, no Cargo.lock edit (out of scope).
- Verified without cargo: python re-based structural check: workspace deps alphabetical, exact line `rustix = "1.1.4"`, tools req `workspace=true features=["fs"]` resolves to workspace version 1.1.4, lock entry 1.1.4. Output: OK.
- `tomllib` unavailable under repo python (3.9.6); used regex/section parse instead. Cargo parse + lock refresh = separate lane.

## Remaining
- Cargo.lock refresh / `cargo metadata` validation not run here (explicitly excluded; separate lane).
- No security claim.
