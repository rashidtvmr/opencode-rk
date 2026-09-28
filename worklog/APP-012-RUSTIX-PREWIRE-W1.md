# APP-012-RUSTIX-PREWIRE-W1

Session ses_f1df5aeefffeC0qRrzE38et5TH. Base 3ecee10 = origin/red/APP-012-PARENT-TOCTOU-W1 (exact, verified rev-parse).
Scope: dependency prewire only. No implementation, no security claim, Cargo.lock untouched.

Change (sole product file crates/tools/Cargo.toml, +1 line, alphabetical, style-matched):
  rustix = { workspace = true, features = ["fs"] }

Evidence:
- 3ecee10 root Cargo.toml [workspace.dependencies] lines 25-44: NO rustix entry (grep miss; transitive
  rustix 1.1.4 exists in Cargo.lock:1296). origin/main Cargo.toml also has no rustix.
- Therefore workspace=true is unresolvable until a root-lane adds
  rustix = { version = "1" } to [workspace.dependencies]. Root Cargo.toml NOT owned by this lane
  (task card forbids root workspace manifest edits); a trial root edit was reverted, final diff excludes it.
- Validation: python3.12 tomllib read-only parse of both manifests OK; no cargo command run
  (any cargo metadata/check would rewrite Cargo.lock, forbidden). git diff: exactly one product file.

Handoff to orchestrator: needs companion root-lane edit (one line, [workspace.dependencies]) +
separate Cargo.lock refresh lane. Until then tools crate build fails dep resolution; that is a
prewire ordering constraint, not a defect in this file. Status blocked reflects root-dependency gap only.
