# Worklog APP-002-retry (daemon_client lane)

Claim: authenticated singleton discovery client, owned file only
`crates/cli/src/daemon_client.rs` (created, does not wire into crate).

Source evidence (HEAD 5af7884):
- `crates/server/src/daemon.rs:43-48` `pid_alive` (/proc check, pid 0 dead).
- `crates/server/src/daemon.rs:107-112` `BackendDescriptor` {pid, http_origin, schema_version}.
- `crates/server/src/daemon.rs:133-149` `read_backend_descriptor`: schema==WIRE, pid alive, origin starts `http://127.0.0.1:` else None.
- `crates/contracts/src/lib.rs:14` WIRE_SCHEMA_VERSION=1 (copied as EXPECTED_SCHEMA_VERSION, dep-free for standalone rustc).
- `crates/cli/src/chat.rs:398-400` probe = GET /health == 200 only; `404-430` spawn/readiness/owned-kill rule.

Observed: `daemon_client.rs` did not exist; client had no descriptor validation (chat.rs probes fixed addr only).
Target boundary: pure types + validation, no sockets/files. Caller supplies bytes, symlink_metadata-derived meta, pid liveness, /health status.

Tests (13, `#[cfg(test)]`, std only): valid accept; schema mismatch; dead+zero pid stale; 12 non-loopback origins; 9 forged/malformed; oversize bytes+meta; symlink; wrong-owner (+own ok); version 3-way; occupied-port safe error (exhaustive match, no kill variant exists); non-200 not healthy; reuse/start matrix; unknown-fields forward compat.

Decisions: exact `http://127.0.0.1:<port>` (reject localhost/0.0.0.0/LAN/https/paths/leading-zero); size check before parse; symlink+owner checked on caller-supplied meta; LifecycleAction has no kill variant by design; no todo!/unimplemented!, `#![forbid(unsafe_code)]`.

Remaining: mod wiring + real IO caller + server-side symlink/owner hardening belong to integrator/other lane, not this file.
