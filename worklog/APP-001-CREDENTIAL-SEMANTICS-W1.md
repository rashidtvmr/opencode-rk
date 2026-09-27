# APP-001-CREDENTIAL-SEMANTICS-W1 — implementation

## Claim
- Task: `APP-001-CREDENTIAL-SEMANTICS-W1`
- Session: `ses_im_w1_impl`
- Status: `completed`
- Scratchpad: `worklog/APP-001-CREDENTIAL-SEMANTICS-W1.md`

## Context
- RED frozen at commit `09a590e94b0ff65b349d189adfcb0c508f9d1382` on `origin/red/APP-001-CREDENTIAL-SEMANTICS-W1`.
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/impl-app001-credential-semantics-w1`
- Branch: `lane/APP-001-CREDENTIAL-SEMANTICS-W1`
- Owned file: `crates/cli/src/onboarding.rs` ONLY.

## Source evidence
- `crates/cli/src/onboarding.rs:523-529`, `submit_credential`: validation enforced `MIN_CREDENTIAL_LEN`, `MAX_SECRET_LEN`, `raw.starts_with("sk-")`, and no control/whitespace chars.
- Frozen tests (verified hashes before and after edit):
  - `secret_never_logged` SHA-256: `03d1623ed0500cbc2e5db830fbc74ab43efda132aee616047536940ef34d7ffe`
  - `credential_semantics_accepts_non_sk_credential` SHA-256: `9b1225c2c7a9ddf7281b8d2005234926bf76a8339399f4c4fa8ca3a41d2bdd6c`

## Implementation
- Removed `&& raw.starts_with("sk-")` from the `ok` predicate in `submit_credential`.
- Updated adjacent comment to describe bounded non-whitespace semantics.
- Preserved: `MIN_CREDENTIAL_LEN`, `MAX_SECRET_LEN`, whitespace/control rejection, state transitions, no secret logging, error semantics.

## Verification
- Standalone: `rustc --edition 2021 --test crates/cli/src/onboarding.rs -o /tmp/onboarding-cred-tests && /tmp/onboarding-cred-tests --test-threads=1` -> 14 passed, 0 failed.
- Frozen test block hashes verified matching (extraction by exact RED worklog method).
- `rustfmt --check` diffs at lines 136, 383, 934 are pre-existing; my change compiles clean.
- `git diff` shows only comment + `sk-` removal (2 insertions, 3 deletions).

## OpenTUI Cargo blocker
- Cargo target blocked by missing `libopentui.a`/`libopentui.dylib` under `crates/opentui-bridge/native/lib/aarch64-apple-darwin` (host is aarch64-apple-darwin). Not retried; standalone `rustc --test` used as approved route.

## File0600 frozen test
- SHA `c99b15bca871f1258f4ae3c886a57cc108a695476083da057caeae7cec0d3e40` referenced in task. No standalone command found in RED worklog; classified as next lane.
