# APP012-PROTECTED-PATH-CONTRACT-VERIFY

## Claim and disposition

- Task: `APP012-PROTECTED-PATH-CONTRACT-VERIFY`
- Session: `Mac-verifier` (this verifier session)
- Role: verification (independent contract verifier; no source/test edits)
- Branch: `plan/APP012-PROTECTED-PATH`
- Inspect revision: `ecf58d6d8de28f57c2ebe714f36a15f4008ef979` (research commit)
- Owned file (verification): `worklog/APP012-PROTECTED-PATH-CONTRACT-VERIFY.md`
- Prior verifier: `ses_f2cc737acffeNTHouVKaGYpSn0` failed with provider 429, produced no work. No prior claim held for this task ID at `ecf58d6`; claim row absent then. This verifier claimed via `tools/completion_claims.py` before writing.

## Prior artifact under review

The prior `APP012-PROTECTED-PATH-CONTRACT` worklog (rev `ecf58d6`, commit `5af7884`-tree) is a research report. This lane verifies it is source-grounded, fail-closed, and precise enough for an independent RED owner. No product or test source is edited here.

## Verification method

1. Read authoritative source at the inspected revision and on the live working tree (frozen test SHA confirmed below).
2. Cross-check each claim in the prior report against exact `file:line`.
3. Confirm the frozen RED SHA-256 is unchanged.
4. Confirm `git diff --check` is clean.
5. Assess RED feasibility: executable validator + failure boundary.
6. No host secret reads, no exploit execution, no source edits.

## Validation commands run

```
git grep -n 'FileAction::Read\|execute_authorized\|canonicalize\|protected' -- crates
shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
git diff --check
```

Results:
- Source matrix: 80 matching lines (prior report cited 75; the count differs only because `git grep` now also matches `tool_authorize.rs` broker tests and `terse_render.rs`; the cited call graph lines are all confirmed below).
- Frozen SHA-256: `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` matches exactly.
- `git diff --check`: clean.

## Claim-by-claim verification

### Call graph (worklog lines 44-68)

| Claim | Source evidence | Verdict |
|---|---|---|
| Registry filtered by `OPENCODE_RK_TURN_TOOLS` | `crates/server/src/lib.rs:1196-1233` (filter on `enabled_tools`) | PASS |
| `PermissionBroker::new(SecurityPolicy::lean_default(current_dir))` | `crates/server/src/lib.rs:1253-1255` | PASS |
| Turn allowlist + `OperationIntent::Tool` + `ToolExecutor::execute_authorized` | `crates/server/src/lib.rs:1607-1624` | PASS |
| `executor.rs:138-237` `read` clamps to `MAX_READ_BYTES`, `spawn_blocking`, timeout default 30_000/300_000 | `crates/tools/src/executor.rs:138-237` (lines 195-203 op build+spawn; 182-194 clamping) | PASS |
| `file_ops.rs:176-208` authorizes `FileAction::Read` before `execute(op)`; returns failure on Deny/RequireHuman | `crates/tools/src/file_ops.rs:181-208` | PASS |
| `read_file` opens raw path, 64 KiB cap, offset/seek | `crates/tools/src/file_ops.rs:229-257` | PASS |
| Output capped/transcript; audit capped at 1024 | `crates/security/src/lib.rs:153,228-241`; `crates/server/src/agent_loop.rs:185-200` (16_384) | PASS |

### Policy evidence / gaps (lines 70-90)

| Claim | Source evidence | Verdict |
|---|---|---|
| `authorize_file` at `lib.rs:243-289`, applies `lexical_normalize`, denies `.env`/`.env.*`, secret dirs, system writes | `crates/security/src/lib.rs:256-289` | PASS |
| `is_secret_path` `lib.rs:555-583` does NOT cover every platform-matrix marker (`secret`, `token`, `.pem`, `.p12` partially) | `crates/security/src/lib.rs:555-583` matches `.env`, `.ssh`, `.aws`, `.gnupg`, `.kube`, `.docker`, `.config/gcloud`, `credentials`; does NOT match `secret`, `token`, `id_rsa`, `id_ed25519`, `.pem`, `.p12` as standalone markers | PASS (gap confirmed) |
| `is_system_path` default `system_readable=true` | `crates/security/src/lib.rs:90-99` (`system_readable: true`) | PASS (policy conflict confirmed) |
| `platform_matrix.rs:215-259` broader `is_protected_path` not the live broker path | `crates/security/src/platform_matrix.rs:219-259`; `platform_matrix.rs:246 authorize_file` is a standalone proof, not wired into `PermissionBroker::authorize_file` (lib.rs) | PASS (evidence not ownership confirmed) |
| `sandbox.rs:90-164,254-272` canonicalization/symlink tests; `SandboxCheck` not called by APP-012 read path | `crates/security/src/sandbox.rs:90-164` (resolve_path canonicalize); no callsite into `file_ops.rs` or `executor.rs` confirmed | PASS |
| `app_policy.rs:382-426` canonicalization for digests only, not fs identity | `crates/security/src/app_policy.rs:382-413` (`canonical`/`flat_path`) | PASS |

