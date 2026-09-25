# PATH-HARDLINK-RED scratchpad

Claim: PATH-HARDLINK-RED, session ses_f3c4de578ffelQv59xDXmOs03B.

Base: `7262682e31c1f473912c9483804c9430eb4ee4c5` on
`red/APP012-PROTECTED-HARDLINK`.

Source evidence:

- Current code, `crates/security/src/lib.rs:206-289`, authorizes file reads from
  a lexical path and checks secret/system markers but never inspects file
  identity or link count. Wildcard permissions cannot override an existing
  baseline deny, but a safe-looking alias currently reaches baseline allow.
- Existing verified contract,
  `worklog/APP012-PROTECTED-PATH-CONTRACT.md:115-141`, requires conservative
  denial of a multi-link regular file unless a platform handle check proves it
  non-protected; canonicalization alone cannot identify hardlink aliases.
- Independent prior verification,
  `worklog/APP012-PROTECTED-PATH-VERIFY.md:11-23`, explicitly leaves hardlink
  identity open because canonicalization sees the alias path, not the shared
  file identity.
- New synthesis stage `PATH-HARDLINK-RED` at commit `7262682` assigns
  `crates/security/tests/app012_protected_hardlink_red.rs`, followed by an
  implementation in `crates/security/src/lib.rs` after freeze/integration gates.

Observable contract: a normal one-link regular file inside the disposable
workspace remains readable. A hardlink inside that workspace to a disposable
protected `.env` fixture is a mandatory `Decision::Deny`, even under
`PermissionSet::star()`, and creates exactly one bounded audit record. The test
does not read host secrets, file contents through the broker, or mutate anything
outside its PID-scoped temporary directory.

Target boundary: RED only. No product, manifest, library, existing-test, or
verifier changes. This proves broker-level identity classification; descriptor-
bound open/read TOCTOU remains the dependent `PATH-OPEN-RED` stage.

## RED receipt

- Focused command, run twice:
  `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p
  opencode-rk-security --test app012_protected_hardlink_red --
  --test-threads=1`.
- Both runs compiled and produced the same behavioral result: 1 passed, 1
  failed, 0 ignored. The single-link workspace control passed. The protected
  hardlink alias was allowed under wildcard permission instead of receiving a
  mandatory denial.
- Frozen SHA-256:
  `71bf5f632a44a3602fe7f188a16886ad04ea87b2da7d62da8506a79260413903`.
- After freezing: hash unchanged, `git diff --check` passed, and no Cargo,
  rustc, or focused-test process survived.

Remaining: `I2-PROTECTED-BASE` and `V1-FREEZE-RED` must authorize the planned
implementation stage. Descriptor-bound open/read remains separately owned by
`PATH-OPEN-RED`. No implementation or acceptance is authorized here.
