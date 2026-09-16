# PROV-016 freeze proposal (additive bounds suites)

Base rev: `248f519d53ed29cbf7fef7ceb2b5010fb1a4224b`.
Lane: verify-only. Zero edits to impl/frozen/lib.rs/ralph.json.

## Exact file hashes (sha256)

- `crates/providers/src/codex_oauth.rs` `e16ef54882ebbe3e4a5578f6ddaa5a05877fbd96420b829997763a3290806af8` (tracked, `git diff` clean)
- `crates/providers/tests/codex_oauth.rs` `aa7459f644f45eae1815fe9c5dfc67fcee45f7ff9df5db8d2aa4369514098851` (frozen T01-T05, untracked, untouched)
- `crates/providers/tests/codex_oauth_bounds.rs` `9d2c8d873731cdbf062c2531631de81d5790fc93482d324d4e3fc4bbef354e28` (additive, untracked)
- `crates/providers/tests/codex_oauth_bounds2.rs` `3595ed6c1c9674885932a979191f4c05aa1c278f4b058747f946a62f582986fc` (additive, untracked)
- GREEN log `/tmp/opencode/bE-prov16.log` `f0dc4d3f9af4a55621325baaea174b5d8163ba6b4b06d186e5b5a2fe7d205ad1`

## Test IDs

- Frozen `codex_oauth`: T01 `prov_016_t01_consent_happy_path`, T02 `prov_016_t02_refresh_and_logout`, T03 `prov_016_t03_consent_required_and_logged_out_refresh`, T04 `prov_016_t04_redaction`, T05 `prov_016_t05_determinism_and_isolation` — 5/5 GREEN.
- Additive `codex_oauth_bounds`: B06 `prov_016_b06_empty_device_code_rejected`, B07 `prov_016_b07_oversize_device_code_rejected`, B08 `prov_016_b08_grant_constructor_device_code_bounds`, B09 `prov_016_b09_consent_url_length_cap`, B10 `prov_016_b10_zero_expiry_rejected`, B11 `prov_016_b11_stale_expiry_rejected_at_boundary` — 6/6 GREEN.
- Additive `codex_oauth_bounds2`: C12 `prov_016_c12_empty_device_code_rejected`, C13 `prov_016_c13_oversize_device_code_rejected`, C14 `prov_016_c14_consent_url_length_cap`, C15 `prov_016_c15_zero_expiry_rejected`, C16 `prov_016_c16_stale_expiry_rejected_on_complete`, C17 `prov_016_c17_stale_expiry_rejected_on_refresh` — 6/6 GREEN.
- Total: 17/17. Kills B3/B3alt/B3exp stub classes (`worklog/RED-VALIDITY-PROV015-016.md`).

## Verifier-rerun commands (serial, JOBS=1 THREADS=1, timeout 120)

```
free -h
cargo test -p opencode-rk-providers --test codex_oauth -- --test-threads=1
cargo test -p opencode-rk-providers --test codex_oauth_bounds -- --test-threads=1
cargo test -p opencode-rk-providers --test codex_oauth_bounds2 -- --test-threads=1
sha256sum crates/providers/src/codex_oauth.rs crates/providers/tests/codex_oauth.rs crates/providers/tests/codex_oauth_bounds.rs crates/providers/tests/codex_oauth_bounds2.rs
git -C . status --short -- crates/providers/src/codex_oauth.rs crates/providers/tests/codex_oauth.rs crates/providers/tests/codex_oauth_bounds.rs crates/providers/tests/codex_oauth_bounds2.rs crates/providers/src/lib.rs ralph.json
```

Expected: 5 + 6 + 6 passed, hashes match above, lib.rs/ralph.json clean.
GREEN evidence: `/tmp/opencode/bE-prov16.log` (this lane, rev 248f519).

## Waiver request (to test-owner / controller)

This lane cannot freeze tests (ADR-007: implementer cannot accept own story; test source owned by trusted controller). Request:

1. Freeze `crates/providers/tests/codex_oauth_bounds.rs` (B06-B11) and `crates/providers/tests/codex_oauth_bounds2.rs` (C12-C17) at the hashes above as additive frozen suites alongside `tests/codex_oauth.rs` (T01-T05).
2. Record all three suites in the verifier manifest with the rerun commands above.
3. Prior mutation controls (disposable /tmp copies, repo tests never mutated): bounds flipped B10 assert → 5 passed/1 failed (`/tmp/opencode/v2-mut-control.log`); bounds2 flipped C15 assert → 5 passed/1 failed (`/tmp/opencode/aC-bounds2-mut.log`). Re-run at freeze time if policy requires.
4. `src/lib.rs` and `ralph.json` untouched (verified clean); no integration action needed from this lane.