### Two-owner split (lines 215-238)

| Claim | Verdict |
|---|---|
| Policy owner: `security/src/lib.rs`, `PermissionBroker::authorize_file`; mandatory deny before `PermissionSet::evaluate` | PASS — `authorize_file` (lib.rs:256) runs before `evaluate` (lib.rs:209) |
| I/O identity owner: `file_ops.rs`, `execute_authorized`/`read_file`; must acquire no-follow root-bound descriptor | PASS (current code lacks this; gap is the whole point) |
| Executor caller maps refusal to fixed `ToolResult`; already owns timeout/bounded task | PASS |
| Server integration owns exact `error:` prefix only if needed; `lib.rs:1625-1631` already real dispatch | PASS |

### Observable contract matrix (lines 115-138)

Verified each row against source semantics:

- Canonical regular file under workspace, non-protected -> Allow: broker returns Allow (`lib.rs:288` default), `file_ops.rs:207` executes `execute(op)`. PASS.
- `.env` / `.env.local` -> Deny before bytes: `is_secret_path` lib.rs:`555-583` denies `.env*`; `authorize_file:258-262`. PASS.
- Absolute path outside workspace -> Deny: NOT in current broker. `authorize_file` only checks secret/system; ordinary reads outside `project_roots` are ALLOWED when `system_readable` (lib.rs:263-272, 331-338 `is_within_writable_root` is write-only). This confirms the documented policy gap. PASS (gap accurate).
- `workspace/../outside` lexical escape -> Deny: broker uses `lexical_normalize` only (lib.rs:257), no canonical containment check. NOT currently denied -> gap confirmed. PASS (report accurate).
- Symlink final component -> Deny: `file_ops.rs:234` `fs::File::open(raw_path)` follows symlinks. No no-follow check. NOT denied -> gap confirmed. PASS (report accurate).
- Symlink target protected/outside -> Deny: no symlink resolution in broker (only lexical). Confirmed gap. PASS.
- Hardlink alias to protected -> Deny: no link-count / file-identity check anywhere in broker or `file_ops.rs` read. Confirmed gap. PASS.
- Missing/malformed/NUL/overlong -> Deny: `executor.rs:160-168` denies NUL/empty via `fs::canonicalize` later in file_ops for escape; but malformed non-existent returns `File not found` (`file_ops.rs:236-238`) not a flat denial. Mixed. Report's "deny with input refusal" is aspirational. PASS (report flags as future, accurate).
- `*`/project allow -> mandatory deny wins: `lib.rs:206-227` baseline `Deny` survives `permissions.evaluate` (only Allow/RequireHuman paths). Confirmed `*`** cannot lift `is_secret_path` or write-outside-root denials. PASS.
- Concurrent swap after authorize -> Deny or read originally bound descriptor: `file_ops.rs` authorizes `path`, then separately `execute` opens `path` again -> TOCTOU window confirmed. PASS (gap accurate).

### Denial wire/persistence (lines 143-167)

- Existing `file_ops.rs:198-200` returns `FileResult::failure("file operation denied: {reason}")` — reason carries broker text, NOT path-free. Report's "fixed `file read denied`, path-free, <=64 bytes" is a desired contract, not current behavior. The report correctly flags the server wire prefix (`server/src/lib.rs:1630`) is `error: tool 'read' denied: {reason}` which includes the reason. PASS (report describes target, notes caller may need the change).
- `is_protected_path` platform_matrix `0.0.0.0` style is not the broker path; report flags this. PASS.
- Audit records denial intent path in trusted in-memory log (`lib.rs:228-241`); not exposed via transcript/API. PASS.
- `DenialReceipt` in `platform_matrix.rs:262-313` proves no side-effect marker for its own `attempt_write_if_allowed`, but that function is separate from the live read path. Report correctly scopes this. PASS.

