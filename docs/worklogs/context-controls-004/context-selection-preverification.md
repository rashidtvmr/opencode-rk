# 004-B context selection — current candidate verification

Base: `e3069eee04d9a3414517b2452df803c9c145c9d3`.
Status: **Focused candidate checks PASS; functional and security source reviews
APPROVE. Integration pending. Not installed release ACCEPTED.**

The coherent slice includes detailed native inspection, acknowledged persistent
session-scoped selectors, filtering before provider admission budgets, atomic
typed tool pairs, opaque/summary checkpoint selectors, and transactional cleanup
of obsolete checkpoint selection on replacement. Durable history and current
instruction/permission assembly are preserved.

## Exact commands and results

All commands used:

```sh
CARGO_TARGET_DIR="$PWD/target/native-usability-candidate-20261024-0542"
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 RUSTUP_TOOLCHAIN=1.98.1
DYLD_LIBRARY_PATH=/Users/mymac/.local/lib
```

as command-local environment variables.

- `cargo test -p opencode-rk-cli --bin oc2 --features native` — **769 passed**,
  `context-native-reviewed-final-check.log`.
- `cargo test -p opencode-rk-server --lib --test session_context_selection
  --test context_selection_projection --test context_checkpoint_selection
  --test session_compaction_settings --test compaction_settings_overrides
  --test session_native_compaction --test session_fallback_compaction
  --test session_fallback_streaming --test fallback_compaction_failure
  --test memory_provider_budget` — **254 passed across 11 targets**,
  `context-backend-reviewed-final-check.log`.
- `cargo test -p opencode-rk-storage --lib
  --test typed_history_boundary_regression --test typed_history_component
  --test compaction_tail_bounds --test compaction_checkpoint_corruption` —
  **137 passed across 5 targets**, `context-storage-reviewed-final-check.log`.
- `cargo build -p opencode-rk-cli --features native --bin oc2` — **PASS**,
  `context-native-reviewed-candidate-build.log`.
- Scoped `git diff --check` — **PASS**.
- Frozen context and compaction contract hashes were checked against disk.
  The checkpoint opaque-marker observer correction is independently recorded;
  its original RED hash is preserved instead of relabeled unchanged.

## Native observation

`context-native-terminal-candidate-journey.md` records actual native selection,
mandatory memory-source behavior, history preservation, daemon termination,
exact-session restart, persistent exclusion, re-inclusion, and shell restoration.
Screenshots and authenticated API snapshots remain in the disposable fixture.
This is a debug-candidate observation, not an exact-final-release journey.

## Independent review

Current source fingerprints and verdicts are in:

- `context-selection-backend-functional-review.md`
- `context-selection-backend-security-review.md`
- `context-native-functional-review.md`
- `context-native-security-review.md`

Integration must verify the recorded fingerprints against the exact staged
source. A negative review cannot be superseded by counting other approvals.

## Remaining observed failures

The broader `cargo test -p opencode-rk-server` run stopped at the existing
`live_write_tool_inroot_is_allowed_and_persisted` transcript-payload assertion;
see `server-broad-current-context-check.log`. It is not reported as PASS.
Foreign-listener identity refusal is being tested at the installed default-entry
boundary; low-level descriptor-reader tests do not establish that guarantee.
The rounded contrasting-outline prototype remains visually NOT ACCEPTED.

The user-authorized Phase-1 override waives repository guard/queue ceremonies
for product work; product permission enforcement and semantic assertions remain
required. No final installation, push, or authorized application restart has
been performed for this candidate.
