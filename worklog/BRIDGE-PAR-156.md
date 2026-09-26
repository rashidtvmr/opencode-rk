# BRIDGE-PAR-156 location_ctx

claim: ses_par156 via completion_claims.claim, in-progress ok.
source: `packages/tui/src/context/location.tsx:1-14` LocationRef context, fail-closed useLocation; `context_session.rs:176` Location workspace-only precedent.
target: `crates/opentui-bridge/src/location_ctx.rs` LocationCtx{path cap1024,exists} set/clear/label cap1100, std-only forbid(unsafe_code) <120L.
tests: 6 in-file (empty-reject, set, clear+label, label-ok, label-missing, trunc caps).
decisions: chars().take for unicode-safe trunc; label format `loc <path> ok|missing`.
verify: `rustfmt --check` FMT_OK, 119 lines. No cargo/commit per scope.
unknowns: none.
