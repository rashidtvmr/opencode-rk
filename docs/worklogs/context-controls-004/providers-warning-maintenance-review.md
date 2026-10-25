# Providers warning-maintenance review

**Verdict: APPROVE** — independent mechanical-test-owner review of the three
provider warning corrections and the mechanical test correction.

The production changes are semantically unchanged: `save_at` is test-only,
`serde_json::json` is test-only, and the unused `model` parameter is explicitly
named `_model`. The test-only correction changes only the assertion macro from
`assert!(HANDSHAKE_TIMEOUT_MS == 10_000)` to
`assert_eq!(HANDSHAKE_TIMEOUT_MS, 10_000)`, retaining the exact 10-second
constant contract. This addresses the clippy constant-assertion warning; the
compiler-warning RED was not fabricated product RED.

SHA-256 (post-change files):

```text
crates/providers/src/custom_provider.rs  8f25ffe19f84a43a6c3dc8a6bf6c99cf4d3d1f85b1ca65588c8954961d8d4dd0
crates/providers/src/imported_config.rs  8dcbfe0acff19af8733c265955a17ec61c12624e96039b947322e8b7440f5fe5
crates/providers/src/responses.rs        c4b932257d7cdbd86e7cb24bd810ddc0cd174b06b133073cf3e44584dfce8d2c
crates/providers/tests/mcp_transport.rs  b4b9f98b2b6295918dfaa78394882624af6e9a72b7aca3dd0821c428547ba52d
```

`git diff --check` passed for the four reviewed files. Focused clippy and
provider-unit execution remain with the parent as requested.

## Second mechanical review

The five-path review is approved. Current SHA-256 manifest:

```text
crates/providers/src/custom_provider.rs  8f25ffe19f84a43a6c3dc8a6bf6c99cf4d3d1f85b1ca65588c8954961d8d4dd0
crates/providers/src/imported_config.rs  8dcbfe0acff19af8733c265955a17ec61c12624e96039b947322e8b7440f5fe5
crates/providers/src/responses.rs        c4b932257d7cdbd86e7cb24bd810ddc0cd174b06b133073cf3e44584dfce8d2c
crates/providers/tests/mcp_transport.rs  b4b9f98b2b6295918dfaa78394882624af6e9a72b7aca3dd0821c428547ba52d
crates/providers/tests/integration_connection.rs b68bcbc9ce464d1e93c47752495878a57f984744d7d061a214b4fb10b03227fc
```

The second compiler log was accidentally overwritten by a reviewer receipt;
the misplaced receipt is preserved as-is and is not raw execution evidence.
The original `CodeRequired` dead-code error was captured in the tool record.
No green result is claimed; the parent will create a fresh third log.

## Third mechanical review

Confirmed `ProviderMethodKind`, `classify`, and `is_empty` are valid public/private
production code, while the warning is isolated to the path-included test module.
Added only module-scoped `#[allow(dead_code)]`; no assertions or production code
changed. Mechanical scope is approved. Current six-file SHA-256 manifest:

```text
crates/providers/src/custom_provider.rs  8f25ffe19f84a43a6c3dc8a6bf6c99cf4d3d1f85b1ca65588c8954961d8d4dd0
crates/providers/src/imported_config.rs  8dcbfe0acff19af8733c265955a17ec61c12624e96039b947322e8b7440f5fe5
crates/providers/src/responses.rs        c4b932257d7cdbd86e7cb24bd810ddc0cd174b06b133073cf3e44584dfce8d2c
crates/providers/tests/mcp_transport.rs  b4b9f98b2b6295918dfaa78394882624af6e9a72b7aca3dd0821c428547ba52d
crates/providers/tests/integration_connection.rs b68bcbc9ce464d1e93c47752495878a57f984744d7d061a214b4fb10b03227fc
crates/providers/tests/int_methods.rs     a1bc55e02849a0c91e477edb9c93fa6a6cc21e6968e1319196b15c180a571085
```

## Fourth mechanical review

The observed `int_refresh.rs` diagnostics are the same isolated path-included
test-module dead-code artifact, not a materially different root cause. The
production constant and public methods remain intact and their assertions are
preserved. Standard policy: apply module-scoped `#[allow(dead_code)]` only to
path-included test modules when diagnostics demonstrate unused production
items caused by isolated compilation; never weaken production lints or add
synthetic assertions. Added only that attribute in `int_refresh.rs`.

Current seven-path SHA-256 manifest:

