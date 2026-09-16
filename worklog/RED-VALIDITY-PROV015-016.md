# RED-VALIDITY-PROV015-016 — stub-bite re-verification (PROV-015/016)

Base revision: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b` (impl predates lane).
Method: temp behavior-stub impl (signatures intact; no test/lib.rs/ralph.json touch),
run frozen suite expecting compile+fail, restore byte-identical (`cmp` + sha256 vs
pre-run hash in `/tmp/opencode/redbak/`), re-run GREEN 5/5.
Bounds: serial, one heavy cmd at a time, `--test-threads=2`, `timeout 115`, `rtk` prefix.
`free -h` at start: 6.2G total, ~2.7G avail; during GREEN: ~3.1G avail. No OOM pressure.

Prior claim: `worklog/RED-VALIDITY-WEB-PROV.md` reports PROV-015 (`profile_of`→`Err(EmptyProvider)`,
0/5 fail) and PROV-016 (`begin_login`→`LoggedOut`, 0/5 fail) as valid RED. This lane re-runs
those two stubs plus two more strategies per file (wrong state, capped overflow).
Result: A-stubs all bite; B1/B2 bite; **three B3-class boundary stubs do NOT bite**
(detals below). Prior worklog's verdict stands for the entry-point stubs only; it missed
the non-pinning boundary validators.

## Impl hashes (pre == post, `cmp` restore verified)

- `crates/providers/src/auth_profile.rs` `109e666c81f2e4cb4679c0a912d99d3635b0b51736ea72512a7ce726aa0def32`
- `crates/providers/src/codex_oauth.rs` `e16ef54882ebbe3e4a5578f6ddaa5a05877fbd96420b829997763a3290806af8`

## Frozen test hashes (untouched; `git diff --stat` clean on all four + lib.rs + ralph.json)

- `crates/providers/tests/auth_profile.rs` `a23f9f322c959260d88fd0cbf033ce605b064ae8add93b843440a0bbd586756e`
- `crates/providers/tests/codex_oauth.rs` `aa7459f644f45eae1815fe9c5dfc67fcee45f7ff9df5db8d2aa4369514098851`

Note: `git status` shows ~40 modified + 10 untracked files under `crates/providers/` from
sibling lanes (e.g. `claude_oauth.rs`, `int_*`, `tests/prov_017_*`); none are owned by this
lane and none were touched here. The 4 owned files + `src/lib.rs` are byte-identical.

## PROV-015 (`auth_profile.rs`) — VALID RED, all 3 strategies bite

| stub (compiles, behavior-only) | RED log | result |
|---|---|---|
| A1 forced `Err`: `profile_of` returns `Err(EmptyProvider)` first (`:194`) | `/tmp/opencode/uD-PROV015-016-a1.log` | `FAILED. 0 passed; 5 failed` — T01 `:16`, T02 `:43`, T03 `:60`, T04 `:93`, T05 `:119` panic |
| A2 wrong state: `describe` emits `auth_mode:"bogus-mode"`, `has_credentials:false` | `/tmp/opencode/uD-PROV015-016-a2.log` | `FAILED. 4 passed; 1 failed` — T01 fails on `auth_mode`/`has_credentials` asserts (`tests/auth_profile.rs:23-25`); T02/T04/T05 pass correctly (provenance/validation/determinism paths don't read `describe` mode) |
| A3 capped overflow: `TooLong` gate moved to `MAX+4096` (`:197`) | `/tmp/opencode/uD-PROV015-016-a3.log` | `FAILED. 4 passed; 1 failed` — T04 fails (129-byte id accepted); T01/T02/T03/T05 pass correctly |

Verdict PROV-015: **pins** — constructor guards (empty/unknown/too-long, T04 `:88-109`),
projection (T01), provenance distinctness (T02), redaction (T03), determinism (T05) all bite.
GREEN after restore: `/tmp/opencode/uD-PROV015-016-green-auth.log`, `ok. 5 passed; 0 failed`.

## PROV-016 (`codex_oauth.rs`) — PARTIAL: lifecycle pins, boundary validators DO NOT

| stub (compiles, behavior-only) | RED log | result |
|---|---|---|
| B1 forced `LoggedOut`: `begin_login` returns `LoggedOut` (`:302`) | `/tmp/opencode/uD-PROV015-016-b1.log` | `FAILED. 0 passed; 5 failed` — T01 `:23`, T02 `:41`, T03 `:70`, T04 `:86`, T05 `:115` panic |
| B2 wrong state: `status` emits `provider:"wrong-provider"`, `auth_mode:"wrong-mode"` (`:430-433`) | `/tmp/opencode/uD-PROV015-016-b2.log` | `FAILED. 4 passed; 1 failed` — T01 fails on provider/auth_mode asserts (`tests/codex_oauth.rs:31-32`); rest pass correctly |
| B3 nocap device-code: `validate_device_code` gutted to `Ok(())` (`:502`) | `/tmp/opencode/uD-PROV015-016-b3.log` | **`ok. 5 passed; 0 failed` — NO BITE** |
| B3alt nocap consent-URL: length check removed, scheme check kept (`:495`) | `/tmp/opencode/uD-PROV015-016-b3alt.log` | **`ok. 5 passed; 0 failed` — NO BITE** |
| B3exp zero-expiry accepted: `==0 → InvalidExpiry` guards removed in `complete_login` + `refresh` (`:346`, `:387`) | `/tmp/opencode/uD-PROV015-016-b3exp.log` | **`ok. 5 passed; 0 failed` — NO BITE** |

Why the B3 class survives (verified against `tests/codex_oauth.rs` source):
- No test feeds an empty/oversize device code: T03 asserts only `BadConsentUrl` for an
  `http://` URL (`:76-79`); `BadDeviceCode` is never asserted anywhere.
