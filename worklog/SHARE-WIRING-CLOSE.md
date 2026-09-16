# SHARE-WIRING-CLOSE

rev: 248f519
owner: SHARE-003/004/005 wiring-close lane (sole writer: crates/sessions/src/lib.rs this wave)
date: 2026-09-16

## Claim

No lib.rs edit needed. All 6 lane modules intentionally NOT wired into lib.rs;
all 6 owned tests use `#[path]` includes; `cargo check -p opencode-rk-sessions`
passes clean (only pre-existing dead-code warnings). Verdict: wired=n, edited=n.

## Source evidence

- lib.rs wiring (lines 17-28): `pub mod share; share_audit; share_count;
  share_expiry; share_invite; share_list; share_merge; share_policy;
  share_queue; share_revoke; share_scope; share_token`. Absent by design:
  share_store, share_enterprise, share_store_lane, share_enterprise_lane,
  share_policy_lane, share_policy2_lane.
- `#[path]` ownership (no wiring needed):
  - tests/share_store.rs:6 `#[path = "../src/share_store.rs"]` ("NOT wired
    into lib.rs (integrator assembles shared files)")
  - tests/share_store_lane.rs:6 `#[path = "../src/share_store_lane.rs"]` (same note)
  - tests/share_enterprise.rs:8 `#[path = "../src/share_enterprise.rs"]`
  - tests/share_enterprise_lane.rs:7 `#[path = "../src/share_enterprise_lane.rs"]`
  - tests/share_policy_lane.rs:5 `#[path = "../src/share_policy_lane.rs"]`
  - tests/share_policy2_lane.rs:6 `#[path = "../src/share_policy2_lane.rs"]`
  - Counter-case: tests/share_policy.rs uses `opencode_rk_sessions::share_policy`
    (wired module, T-different suite, not SHARE-005 lane module).
- No cross-crate consumers of unwired modules: grep `crate::share_store|
  crate::share_enterprise|share_*_lane` in src/ hits only self-doc comments;
  only `use share_*` hits are inside the `#[path]` test shims.
- cargo check: `CARGO_BUILD_JOBS=1 timeout 120 cargo check -p
  opencode-rk-sessions` -> Finished dev profile, 5 pre-existing warnings
  (types.rs dead code). Zero E0432, zero errors.
- E0432 rule: no error exists, so additive `pub mod` lines NOT triggered.
  Adding them would create duplicate-symbol risk once integrator assembles
  lane files; correctly deferred.

## GREEN: 10 suites, 50/50

cmd: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 120 cargo test
-p opencode-rk-sessions --test share_store --test share_store_lane
--test share_enterprise --test share_enterprise_lane --test share_policy
--test share_policy_lane --test share_policy2_lane --test share_queue
--test share_queue_lane --test share_merge`
log: /tmp/opencode/bA-share.log (exit=0)
counts: `grep -c "^test .* ok$" = 50`; `grep -c "test result: ok" = 10`;
each suite `5 passed; 0 failed`.
suites: share_enterprise, share_enterprise_lane, share_merge, share_policy
(wired visibility suite), share_policy2_lane, share_policy_lane, share_queue,
share_queue_lane, share_store, share_store_lane.
note: share_queue/queue_lane/merge suites included as adjacent share-lane
regression guard; frozen SHARE-003/004/005 suites (7) all green.
untouched: merge/queue sources, frozen tests, ralph.json.

## Debug redaction probe

log: /tmp/opencode/bA-probe.log
- ShareId Debug: 2-char prefix + `***` (share_store.rs:41-47).
- ShareSecret Debug: type name only, zero bytes (share_store.rs:80-84);
  struct not Clone, Drop+zeroize.
- ShareMeta Debug: session + redacted share_id + redact_url(public_url)
  (strips query/fragment), finish_non_exhaustive (share_store.rs:96-106).
- ShareStore Debug: len + redacted records only (share_store.rs:130-140).
- T04 live: share_store share004_t04_no_secret_retention ok (1 passed);
  share_store_lane share004_t04_no_secret_retention ok (1 passed).

## Hashes (sha256)

- lib.rs 3e876b979541a81128b78f4dec406cdfb68a2c542e542f921b53ff78d706a5fd
- share_store.rs 418be003787f5267e47f78856fda514d741f942c463e0ddc5917b80d6a686f41
- share_store_lane.rs 7c32b321da220e69916d4c6d0b122ff421e4a75ea2daf1bfadacad36bb895d10
- share_enterprise.rs 7f7e7ac448f61b62fb6022a9a7fed5f0acd95924139c9caa54cd5332a7ff86b4
- share_enterprise_lane.rs b1bef9e090b31af00ea1a2bc1008a0b3f78d554c107ae63a3019900de8eaa8ab
- share_policy_lane.rs 10bbb9fb8529eece8eb52241d55356e5b9cdfd5219db4f88c18263fecec544be
- share_policy2_lane.rs 0345fd4fcfd31748b19a6a27116d294770cf5578bd24cf0bdc421c96cf535eb1
- share_policy.rs (wired, untouched) bcd2246687271a15092e0ad983cb981f5dd1fc1492f2972eb8ab1d79b3b7e29d
- bA-share.log 63f7dcc2bf715ac26b8ac71bde792a1f84f6606875ed692f6adfde8cceb49a29
- bA-probe.log 06cf9595fe4b2b62a7d624da91525ef681d2c0bc0a305a2f9c2dcae1d280c539

## Remaining unknowns

None for wiring-close. Integration assembly of lane files into shared
lib.rs/Cargo.toml stays with integrator (out of scope; merge/queue untouched).
