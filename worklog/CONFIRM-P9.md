# CONFIRM-P9 — VERIFY-ONLY INT 8 suites

Rev: `b60ceda1eaec7f3a7df0c0eb17328a6eef6368dc`.
Workdir: /home/rashid/projects/opencode-rk.
Dirty-on-arrival: y (ralph.json M + worklog/ACCEPTANCE-FLIP-PROPOSAL.md untracked; ralph diff 82+/82- pre-existing, not this lane).
Lease: VERIFY-ONLY. No product/test edits. No frozen/lib.rs/ralph.json touches by this lane.

## Serial protocol
`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`, `timeout 120`, `--test-threads=1`, `free -h` before each run (~1.0Gi avail each run, 6.2Gi total).

## Results

| suite | tests | log |
|---|---|---|
| int_alias | 5/5 | /tmp/opencode/p9-int.log |
| int_backoff | 5/5 | /tmp/opencode/p9-int.log |
| int_catentry | 5/5 | /tmp/opencode/p9-int.log |
| int_client | 5/5 | /tmp/opencode/p9-int.log |
| int_connect | 5/5 | /tmp/opencode/p9-int.log |
| int_events | 5/5 | /tmp/opencode/p9-int.log |
| int_handler_lane | 5/5 | /tmp/opencode/p9-int.log |
| int_ledger | 5/5 | /tmp/opencode/p9-int.log |

All exits 0. Every suite `5 passed / 0 failed`.

## Totals
- suites: 8. tests passed: 40/40. failed: 0.

## Stub scan
`todo!` over crates/providers/src: 0 hits (rc=1). `unimplemented!`: 0 hits (rc=1). No stubs.

## Guards
- sha256 on-arrival: ralph.json `4b99ccdc6fbe9df8397d15af7ed8682e43fd6de660cb5e2df71465e5835ae9c6`.
- sha256 at close: ralph.json `8d1e89f91b4241a78a6d2f4188e5e73ae989889e28d8ba036c8448c84d4e3ecb`, Cargo.lock `a04622ba8bf5d7694390a8b1a30d4ad083a98b51cffdfac533db6ebc13d97d2d`. Drift mid-run by sibling lanes, not this lane.
- ralph.json dirty pre-existing on arrival; lane made no edit (diff names added during run by sibling lanes only: FEATURES.md, REL-00x, CONFIRM-P8/P10, etc.).
- `frozen/` dir absent; no frozen touch. No `*/lib.rs` edit by this lane.

Edits: this file only.
