# APP-005 worklog (onboarding.rs lane)

## Claim
- First-run provider setup types in owned file only: `crates/cli/src/onboarding.rs`.

## Source evidence
- Commit `5af7884cf7637c0760d985da7a03f2f99ccd0c78`.
- Card `APP-005` via `python3 tools/completion_plan.py --card APP-005`: journey
  first-run provider/model setup without hand-edited config; T01-T05 map to
  authorized-account turn, cancel-clean PKCE/credential, retry actions, secret
  redaction, offline cached path.
- `docs/TDD.md` / `docs/SECURITY.md` binding: compiling RED failing for missing
  behavior; secrets never logged; cancel asserts absence of side effects.
- No prior `crates/cli/src/onboarding.rs` (verified absent before creation).

## Observed scenario
- RED run (`rustc --edition 2021 --test crates/cli/src/onboarding.rs -o
  /tmp/opencode/ob && /tmp/opencode/ob`): 3 passed, 5 failed —
  `cancel_leaves_no_half_account`, `invalid_credential_surfaces_retry`,
  `offline_cached_settings_path`, `pkce_expiry_rejected`, `secret_never_logged`.
- GREEN run, same command: `test result: ok. 8 passed; 0 failed`.
- RED causes (planted failing impl): secret Debug/Display leaked bytes,
  `is_expired` always false, `remove_account` no-op, credential accept-all,
  `start_offline` always error. Each fixed in owned file only.

## Target boundary
- Owned file: `crates/cli/src/onboarding.rs` (872 lines, sha256
  `ce943358e9a3d330859419f108791ce4ab5cd06555e0175b934b0c4fd460d482`).
- No other edits: no mod wiring, no Cargo edits, no provider crate edits.
- `git status --short -- crates/cli/src/onboarding.rs` shows `??` (untracked);
  no diff to stat.

## Tests
- 8 `#[cfg(test)]` tests in-file: `setup_step_order`,
  `cancel_leaves_no_half_account`, `cancel_before_provider_selection_leaves_clean`,
  `complete_flow_commits_account`, `invalid_credential_surfaces_retry`,
  `offline_cached_settings_path`, `secret_never_logged`, `pkce_expiry_rejected`.

## Decisions
- `#![forbid(unsafe_code)]`, std only, no new deps.
- PKCE holder stores opaque string + expiry only; crypto invented nowhere.
- `SetupError` carries no secret bytes; Debug/Display of `SecretString` and
  `PkceChallenge` emit `[REDACTED]`.
- Cancel consumes session; receipt `left_no_half_account` asserts store state.
- Bounded inputs: `MAX_PROVIDER_ID_LEN` 64, `MAX_MODEL_ID_LEN` 128,
  `MAX_SECRET_LEN` 8192, PKCE opaque 512 / state 256.
- Credential fixture rule documented on `submit_credential`: `sk-` prefix, no
  whitespace/control, 8..=8192 bytes.
- `Off` features / secure-storage wiring left to integrator (one-file lane).

## Remaining unknowns
- Integrator must wire `mod onboarding` and map `RetryAction` to real TUI/CLI
  screens; provider-side account authorization lives in sibling slice
  `crates/providers/src/account_setup.rs` (not this lane).
