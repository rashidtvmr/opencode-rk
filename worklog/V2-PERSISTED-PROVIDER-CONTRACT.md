# V2-PERSISTED-PROVIDER-CONTRACT (contract/test owner)

Branch: `v2/provider-auth-contract`, base `7fe4656`.
Lane: G2 persisted credential failure. Package: ready-to-run contract only.
No product source touched. No commit. No Cargo run (heavy slot held by
native fixture verifier).

## Import

- `crates/server/tests/prov_025_request_auth.rs` restored via
  `git restore` from historical `4c307043091232939d8df6ac43ba45c66147d381`.
- Import SHA-256: `11649945826b9e3e2f9b4ebff3aaa2b6d966084f8a8f881368db54c937977332`
  (441 lines, header still v3 text at import).
- Only owned edits applied (mechanical, semantics preserved):
  1. Header comment: import SHA recorded, "clean RED unclaimed until
     compiled target demonstrates it".
  2. `Server::new`: fail-fast assert that
     `<data>/runtime/opencode-rk.sock` path length < 104 chars, with
     message `disposable HOME too long for unix socket; rerun with a short TMPDIR`.
- Current proposed test SHA-256:
  `9aafd08cb174e5cc9ac673c44bcc60aacb97fb7c1cd38896f1ea3bc9b19813cc`
  (454 lines). Untracked, uncommitted. All six semantic controls byte-identical:
  ambient nonstream, ambient stream write, persisted+malformed-sibling,
  persisted-over-ambient precedence, no-usable-auth zero-request, persisted
  stream write follow-up (`call030`, `write success`, terminal completed).
  Assertion helpers (`assert_auth` exact equality, `assert_no_daemon_bearer`,
  `no_secret`, `terminal_success`) untouched.

## Portable fixture pitfalls inspected (no Cargo run)

- SUN_LEN: daemon binds unix socket `<data>/runtime/opencode-rk.sock`
  (`crates/server/src/daemon.rs:160`, `SingletonDaemon::bind`). Long
  macOS `TMPDIR` + `tempfile::tempdir()` HOME + `rk-state` overflows 104
  chars; daemon exits 1 with `daemon io: path must be shorter than SUN_LEN`.
  Observed directly in prior route probe. Owned assert above converts this
  to an explicit test failure. Run instruction: short TMPDIR
  (e.g. `/private/var/.../T/pp`, HOME 71 chars, verified).
- Provider loop boundedness: per-round `READY_TIMEOUT` (12s) accept deadline,
  `stop.recv_timeout(25ms)` cancel check, `MAX_WIRE_BYTES` (256KiB) cap,
  `IO_TIMEOUT` (8s) reads, `Drop` kills/joins. `finish()` sends cancel then
  joins. `no_usable_auth` case: provider thread exits via cancel or deadline;
  no hang, but note strict `assert!(captures.is_empty())` requires the
  target to make zero requests (current baseline satisfies: 503 before dial).
- Daemon lifecycle: `ChildGuard`/`Server Drop` kill+wait; readiness loop
  panics on early exit, 12s bound. No orphan risk beyond normal kill.
- `ENV_LOCK` serializes the six tests (env_clear spawn is per-child, lock is
  belt-and-braces). No change needed.
- Deps present in `crates/server/Cargo.toml`: `serde_json.workspace`,
  `tempfile.workspace`. `#[test]` fns (not tokio) use blocking sockets;
  consistent with existing suite style.

## Upstream 95daf906 evidence (read-only)

- `packages/opencode/src/auth/index.ts`: `file = path.join(Global.Path.data,
  "auth.json")`; `all()` returns `OPENCODE_AUTH_CONTENT` JSON else
  `readJson(file)` filtered by `Info` schema decode (malformed entries
  dropped); `set/remove` normalize trailing `/`, write mode `0o600`.
  `Api {type:"api", key, metadata?}`; `Info = Union(Oauth, Api, WellKnown)`.