```text
crates/providers/src/custom_provider.rs  8f25ffe19f84a43a6c3dc8a6bf6c99cf4d3d1f85b1ca65588c8954961d8d4dd0
crates/providers/src/imported_config.rs  8dcbfe0acff19af8733c265955a17ec61c12624e96039b947322e8b7440f5fe5
crates/providers/src/responses.rs        c4b932257d7cdbd86e7cb24bd810ddc0cd174b06b133073cf3e44584dfce8d2c
crates/providers/tests/mcp_transport.rs  b4b9f98b2b6295918dfaa78394882624af6e9a72b7aca3dd0821c428547ba52d
crates/providers/tests/integration_connection.rs b68bcbc9ce464d1e93c47752495878a57f984744d7d061a214b4fb10b03227fc
crates/providers/tests/int_methods.rs     a1bc55e02849a0c91e477edb9c93fa6a6cc21e6968e1319196b15c180a571085
crates/providers/tests/int_refresh.rs     95203bfa2a86ae552b402d3d456af607a0b1464ed7f84943b7982584531724d7
```

## Fifth maintenance gate

Command executed with the requested unique log and environment:

```text
CARGO_TARGET_DIR=$PWD/target/native-usability-candidate-20261024-0542 CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 RUSTUP_TOOLCHAIN=1.98.1 DYLD_LIBRARY_PATH=/Users/mymac/.local/lib cargo clippy -p opencode-rk-providers --all-targets -- -D warnings
```

**FAIL (exit 101).** The preserved log is
`docs/worklogs/context-controls-004/providers-warning-maintenance-clippy-fifth.log`.
It reports a new, substantive-outside-grant test lint in
`crates/providers/tests/location_ctx.rs`: redundant closure and non-snake-case
`WsHintAlias`. No source changes were made, no provider tests were run, and the
failure is handed off without widening this maintenance scope.

The seven reviewed source/test hashes remain unchanged from the manifest above.

## Location-context mechanical correction

Applied the two observed test-only lints from the preserved fifth clippy log:
replaced the redundant closure with the equivalent function item and renamed
the private helper to snake case. Assertions and behavior are unchanged; no
production lint was suppressed. Current eight-path SHA-256 manifest:

```text
crates/providers/src/custom_provider.rs  8f25ffe19f84a43a6c3dc8a6bf6c99cf4d3d1f85b1ca65588c8954961d8d4dd0
crates/providers/src/imported_config.rs  8dcbfe0acff19af8733c265955a17ec61c12624e96039b947322e8b7440f5fe5
crates/providers/src/responses.rs        c4b932257d7cdbd86e7cb24bd810ddc0cd174b06b133073cf3e44584dfce8d2c
crates/providers/tests/mcp_transport.rs  b4b9f98b2b6295918dfaa78394882624af6e9a72b7aca3dd0821c428547ba52d
crates/providers/tests/integration_connection.rs b68bcbc9ce464d1e93c47752495878a57f984744d7d061a214b4fb10b03227fc
crates/providers/tests/int_methods.rs     a1bc55e02849a0c91e477edb9c93fa6a6cc21e6968e1319196b15c180a571085
crates/providers/tests/int_refresh.rs     95203bfa2a86ae552b402d3d456af607a0b1464ed7f84943b7982584531724d7
crates/providers/tests/location_ctx.rs    67a06c3bff137935d34daa074e73323c14e679ff396f910c2796b15134f78446
```

## Extended provider maintenance gate

The sixth clippy execution failed on an assertion-preserving test lint in
`prov_024_provider_contracts.rs`; `unwrap_err()` on a literal `Err` was replaced
with explicit `let Err(error)` matching. The seventh execution failed on an
unreachable assignment in `refresh_gate.rs`; only that assignment was removed.
The eighth execution failed on the same path-included dead-code artifact in
`int_connect.rs`; added only the narrow test-module `#[allow(dead_code)]`.
These are mechanical test corrections; no production lint suppression,
assertion weakening, or green result is claimed.

Current changed-file SHA-256 manifest:

```text
crates/providers/src/custom_provider.rs  8f25ffe19f84a43a6c3dc8a6bf6c99cf4d3d1f85b1ca65588c8954961d8d4dd0
crates/providers/src/imported_config.rs  8dcbfe0acff19af8733c265955a17ec61c12624e96039b947322e8b7440f5fe5
crates/providers/src/responses.rs        c4b932257d7cdbd86e7cb24bd810ddc0cd174b06b133073cf3e44584dfce8d2c
crates/providers/tests/mcp_transport.rs  b4b9f98b2b6295918dfaa78394882624af6e9a72b7aca3dd0821c428547ba52d
crates/providers/tests/integration_connection.rs b68bcbc9ce464d1e93c47752495878a57f984744d7d061a214b4fb10b03227fc
crates/providers/tests/int_methods.rs     a1bc55e02849a0c91e477edb9c93fa6a6cc21e6968e1319196b15c180a571085
crates/providers/tests/int_refresh.rs     95203bfa2a86ae552b402d3d456af607a0b1464ed7f84943b7982584531724d7
crates/providers/tests/location_ctx.rs    67a06c3bff137935d34daa074e73323c14e679ff396f910c2796b15134f78446
crates/providers/tests/prov_024_provider_contracts.rs cc355c5554f087cfd03f85a6467e0d549a3baf4c8226b86d25fd507b0453ea44
crates/providers/tests/refresh_gate.rs    6f7303ddc03077196d73f84b4d2c21831bded80236c9f0f6b5531b60ac73c686
crates/providers/tests/int_connect.rs     27c4ebd44e959c4cd8d93f92cdd5f60232dec5a4e03bd33d94d12588097c5a11
```

