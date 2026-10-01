# G4 session execution ownership

## Package and frozen failure

- Package: `G4-SESSION-EXECUTION-SERIALIZATION`.
- Implementation base: `75ad7b5b681c9bcdb4d37f6f84f3f410e87dabda`.
- Changed implementation paths: `crates/server/src/lib.rs`, this worklog.
- Frozen test: `crates/server/tests/session_execution_ownership.rs`, SHA-256
  `9fdb5a6c01eb7d663421956fda1cc6c5bbf3ffcd43f7873f44bfb8e1f1cbb70c`.
- Frozen command manifest: retained `v2-session-ownership-red-spec.json`, SHA-256
  `7684e070a6cae81cf2d955031d03a051b77baac6f9e6d940c74e4bc042f6a673`.
- Exact canonical RED receipt:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-session-owner-canonical-red-75ad7b5-r_1brj9z/receipt.json`.
  Format passed; the compiling five-case gate had two passes, three failures,
  zero ignored. All same-session pairs reported `same session provider overlap: 2`.

## Authority and implementation

Pinned OpenCode `95daf90670b7c039c436c85537da5fbfe2205b41`:

- `packages/core/src/session/run-coordinator.ts:5-15,67-79` establishes
  session-keyed execution ownership while different keys can run concurrently.
- `packages/core/src/session/execution/local.ts:16-21,31-35` connects that
  coordinator to session IDs and the runner.

The frozen contract accepts either successful serialized execution or the
existing typed HTTP 429 capacity refusal with zero provider/durable-input
effects. This slice preserves the bounded admission policy: both HTTP turn
routes acquire the same session ownership before provider access or appending a
user message. Two different sessions may still use both global slots.

A fixed two-entry registry is bounded by the existing global capacity. It
allocates no queue and holds no blocking lock across an await. An RAII permit
reclaims the session entry on early errors and completion. A streaming body owns
the permit through all provider/tool/continuation stages and releases it on body
drop, after the provider stream field is dropped. Unauthorized requests are
rejected by the existing middleware before admission.

This is the frozen ownership scope. It does not establish complete upstream
join/wake/coalescing or authenticated interruption parity. No admitted effects
are automatically replayed.

## Verification status

Candidate implementation, awaiting independent preverification and the exact
integrated gate. Frozen tests, manifests, dependencies and `Cargo.lock` are
unchanged. Required gate:

```text
cargo test --offline --locked -p opencode-rk-server --test session_execution_ownership -- --test-threads=1 --nocapture
```

The parent integration writer will land this only after the priority native
resize product integration, then rerun the same gate on the integrated SHA.