- `packages/opencode/src/provider/provider.ts:1582-1610`: load order env
  first (`source:"env"`, `key` only when single env var), then persisted
  apikeys (`source:"api"`, `key: provider.key`) via `mergeProvider` (later
  wins, so persisted overrides ambient). Fixture `records()` schema
  (`{"openai":{"type":"api","key":...}}` + malformed sibling `"key":7`)
  matches: malformed dropped by decode, persisted key wins.
- No upstream `OPENAI_API_KEY`-only behavior; env is one source among
  config/custom/api.

## Exact observed RED (source-independent route probe, NOT the target)

Binary: `/private/.../T/opencode/v2-fc2d201-http-odzd04nk/bin/oc2`
(fc2d201 baseline, `opencode-rk 0.1.0-alpha.1`). Cleared synthetic env,
disposable HOME/data/project, loopback capture provider, short base `/T/pp`.
Evidence at `/T/pp/result.json` (+ probe scripts `/T/pp/probe.py`,
`/T/pp/probe3.py`; long-dir first attempt under
`v2-provider-persisted-probe/probe-g1g2-001/` hit SUN_LEN).

- cleared env, no key, no auth.json: session 201; turn 503
  `{"code":"service_unavailable","message":"provider credential is
  unavailable: OPENAI_API_KEY"}`, provider_requests 0.
- persisted-only auth.json (openai api key + malformed sibling): identical
  503, provider_requests 0. Persisted config ignored. This is the G2 RED the
  contract pins.
- ambient-only `OPENAI_API_KEY`: turn 201, assistant `OK-ambient-only`,
  provider_requests 1, `Authorization: Bearer synth...`. Transport healthy;
  gap is credential resolution, not HTTP.
- Target status: `prov_025_request_auth` NOT compiled, NOT run on this
  branch (heavy slot exclusive). Clean RED unclaimed. Route-probe RED is
  supporting evidence, not a substitute for the compiled target.

## Ready-to-run command (for main, after slot release)

```sh
cd /Users/mymac/Projects/opencode-rk-v2-provider-auth-contract
TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp \
OC2_TEST_BINARY=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-fc2d201-http-odzd04nk/bin/oc2 \
/usr/bin/arch -arm64 env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=1 \
cargo test --offline --locked -p opencode-rk-server --test prov_025_request_auth -- --nocapture --test-threads=1
```

Expected on unfixed baseline: the two ambient controls GREEN; the three
persisted/precedence/stream-followup tests RED (503, zero provider
requests); `no_usable_auth` GREEN (already fail-closed). Do not label
frozen/clean-RED until this output is captured. Do not create a parallel
auth source implementation; typed writer owns product files.

## Compiled clean-RED result (this branch, 2026-09-30)

Exact command above run with `--test-threads=1` (serial; parallel default
showed cross-test interference: ambient stream-write lost its marker file
and `both_present` saw a wrong credential). Full log:
`/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp/run2.log`.
E0308 fixed mechanically (`MAX_DESCRIPTOR_BYTES as usize`, same 8KiB bound;
trailer-cursor dead store neutralized, `mut file` -> `file`). No semantic
change. Post-fix compile warnings in target: only `assert_no_auth`
dead-code (historical helper, kept).

```
running 6 tests
test ambient_only_nonstream_control_succeeds ... ok
test ambient_only_stream_write_control_succeeds ... ok
test both_present_uses_persisted_precedence ... FAILED (provider Authorization credential mismatch)
test no_usable_auth_makes_zero_provider_requests ... ok
test persisted_stream_write_followup_has_effect_call_identity_and_terminal_success ... FAILED (503 vs 201)
test persisted_valid_auth_with_malformed_sibling_succeeds ... FAILED (503 vs 201)
test result: FAILED. 3 passed; 3 failed; 0 ignored; 0 measured; 0 filtered out
```

Exactly the expected 3-red/3-green clean RED: ambient controls GREEN,
no-usable-auth fail-closed GREEN, all three synthetic-persisted assertions
RED (persisted key never reaches the provider request on the unfixed
baseline). Fixture bounded cleanup verified: no orphan `oc2`/`prov_025`
processes, provider threads join via cancel/deadline, daemon kill+wait.
Test SHA-256 at commit recorded below.