## Final execution result

Final all-target clippy command (`--keep-going`, requested environment) passed
with exit 0 in `providers-warning-maintenance-clippy-final3.log`. The full
`cargo test -p opencode-rk-providers` command then passed with exit 0 in
`providers-warning-maintenance-test-final.log`: 88 library tests plus all
integration targets passed, with zero failures (77 successful test-result
groups; 0 failed). No production behavior was intentionally changed; the two
observed production fixes were exact mechanical lint transformations.

Final changed-path SHA-256 manifest:

```text
crates/providers/src/custom_provider.rs  8f25ffe19f84a43a6c3dc8a6bf6c99cf4d3d1f85b1ca65588c8954961d8d4dd0
crates/providers/src/imported_config.rs  8dcbfe0acff19af8733c265955a17ec61c12624e96039b947322e8b7440f5fe5
crates/providers/src/responses.rs        c4b932257d7cdbd86e7cb24bd810ddc0cd174b06b133073cf3e44584dfce8d2c
crates/providers/src/budget.rs            c59db87f158b539c9384d8331c69deea59c4dd49dad3d85aeb4abaf486832a89
crates/providers/src/app_routing.rs       3a89158da648bcaa72366dbe21dd986f5a72c5a9e91962a054148b03789a7b85
crates/providers/tests/mcp_transport.rs  b4b9f98b2b6295918dfaa78394882624af6e9a72b7aca3dd0821c428547ba52d
crates/providers/tests/integration_connection.rs b68bcbc9ce464d1e93c47752495878a57f984744d7d061a214b4fb10b03227fc
crates/providers/tests/int_methods.rs     a1bc55e02849a0c91e477edb9c93fa6a6cc21e6968e1319196b15c180a571085
crates/providers/tests/int_refresh.rs     95203bfa2a86ae552b402d3d456af607a0b1464ed7f84943b7982584531724d7
crates/providers/tests/location_ctx.rs    67a06c3bff137935d34daa074e73323c14e679ff396f910c2796b15134f78446
crates/providers/tests/prov_024_provider_contracts.rs cc355c5554f087cfd03f85a6467e0d549a3baf4c8226b86d25fd507b0453ea44
crates/providers/tests/refresh_gate.rs    6f7303ddc03077196d73f84b4d2c21831bded80236c9f0f6b5531b60ac73c686
crates/providers/tests/int_connect.rs     27c4ebd44e959c4cd8d93f92cdd5f60232dec5a4e03bd33d94d12588097c5a11
crates/providers/tests/imported_config.rs 1e104a79dc4ae5acca2dac69815f91e61bd15f2f8cf5616b191fb8cebde46459
crates/providers/tests/responses_request_retry.rs 1ba3e5165831d09f18d9044e2c14226d0414989d8f56ef31c0bbc761fcb5e7b9
crates/providers/tests/int_mcp_lane.rs    46f15145cbee64c1e850300172c0587f9321690881b39a538e03cceb58aef870
crates/providers/tests/connection.rs      9fb3ec7ed7a729d827edced993df1858b96bbac85fe6da1c8b82e38e43e2a09f
crates/providers/tests/provider_dispatch.rs a937094cd95d3661e4f67716d15584d8b315b05e6cdc3d807833802794babecb
```

## Keep-going gate outcome

Executed the requested all-target diagnostic command with `--keep-going` in
`providers-warning-maintenance-clippy-keep-going.log`; result **FAIL (101)**.
The log collected mechanical test diagnostics in `imported_config.rs`,
`responses_request_retry.rs`, `int_mcp_lane.rs`, `connection.rs`,
`provider_dispatch.rs`, and `refresh_gate.rs`. It also exposed two production
lint failures in `crates/providers/src/budget.rs` (`unused_mut`) and
`crates/providers/src/app_routing.rs` (`cloned_ref_to_slice_refs`). Because
production edits are outside this grant, execution stopped without claiming
green or running the provider test suite. No additional source changes were
made after the keep-going diagnostics.

Final test-only changed-file hashes remain the manifest above, plus:

```text
crates/providers/tests/prov_024_provider_contracts.rs cc355c5554f087cfd03f85a6467e0d549a3baf4c8226b86d25fd507b0453ea44
crates/providers/tests/refresh_gate.rs    6f7303ddc03077196d73f84b4d2c21831bded80236c9f0f6b5531b60ac73c686
crates/providers/tests/int_connect.rs     27c4ebd44e959c4cd8d93f92cdd5f60232dec5a4e03bd33d94d12588097c5a11
```
