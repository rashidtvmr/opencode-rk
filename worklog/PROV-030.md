# PROV-030 external candidate v4 worklog

## Status

Registered candidate imported byte-for-byte from the independently reviewed v4 source.

External source-only narrow correction. V3 remains preserved unchanged. No
repository, registry, claim, product, compile, runtime, RED, freeze, or
publication operation was performed.

- Exact base: `d542fc0c021d27bf6985576f79972b8240c4fa92`
- Pinned source: `95daf90670b7c039c436c85537da5fbfe2205b41`
- Local unpublished registration correction: `691709c46785a78a563f173a3d90a9d570c4bbfb`
- Review input: `AUTH-REQUEST-RED-V3-INDEPENDENT-REVIEW.md`, SHA-256
  `fcb59b54bd272bc3ef603d4ef81c7a6bf7e7a8c48e506f2f391dd7da2ba3f7a1`
- V3 source preserved at SHA-256
  `6a635093b43dc25513c224f4564aca1dfcd23fd528157cfa880d5ee30bb2571e`.

## Narrow v4 change

Removed the fixed marker constant and marker-based non-disclosure checks. Added:

```rust
fn assert_no_daemon_bearer(request: &[u8], bearer: &str)
```

Every captured provider request in every control now checks the actual random
per-run `Server.bearer` loaded from the daemon descriptor. Response bodies are
also checked against that actual bearer. The assertions are boolean-only and
have constant redacted diagnostics; they never format the token or raw capture.

Existing persisted-key and ambient-key redaction checks remain alongside the
actual-bearer response check.

Exact Authorization parsing and case-sensitive credential equality are unchanged.
All v3 stream, write-effect, follow-up, framing, cancellation, descriptor,
environment-isolation, and malformed-auth controls are otherwise unchanged.

## Coverage retained

- Ambient-only non-stream success.
- Ambient-only streaming broker-authorized write success.
- Persisted valid auth with malformed sibling.
- Persisted-over-ambient precedence.
- No-usable-auth zero-provider-request case.
- Persisted streaming write follow-up with actual marker effect, `call030`,
  `write success`, and terminal assistant completion.

## Remaining prerequisites

The unpublished registry correction must be published by an authenticated owner.
After publication, the v4 source can be copied to the registered test path,
compiled against the exact installed `OC2_TEST_BINARY`, and independently run.
Ambient controls must pass before persisted-auth RED is assessed. No runtime or
freeze claim is made here.


## Registered lane evidence

- Task: PROV-030; claim session: `ses_f12ac72c6fferNdqACHVVBN6qd`.
- Published registry ref: `refs/heads/integration/AUTH-RED-prerequisite` at
  `691709c46785a78a563f173a3d90a9d570c4bbfb`; corrected tree
  `7002496928d57a3203024cc5bf8c86882b10a9ce`; parent
  `d542fc0c021d27bf6985576f79972b8240c4fa92`.
- Registered source: `crates/server/tests/prov_025_request_auth.rs`.
- Import was byte-for-byte from external v4 SHA
  `11649945826b9e3e2f9b4ebff3aaa2b6d966084f8a8f881368db54c937977332`.
- No compiler, CLI, listener, database, runtime, or RED command was run.
- Required future prerequisite: exact installed `OC2_TEST_BINARY`; controls-first
  compile/run, then persisted-auth RED and independent freeze.
- This lane remains `in-progress`; no freeze or completed status.