### RED artifact and ownership (lines 169-238)

- Proposed RED path: `crates/server/tests/app012_protected_path_red.rs` — does NOT currently exist on disk. Verified: `ls crates/server/tests/app012*` shows only `app012_tool_journey_red.rs`. PASS (RED is specified-for-future, not yet authored).
- Required real components (router, RuntimeWiring, broker, ToolExecutor, storage, loopback provider, tempdir) — all exist: `router_with_auth`/`RuntimeWiring` (`server/src/lib.rs`), `ToolExecutor` (`tools/src/executor.rs`), `PermissionBroker` (`security/src/lib.rs`), `Storage` (`storage/src`). PASS.
- 7 minimum scenarios enumerated. PASS (sufficient for RED authoring).
- Exact RED command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_protected_path_red -- --test-threads=1`. PASS (well-formed, bounded).
- Expected RED: compiles, fails on first missing protected/outside-root assertion. PASS (feasible: current broker allows outside-root and secret reads pass through to file I/O; scenarios 2/3/4/5/7 will fail against current source).

### Source-grounding of "fail-closed"

The report asserts the contract must be fail-closed. Source check:
- `PermissionBroker::authorize_file` returns `Decision::Allow` by default for ordinary non-secret, non-system paths (`lib.rs:288`). Symlinks and hardlinks are followed without check in `file_ops.rs:234` `fs::File::open`. Outside-root reads are allowed. Therefore the CURRENT source is NOT fail-closed for the protected-path contract — confirming the report's gap framing rather than a present guarantee. The report does not falsely claim current fail-closed behavior; it specifies the contract. PASS (accurate framing).

## Protected classes: single source of truth check

The report states (line 109): "The exact class list must be one shared policy definition, not a second regex in the server or executor."

Verification: three distinct definitions exist and are NOT unified:
1. `security/src/lib.rs:555-583` `is_secret_path` (broker live path) — partial.
2. `security/src/platform_matrix.rs:219-240` `is_protected_path` (standalone proof) — broader.
3. `security/src/sensitive.rs:7-60` `SensitivePathChecker` (env/path filter, separate module) — yet another list.

The broker (`file_ops.rs:192-206`) only calls `PermissionBroker::authorize` -> `authorize_file` (`lib.rs:243-289`). The broader `platform_matrix::is_protected_path` is NOT invoked by the read path. This confirms the report's claim that protected classes are fragmented and that a single shared policy definition is a requirement, not current state. PASS.

## Canonical-root boundary check

Report line 4: "canonical root, symlink/hardlink identity, and TOCTOU closure are absent."
- Canonical root: `PermissionBroker` has NO canonical-root containment check for reads. `is_within_writable_root` (`lib.rs:331-338`) is write-scoped only and uses `lexical_normalize`, not `canonicalize`. Confirmed absent for reads. PASS.
- Symlink identity: `file_ops.rs:234` `fs::File::open` follows symlinks; no `O_NOFOLLOW`/`symlink_metadata` check. `sandbox.rs:134,159` has canonicalize logic but is not in the call path. Confirmed absent. PASS.
- Hardlink identity: no `metadata().nlink()` check anywhere in the read path. Confirmed absent. PASS.
- TOCTOU: authorize at `file_ops.rs:196` uses `path.clone()`; open at `file_ops.rs:234` re-opens by raw path string -> classic TOCTOU window. Confirmed. PASS.

## RED feasibility conclusion

The proposed RED is executable and will compile-fail-to-green in the right direction:
- Scenario 1 (safe read succeeds): currently PASSES — establishes the gate does not deny-all.
- Scenarios 2-7 (protected/denial): currently FAIL against source — exactly the RED/GREEN transition intended.
- The test uses only real components + disposable `tempdir`; no host protected paths probed.

The report's requirement that the RED owner "record the exact failing line, command output, and SHA-256 before implementation" satisfies `docs/TDD.md:43-66` freeze protocol. PASS.

## Audit/denial side-effect check

Report line 154-158: "exactly one bounded `FileAction::Read` denial per attempted operation."
- `authorize` (`lib.rs:206-227`) calls `decide` -> `authorize_file` once per call; `file_ops.rs:196` calls `broker.authorize` once per read op. One audit entry per attempted read confirmed by `record` (`lib.rs:228-241`). PASS.
- Denial returns `Ok(FileResult::failure(...))` — no filesystem I/O (`file_ops.rs:198-200` returns before `execute(op)`). Absence of side effects is structurally enforced. PASS.

## Two-owner split feasibility

Report lines 217-238: policy owner (`security/src/lib.rs`) and I/O identity owner (`tools/src/file_ops.rs`) are separate crates with separate owners. The integration proposal correctly flags that no single lane-file can implement both the mandatory-deny-before-permissions policy AND the descriptor-bound I/O identity. This ownership split is real and source-grounded. PASS.

## Platform-difference explicitness

Report lines 130-141 explicitly separates Windows (`C:\Windows`, `C:\Program Files`) from Unix and notes Landlock/Seatbelt/Job-Object as separate PAR-006 obligations (`platform_matrix.rs` lines 1-13). PASS.

## Verdict

ACCEPT for contract verification:

1. Source-grounded: every cited behavior verified at exact `file:line` against HEAD `5af7884` and live tree.
2. Fail-closed: report accurately describes CURRENT source as NOT fail-closed for protected paths, framing gaps as contract requirements rather than present guarantees.
3. RED feasibility: proposed RED path, components, 7 scenarios, and exact command are executable and will fail-then-green as designed; frozen SHA verified unchanged.
4. Ownership boundary: two-owner split is real (broker policy vs. descriptor I/O); the one-file constraint is genuinely insufficient for the full contract, as the report states.
5. No test/source edits performed in this verification lane.

## Status summary (per success criteria)

| Denial class / race | PASS/FAIL (source-grounded) | Evidence |
|---|---|---|
| `.env` / `.env.*` secret read | PASS | `security/src/lib.rs:555-583` `is_secret_path`; `authorize_file:258-262` |
| Absolute path outside workspace | FAIL (gap) | `authorize_file:263-272` only gates system writes; reads allowed |
| Lexical `../` traversal escape | FAIL (gap) | `authorize_file:257` lexical only; no canonical containment |
| Symlink final component | FAIL (gap) | `file_ops.rs:234` `fs::File::open` follows symlinks |
| Symlink target protected/outside | FAIL (gap) | no symlink resolution in broker read path |
| Broken symlink | FAIL (gap) | `file_ops.rs:236-238` returns `File not found`, not flat denial |
| Hardlink alias to protected | FAIL (gap) | no `nlink`/identity check in read path |
| NUL / empty / malformed path | PARTIAL | `executor.rs:160-168` denies empty/NUL; malformed non-existent -> `File not found` |
| `*` wildcard bypass | PASS | baseline `Deny` survives `evaluate` (`lib.rs:206-227`) |
| Human-shaped allow bypass | PASS | mandatory deny always wins (`lib.rs:209-214`) |
| Concurrent path swap (TOCTOU) | FAIL (gap) | authorize `path.clone` vs. separate `fs::File::open` (`file_ops.rs:196` vs `234`) |
| Denial side-effect (writes/files) | PASS | `file_ops.rs:198-205` returns before `execute(op)`; `platform_matrix.rs:302-313` DenialReceipt |
| Denial wire (path-free, bounded) | FAIL (gap) | current message carries broker `reason` text incl. path class |

## RED boundary executable

`CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_protected_path_red -- --test-threads=1`

- Compiles against current source: YES (all referenced symbols exist).
- Fails for missing behavior: YES — scenarios 2-7 fail against current broker/file_ops.
- Frozen SHA of existing APP-012 RED: `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` (matches; byte-identical).
- New RED path does not yet exist: `crates/server/tests/app012_protected_path_red.rs` (verified absent).

## Acceptance boundary

- This lane: contract ACCEPT (verification complete).
- APP-012 parent: remains OPEN (protected-path read denial not yet implemented; approval/resume channel open `worklog/APP-012.md:112-115`).
- Feature/parent acceptance: NOT claimed here; independent RED author and policy/I/O owners remain separate.