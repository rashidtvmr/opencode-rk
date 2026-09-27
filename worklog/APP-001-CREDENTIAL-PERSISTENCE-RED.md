# APP-001-CREDENTIAL-PERSISTENCE-RED — worklog

## Claim

- Task: `APP-001-CREDENTIAL-PERSISTENCE-RED` (vertical TEST-AUTHOR lane).
- Session: `ses_f1ea8c87bffetFz0ztkES0ZnxZ`.
- Route: `@xkiro-dsv41-flash-free` (allowlisted temporary pool).
- Owned file: `crates/cli/tests/installed_setup_persistence.rs` (new, one file).
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/red-app001-credential-persistence-w2`
- Branch: `red/APP-001-CREDENTIAL-PERSISTENCE-W2`, base `ecec045`.
- Local ledger claim: CLAIM OK via `tools/completion_claims.py` (`claim`).

## Source evidence (base `ecec045`, 2026-09-26)

- `crates/cli/src/daemon_client.rs:775-791` — `creds_configured(_data_dir)` reads only
  provider env vars (`OPENAI_API_KEY` etc.); it never consults a persisted credential
  store. There is no on-disk credential read path.
- `crates/cli/src/onboarding.rs:348-398` — `AccountStore` / `MemoryAccountStore`: the
  only store is in-memory and holds a `bool` ("committed"), never credential bytes and
  never a file.
- `crates/cli/src/onboarding.rs:157-199` — `SecretString` redacts `Debug`/`Display`,
  but nothing writes the secret to durable storage.
- `crates/cli/src/app_start.rs:287-346` — `plan_default_launch` / `needs_setup` route
  missing creds to `StartupView::Setup`, but the setup outcome is never committed to a
  durable store a later launch can read.
- `grep 0600|set_permissions|PermissionsExt|mode(0o` over `crates/cli/src` → **no
  matches**: no owner-only file mode is ever set.
- `tasks/completion/local.json:4` — APP-001 journey text and five acceptance bullets
  that this lane's contract extends to credential persistence.

Conclusion: credential persistence + read-back + `0600` does not exist. The RED suite
compiles with std only and fails for the missing behavior (not for imports/fixtures).

## Observable contract

1. Fresh disposable HOME/data, no provider env var: no-subcommand launch runs in-app
   setup, accepts provider + fake credential + model, exits 0, and writes durable
   credential state.
2. Credential state is owner-only (`0600` on Unix; no group/other bits).
3. Restart with same HOME/data skips setup and can construct/send exactly one
   fake-provider request using the stored credential.
4. Raw credential never in stdout, stderr, argv, or URLs/state files.
5. Cancel (EOF before model step) leaves no residue: no credential file, no staged
   account.
6. Launch reaps any owned child/daemon on exit; no staged descriptor left.

## Tests written (frozen RED)

File `crates/cli/tests/installed_setup_persistence.rs`, std-only:
- T01 `fresh_setup_accepts_provider_credential_model_and_exits_clean` (RED)
- T02 `credential_state_is_owner_only_0600_on_unix` (RED)
- T03 `restart_skips_setup_and_uses_stored_credential_for_one_request` (RED)
- T04 `key_absent_from_stdout_stderr_argv_and_urls` (guard; green today)
- T05 `cancellation_leaves_no_residue` (guard; green today)
- T06 `owned_child_daemon_is_reaped_on_exit` (RED)

Disposable `DisposableHome` (temp dir removed on drop), provider env scrubbed,
`ChildGuard` kills+waits owned child, bounded `try_wait` polling (std has no
wait_timeout). No real account file, no network, no user DB.

## Decisions

- Tests assert on the *absence of any owner-only file* under the data dir, so they do
  not depend on the implementation choosing a particular path. A documented candidate
  path (`data/credentials/provider-credentials.json`) is referenced in messages only.
- T04/T05 are safety guards that may already pass; the suite as a whole is RED via
  T01/T02/T03/T06.
- No product code edited. No frozen test edited.

## Remaining unknowns / blocker

- Real PTY interaction is unavailable in this lane; the suite drives the binary with
  piped stdin and bounded waits instead, which is enough to observe persistence,
  mode bits, redaction, and residue.
- Status after genuine RED: `blocked` (RED-only lane; implementation is a separate
  task).