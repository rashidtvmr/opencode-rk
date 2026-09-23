# PROV-023 loader RED

## Claim
- Task: `PROV-023`
- Session: `ses_f31706c24ffeCpyRA4Srbaiivb`
- Scratchpad: `worklog/PROV-023-LOADER-RED.md`

## Source evidence
- `crates/providers/src/request_profile.rs:162-206` -- `profile_for`, `profile_for_with_options` resolve via `documented_entry` which is hardcoded for `openai` and `anthropic` only (lines 302-350). No env var, no catalog file read.
- `crates/providers/src/request_profile.rs:95-109` -- `RequestError` has `UnknownProvider`, `UndocumentedEndpoint`, `HeaderNotAllowed`, `BadEndpoint`, `BadBounds`. Cannot distinguish malformed catalog (no such variant), so assert only fail-closed Err.
- `crates/providers/tests/prov_019_request_profile.rs` -- frozen tests for `profile_for` happy path (openai, anthropic); currently GREEN because `documented_entry` hardcodes these.
- `crates/providers/tests/prov_023_provider_catalog.rs` -- validates the static JSON catalog file, but does NOT exercise `profile_for` against it.
- `docs/provider-compatibility.json` -- versioned catalog v1 containing openai, anthropic, google; Google endpoint has `{model=models/*}` placeholder in `url_template`.

## Environment fixture convention
- Required env name: `OPENCODE_RK_PROVIDER_CATALOG_PATH` (determines which compatibility catalog the provider request profile loader consults).
- Tests set this env var to a disposable tempfile path, use a global mutex + RAII guard to restore prior value, and assert fail-closed behavior.

## Target boundary
- Through the existing public `request_profile::profile_for` boundary.
- Tests prove that `profile_for` consults `OPENCODE_RK_PROVIDER_CATALOG_PATH`:
  - valid v1 catalog with google entry drives Google resolution (currently RED: returns UnknownProvider).
  - missing path, malformed JSON, unsupported version, over-256KiB bytes, duplicate provider/kind, non-HTTPS url_template, too many providers/endpoints/auth headers, placeholder syntax outside allowed `{model=...}` all fail closed (Err).
  - unknown provider/endpoint remain typed (UnknownProvider / UndocumentedEndpoint), no endpoint guessed.
  - no secret values leak into profile/debug/errors.
- Do NOT edit product code or existing/frozen tests.

## Tests
- One new test file: `crates/providers/tests/prov_023_loader_red.rs`.
- All tests use disposable tempfiles, no network, no source scan.

## Decisions
- If `RequestError` cannot distinguish malformed catalog, assert only `is_err()` (fail-closed Err), never inspecting an invented variant.
- `profile_for` signature unchanged; tests set env var before calling.
- Placeholder model syntax: the allowed documented model placeholder is `{model=models/*}` style; disallowed placeholders in `url_template` must fail.

## RED execution
- Command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 python3 - <<'PY' ... subprocess.run(['cargo', 'test', '-p', 'opencode-rk-providers', '--test', 'prov_023_loader_red', '--', '--test-threads=1'], timeout=180) ... PY`
- Result: compiling RED; `21` executed, `3` passed, `18` failed, `0` ignored; no construction panic; exit `101`.
- Passed GREEN guards: `prov_023_loader_green_debug_contains_no_secret_bytes`, `prov_023_loader_green_unknown_endpoint_fails_closed_without_guessing`, `prov_023_loader_green_unknown_provider_fails_closed_without_guessing`.
- Failed RED: `prov_023_loader_red_auth_header_cap_fails_closed`, `prov_023_loader_red_credential_bearing_url_fails_closed`, `prov_023_loader_red_disallowed_placeholder_fails_closed`, `prov_023_loader_red_duplicate_endpoint_kind_fails_closed`, `prov_023_loader_red_duplicate_provider_fails_closed`, `prov_023_loader_red_endpoint_cap_fails_closed`, `prov_023_loader_red_malformed_json_fails_closed`, `prov_023_loader_red_missing_configured_file_fails_closed`, `prov_023_loader_red_missing_version_fails_closed`, `prov_023_loader_red_model_alias_cap_fails_closed`, `prov_023_loader_red_non_https_url_fails_closed`, `prov_023_loader_red_oversize_fails_closed`, `prov_023_loader_red_provider_cap_fails_closed`, `prov_023_loader_red_secret_like_fixture_fails_closed`, `prov_023_loader_red_unset_and_empty_use_embedded_google`, `prov_023_loader_red_unsupported_version_fails_closed`, `prov_023_loader_red_valid_json_wrong_shape_fails_closed`, `prov_023_loader_red_valid_override_drives_google`.
- Freeze command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 python3 - <<'PY' ... PY` as above.
- Freeze SHA-256: `d6aa3a01ad4e6a4d09c148672d29dbc608c739e5bbf7cb574f8b9ad16becc2d8` (`shasum -a 256 crates/providers/tests/prov_023_loader_red.rs`).

## Status
- blocked awaiting implementation of catalog-backed loader in `request_profile`.
