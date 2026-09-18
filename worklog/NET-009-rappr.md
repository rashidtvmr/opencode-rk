# NET-009-rappr — remote approval types

Claim: standalone `crates/server/src/remote_approvals.rs` implements NET-009 decision binding.
Commit: 5af7884. Owned file only, no other edits. std only, `#![forbid(unsafe_code)]`.

Source evidence: NET-009 card via `python3 tools/completion_plan.py --card NET-009`;
docs/TDD.md, docs/SECURITY.md read. No in-repo remote-approval impl found
(`crates/server/src/` has remote_ledger/sync but no approvals module).

Observed: `rustc --edition 2021 --test crates/server/src/remote_approvals.rs -o /tmp/opencode/ra && /tmp/opencode/ra` => 5 passed, 0 failed.

Target boundary: decide() binds digest/device/workspace/requester/expiry/policy;
Mutex commit => race one-winner (loser AlreadyDecided); consume() single-use,
current-digest check, expiry/policy/revocation checks; local_only/human_only
rejected remotely; revoke_request/revoke_device/bump_policy invalidate pending;
MAX_PENDING_APPROVALS=1024, MAX_FIELD_LEN=256. No DB, no network, no secrets.

Tests (T01..T05 in-file):
- valid_approval_matches_all_fields (incl. per-field mismatch => nothing recorded, expiry)
- race_yields_one_decision (2 threads + Barrier, one execution, replay refused)
- replay_or_changed_content_invalid
- local_only_human_only_enforced_remotely
- revocation_invalidates_pending (request, device-logout-after-decide, policy bump)

Decisions: single Mutex store, minimal; revocation sets separate from requests;
policy bump => stale decide/consume give Revoked. Quota errs QuotaExceeded.

Remaining unknowns: integration wiring (lib.rs/routes) owned by integrator, not this lane.