- No test feeds a >2048-byte consent URL: `begin_login_with` is called once, with a short URL.
- No test uses `expires_at_ms == 0`: `EXPIRY_MS`/`LATER_MS` constants only; `InvalidExpiry`
  is never asserted.

Verdict PROV-016: **non-pinning for boundary validation**. Lifecycle (begin/complete/refresh/
logout/status-shape) pins via B1/B2. But `validate_device_code` (empty + `>MAX_DEVICE_CODE_BYTES`),
`validate_consent_url` length cap (`>MAX_CONSENT_URL_BYTES`), and zero-expiry rejection are
**dead enforcement from the test suite's view** — impl enforces, tests never exercise, so a
regression silently widening those gates stays GREEN.
GREEN after restore: `/tmp/opencode/uD-PROV015-016-green-codex.log`, `ok. 5 passed; 0 failed`.

## Additive proposal (for verifier/controller; frozen tests NOT edited by this lane)

Add to `crates/providers/tests/codex_oauth.rs` (or a new frozen target), all against real impl,
no network/fs outside disposable dir:

1. `prov_016_t06_device_code_bounds`: `begin_login_with(CODEX_CONSENT_URL, "")` → `BadDeviceCode`;
   129-byte device code → `BadDeviceCode`; `HumanGrant::from_device_code("", E, true)` →
   `BadDeviceCode`; `HumanGrant::for_device(&"d".repeat(129), E)` → `BadDeviceCode`.
   Kills B3 stub.
2. `prov_016_t07_consent_url_length_cap`: `"https://x/"` + pad to 2049 bytes →
   `begin_login_with` → `BadConsentUrl`; 2048-byte URL stays `Ok`. Kills B3alt stub.
3. `prov_016_t08_zero_expiry_rejected`: `complete_login(&pending, HumanGrant::for_test(0))` →
   `InvalidExpiry`; `refresh(&ready, RefreshGrant::for_test(0))` → `InvalidExpiry`.
   Kills B3exp stub.

Each proposed test must first be run RED against its corresponding stub above (expect fail),
then GREEN on restored impl, hashes frozen.

## Evidence paths

- RED logs: `/tmp/opencode/uD-PROV015-016-a1.log`, `-a2.log`, `-a3.log`, `-b1.log`, `-b2.log`,
  `-b3.log`, `-b3alt.log`, `-b3exp.log`
- GREEN logs: `/tmp/opencode/uD-PROV015-016-green-auth.log`, `/tmp/opencode/uD-PROV015-016-green-codex.log`
- Restore backups: `/tmp/opencode/redbak/auth_profile.rs.bak`, `/tmp/opencode/redbak/codex_oauth.rs.bak`
- Impl evidence: `auth_profile.rs:189-209` (guards), `codex_oauth.rs:495-507` (validators),
  `codex_oauth.rs:346/387` (zero-expiry guards)
