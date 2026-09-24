# APP012-PROTECTED-PATH-RED

## Claim

- Task: `APP012-PROTECTED-PATH-RED`
- Session: `ses_f2b8ff1e5ffe7F4M9TAgQi2nMB`
- Branch: `red/APP012-PROTECTED-PATH`
- Owned test: `crates/server/tests/app012_protected_path_red.rs`
- Scope: RED authoring only. No product, existing-test, Cargo, freeze, or commit changes.

## Source evidence

- `crates/security/src/lib.rs:206-227`: broker records one decision and mandatory baseline denial cannot be upgraded by permission rules.
- `crates/security/src/lib.rs:243-289`: live file policy currently permits ordinary reads outside `project_roots`, denies `.env`, and preserves `system_readable=true` compatibility.
- `crates/tools/src/file_ops.rs:176-208`: public `execute_authorized` authorizes before execution; denial currently returns a non-fixed reason.
- `crates/tools/src/file_ops.rs:228-257`: public read path opens bytes with the bounded read cap.
- `worklog/APP012-PROTECTED-PATH-CONTRACT.md:117-167`: protected-path matrix, fixed redacted denial, no content or side effects.
- `worklog/APP012-PROTECTED-PATH-CONTRACT-VERIFY.md:127-185`: RED feasibility and exact test command independently verified.

## Observable scenarios

| Scenario | Fixture | Assertion |
| --- | --- | --- |
| Safe workspace file | `workspace/safe.txt` | Successful bytes; prevents deny-all implementation. |
| Workspace `.env` | Disposable canary | Exact `file read denied`; empty content; one audit denial; unchanged file. |
| Absolute outside file | Disposable sibling | Same fixed redacted denial; no outside bytes. |
| `../` traversal | Disposable sibling through workspace path | Same denial; no path/content leakage. |
| Symlink to outside ordinary file | Unix disposable symlink | Deny without following target. |
| Symlink to outside `.env` | Unix disposable symlink | Deny without following target. |
| Broken symlink | Unix disposable symlink | Fixed denial, not filesystem error leakage. |
| Hardlink alias | Unix/macOS disposable hardlink to `.env` | Blocked: current public `file_ops` API authorizes a path then opens it, with no descriptor/file-identity seam. Removed from executable RED to avoid freezing an untestable identity assertion. |
| Wildcard/human-shaped allow | Direct broker fixture | Mandatory `.env` denial survives both grants. |
| System-readable compatibility | Direct broker check for `/etc/hosts` only | `Decision::Allow`; no host file probe. |

No rename/replace TOCTOU test is included. A deterministic synchronization barrier and an implementation-owned descriptor seam are required for useful race coverage; timing-based rename tests would be flaky. Descriptor-I/O identity remains an implementation/verifier seam. Hardlink identity is likewise blocked until that seam exists; no host-secret content scan substitutes for identity proof.

## RED status

- Test source authored. Focused Cargo run completed with one bounded worker.
- Compile: PASS. No fixture/API compile errors.
- Behavioral result: `7 tests, 2 passed, 5 failed`.
- Passing: `app012_workspace_read_succeeds`; `app012_explicit_system_read_compatibility_remains_broker_visible`.
- Expected RED failures: `app012_workspace_env_read_is_redacted` and the same denial assertion in `app012_wildcard_and_human_allow_cannot_bypass_env_denial` (current message is `file operation denied: secret-bearing paths are not accessible to agents`, not fixed); `app012_absolute_outside_read_is_redacted`; `app012_parent_traversal_read_is_redacted`; `app012_symlinks_inside_workspace_never_follow_outside_or_broken_targets` (current path is read/followed).
- Hardlink identity: blocked by current public API's path authorization then raw path open; removed from executable RED. TOCTOU rename/replace remains documented only.
- Fixture review: all canaries disposable; no host file bytes read. Failure output contains fixture paths only, no content canaries.
- Resource check: `vm_stat` before handoff showed 10,214 free pages; no Cargo/rustc survivors after test.
- Final RED SHA-256: `ed524ec1a239e8403c76d133e6929683a5a751f758f662b91b0a8d5a2b687d77`.
- Frozen existing APP-012 SHA-256: `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- `git diff --check`: PASS.
- Required verifier command:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_protected_path_red -- --test-threads=1
```

- Cancellation verification owns execution, compile assessment, RED output, and freeze hash.
