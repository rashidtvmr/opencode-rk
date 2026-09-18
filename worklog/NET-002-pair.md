# NET-002-pair worklog

Task: NET-002 pairing types. Owned file only: `crates/cli/src/pair.rs`.

## Claim
Pairing types created: `SingleUseChallenge` (opaque + expiry + account binding),
`DeviceFingerprint` display holder, QR payload builder (challenge material only),
cancel-clears-pending receipt. `#![forbid(unsafe_code)]`, std only, no crypto invented.

## Source evidence (commit 5af7884)
- docs/architecture/COMPLETION_REMOTE.md:28-37 pairing section: short-lived
  single-use account-bound challenge, visible device fingerprint, explicit local
  approval. QR must not contain provider secrets or long-lived tokens. Never
  roll ad-hoc cryptography.
- NET-002 card via `python3 tools/completion_plan.py --card NET-002`: tests
  T01..T05 (correct account/device, expired/reused/other-account fail,
  QR challenge-only, one enrollment on concurrent redemption, cancel clears).
- crates/cli/src/main.rs:19,21 `mod tui_entry; mod chat;` pattern (new module
  not yet wired; orchestrator wires `mod pair;` to avoid lane races).
- crates/cli/src/pair.rs did not exist before this lane (fresh file).

## Observed scenario
RED first: file written with stub impl + 8 compiling tests. Ran via mandated
command shape: 0 passed, 8 failed (see /tmp/opencode/red.log). Failure modes
matched missing behavior (accept-anything redeem, empty QR, non-clearing
cancel, leaking Debug).

## Target boundary
- This lane: owned file `crates/cli/src/pair.rs` ONLY. No `main.rs` edit, no
  other files.
- Integration seams left to orchestrator: add `mod pair;` to main.rs,
  gateway-issued opaque bytes, clock source, re-enrollment consent gate live in
  other slices.

## Tests (frozen in-file, 8 tests)
- redeem_ok_binds_intended_account_and_device
- expired_challenge_fails_without_enrollment (pending preserved)
- reused_challenge_fails_second_redeem (AlreadyUsed)
- other_account_fails_without_enrollment (pending preserved)
- wrong_opaque_fails (constant-time compare, pending preserved)
- qr_payload_has_challenge_only_no_provider_secret
- cancel_clears_pending_and_blocks_redeem (receipt.cleared_pending, redeem->Cancelled)
- debug_redacts_opaque_challenge_material

## Decisions
- `SingleUseChallenge` has no provider-key/long-lived-token field by
  construction; `qr_payload` takes `&SingleUseChallenge` so secrets cannot
  reach QR.
- Manual `Debug` impl redacts opaque + account_id; fingerprint stays visible
  by design (operator compares on both screens).
- `Pairing` state machine Pending/Consumed/Cancelled; redeem zeroes opaque
  after success; cancel zeroes + receipt. No `unsafe` (forbid); zeroize via
  overwrite-then-clear to respect forbid(unsafe_code).
- Bounds: MAX_OPAQUE_LEN 256, MAX_ACCOUNT_LEN 128, MAX_FINGERPRINT_LEN 64.
- Check order in redeem: state -> account -> expiry -> opaque. Wrong-account
  and expired never touch opaque state; failed attempts keep Pending.
- `ponytail:` concurrent-redemption atomicity (this type is single-slot,
  in-process; cross-process one-enrollment needs gateway-side atomic consume
  in control-plane pairing slice). Brute-force rate limiting likewise belongs
  gateway-side, not this type.

## Remaining unknowns
- Exact gateway challenge-issue/consume RPC shape (sibling slice owns it).
- QR transport rendering (TUI/terminal QR lib) out of scope for this type file.
