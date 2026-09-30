# G2 native provider-auth control contract

## Authority and scope

This is a source-only security contract for the native API route
`PUT /auth/{providerID}`.  It follows the pinned OpenCode revision
`95daf90670b7c039c436c85537da5fbfe2205b41`: the upstream control handler
accepts `Auth.Info`, calls `Auth.set`, normalizes trailing slashes, merges
other accounts, and persists `auth.json` with mode `0600`.  The pinned auth
types permit `oauth`, `api`, and `wellknown`; this contract exercises the
legacy-compatible API-key connection path without constraining future OAuth
payloads.

The current native router protects `/api/*` but has no public `/auth/{id}`
route, so the authorized test is expected to be RED against the baseline
(normally HTTP 404).  The implementation worker owns the product route and
storage; this worktree owns only the executable contract.

## Contract cases

Six tests use an installed binary and a loopback bearer descriptor.  They use
fresh disposable HOME/XDG/data/project roots, a generated offline OpenAI
catalog, bounded HTTP bodies/responses/logs, no ambient API key, and owned
process-group cleanup.  They verify:

1. missing and wrong bearer credentials return `401`/`403` and create no file;
2. authorized API auth returns success, persists the exact API schema at `0600`,
   and never echoes the fixture key;
3. trailing-slash provider IDs normalize while existing OAuth/API accounts and
   metadata remain intact;
4. invalid API types/metadata are rejected without file mutation or key echo;
5. an auth destination directory and marker are never overwritten;
6. an existing auth file over 1 MiB is rejected without truncation or mutation.

The 1 MiB file bound is an approved resource-bounded native extension; it is
explicitly not a claim that the upstream implementation has the same bound.

## Preparation evidence

Base SHA:
`710a410e0ce32c716c40217696b515caa6c52310`

Mechanical review corrections after the initial `360ca736` candidate:

- `XDG_DATA_HOME` now points at the service data root used by `auth.json` and
  the generated catalog.
- cleanup is registered immediately after service construction and is
  idempotent, including constructor/start failures; owned daemon output is
  file-backed, bounded, and optionally copied to the absolute
  `OC2_AUTH_CONTROL_ARTIFACT_ROOT` on failure/evidence collection.
- receipt validation requires an absolute executable/readable binary,
  absolute matching `.dylib`/`.so`, valid library SHA, and native release
  attestation.
- descriptor validation requires a regular `0600` descriptor no larger than
  8 KiB, the owned daemon PID/group, strict loopback origin and valid port,
  and exact OpenAI model IDs.
- absent Authorization and wrong bearer are separate cases (`401` versus
  `403`); response bodies are bounded and closed.
- trailing-slash normalization is tested using encoded `/auth/openai%2F`, not
  an unencoded route slash.
- authorized success requires the JSON boolean `true` exactly.

No controlling-TTY ioctl, product API assumption beyond the pinned wire path,
or source-hash-dependent behavior was added.

Owned paths:

- `tests/bootstrap/test_native_provider_auth_control.py`
- `worklog/V2-PROVIDER-AUTH-CONTROL-CONTRACT.md`

No runtime, Cargo, PTY, Docker, or product validation was run in this
source-only package.  Parent must run the test against the baseline to capture
the compiling RED, freeze the exact source hash/command manifest, then rerun
after product implementation.  This handoff makes no acceptance claim.
