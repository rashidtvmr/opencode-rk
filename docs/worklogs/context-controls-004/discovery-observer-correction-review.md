# DISC-101 observer correction review

Base: `e3069eee04d9a3414517b2452df803c9c145c9d3`

Before hash (`crates/server/tests/discovery_auth_red.rs`):
`e92dc84e56c678d746ff9aa760f7a2432a7e30c5`

The correction removes only the artificial T01/T03 `todo!` failures. T01's
stale-descriptor `None` and stale-bearer `403` assertions remain unchanged.
T03 now explicitly requires `DaemonError::Descriptor` for an empty token,
retains `DaemonAuth::from_published("").is_err()`, and retains unauthenticated
`/api/sessions` `401`. T02 is documented as a liveness-only reader contract
and now asserts `Some`, rather than encoding the impossible same-PID rule for
clients attaching to another daemon.

No CLI installed boundary test was added: the existing product path does not
provide a supported fixture for a controlled foreign listener plus request
capture, and adding one without a production ownership check would be a false
pass. The higher-level foreign-listener refusal remains **NEEDS_FIX / UNPROVEN**;
this is not a release claim. Required future test: `OC2_E2E_BIN` with a
disposable data dir, live foreign HTTP listener, forged valid descriptor, and
assertions for refusal before `Authorization` or any `/api/*` request while
the listener remains alive.

After hash (`crates/server/tests/discovery_auth_red.rs`):
`596c26bf4b0d84e094f174db410d92b39151f7e6`
