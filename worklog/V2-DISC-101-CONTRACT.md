# V2-DISC-101 contract repair

## Candidate metadata

- Package/gate: DISC-101 / G1 Startup discovery-auth contract
- Base: `f8bde4923cb0f6fdf9c86cbb9341d82d50a89209`
- Candidate: recorded after the parent commits this candidate
- Owned paths: `crates/server/tests/discovery_auth_red.rs`, this worklog
- No Cargo, product, frozen CLI, or e2e fixture changes were made.

## Frozen contract

The evaluator receipt (`discovery-contract-evaluation.json`, source base
`0bf0274ad863a7a60fd6631042871ec3c8016256`) supersedes the three historical
assertion mistakes without weakening their security intent:

1. A stale descriptor is `None`, and its token receives `403`; the native
   caller must additionally prove no origin contact, no provider/store effect,
   and no duplicate lifecycle action.
2. A live foreign PID is valid at the low-level liveness/parser boundary. It
   must not be trusted at the real endpoint handshake unless authenticated
   serialized registration identity (including PID/origin) matches. This is not
   a current-process PID comparison.
3. A live schema-valid empty-token descriptor returns the exact loud error:
   `descriptor predates bearer auth (empty auth_token); refusing to attach`.
   Client discovery remains unusable, with no credential, contact, or effects.

Every spawned child is owned by an unconditional RAII guard which kills and
waits before assertion completion or unwind.

## Source evidence

- `crates/server/src/daemon.rs:166-206`: schema, PID liveness, loopback and
  token validation; empty token is an exact `DaemonError::Descriptor`.
- `crates/server/src/lib.rs:134-184,212-214`: `/api/sessions` is protected by
  bearer middleware while `/health` is public and currently emits schema/status
  only.
- `crates/cli/src/chat.rs:479-518`: health probing and descriptor-origin
  matching; published PID is not a same-process ownership proof.
- `crates/cli/src/daemon_client.rs:647-729`: credentialed discovery is a
  file/parser gate and explicitly promises no origin contact on refusal.
- Pinned upstream `95daf906...`, `packages/cli/src/services/daemon.ts`:
  `healthy` authenticates health and `sameRegistration` gates ownership.

## Test status and remaining product gap

This worker did not run Cargo, PTY, native, container, network, or heavy gates.
The repaired test is intentionally a compiling RED for the missing native
caller identity refusal: it publishes a forged live PID, contacts the actual
endpoint with its bearer, and requires `/api/*` denial before trust. It does
not pretend that a public `/health` PID field is authentication. The current
server accepts the credential because no serialized registration identity
handshake exists; this is the genuine product gap.
The parent must run the focused server test and native G1 gate with the supplied
`OC2_TEST_BINARY`/`OC2_TEST_BUILD_JSON` artifact, then freeze the resulting
hash and command manifest. This candidate is **PREVERIFIED only after parent
compilation/RED confirmation**, never ACCEPTED; acceptance requires the exact
integrated SHA.

## Integrity freeze plan

Parent: AST-parse the test, inspect `git diff --check`, record old assertion
blob/hash and new blob/hash, compile and capture RED/GREEN, then integrate only
these granted paths. Preserve the evaluator receipt and old assertion history.
The t01/t03 server checks remain lower-level executable evidence only; their
native no-contact/no-side-effect composite gate is not falsely claimed here.
Closing that caller contract needs the existing CLI native flow fixture or,
if its controls cannot be reused from this server target, the separately
granted `tests/e2e/discovery_identity.py` using `OC2_TEST_BINARY` and
`OC2_TEST_BUILD_JSON`.
Next gates: focused `discovery_auth_red`, `native_daemon_flow`, installed
default entrypoint, and the complete fresh-HOME G1 startup journey.
