# BRIDGE-PAR-332 scratchpad

Claim: attempted via completion_claims.claim -> ClaimError "claims must be a bounded mapping" (ledger missing schemaVersion=1; pre-existing repo-wide drift, out of lane authority). Proceeded file-only per lane scope; no ledger write possible. Session ses_par332.

Source evidence:
- TS truth /home/rashid/projects/opencode/packages/tui/src/component/register-spinner.ts:1-6 (registerOpencodeSpinner guards catalogue.spinner, calls registerSpinner once)
- Sibling crates/opentui-bridge/src/spinner_full.rs (braille FRAMES, Spinner::tick/frame_of) - idempotent-register analog

Target boundary: ONE new file crates/opentui-bridge/src/spinner_register_full.rs only. No lib.rs/Cargo.toml/cargo/commit.

Tests: 3 in-file (register_and_has, duplicate_rejected, bounds_enforced).

Decisions: idempotent register returns false on dup (mirrors TS guard); empty/overlong/cap reject false; Vec<String> std-only, forbid(unsafe_code), 68 lines <80.

Verification: rustfmt --check PASS (FMT_OK).

Unknowns: ledger claim blocked by pre-existing claims.json shape; orchestrator must claim/reconcile.
