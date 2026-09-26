# TUI-011 NATIVE COMPILE FIX — worklog

## 1. Task identity
- Task ID: TUI-011-NATIVE-COMPILE-FIX (repair child of TUI-011)
- Task type: one-file Rust implementation patch
- Role: implementer/finisher, NOT verifier
- Assigned route: vyce-deepseek-v41 (verified against allowlist)

## 2. Source evidence
- Worktree: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/impl-tui011-native-compile
- Branch: lane/TUI-011-NATIVE-COMPILE-FIX at base 5c5f03e13a659a947505d470a9e7fbee540f328d
- Owned file: crates/cli/src/tui_entry.rs
- Error site: crates/cli/src/tui_entry.rs:491 (in `native_page_lines`, `#[cfg(feature = "native")]`)
- Error: E0308 mismatched types — `lines.push(&str)` on `Vec<String>`
  - `lines` is `Vec<String>` (inferred from `lines.push(title)` at line :467 where `title: String`)
  - Line 491 (at base 5c5f03e): `lines.push("Enter send \u{b7} ...")`
  - compiler suggestion: add `.to_string()`

## 3. Observable contract
- `oc2` binary must build with `--features native` using the genuine arm64 dylib.
- One-line real fix: append `.to_string()` to the `&str` literal at line 491.
- No stub, no placeholder; preserves all behavioral semantics of `native_page_lines`.

## 4. Fixture
- Path: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/mac-opentui-preserved-4954312/packages/native/lib/aarch64-macos/libopentui.dylib
- SHA256: 5e0265d4b053004fe78974562561e10e46567cca96697f100e3288653163330e
- Staged into worktree at `crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib` (NOT committed to product repo).
- Genuine arm64 dylib is currently staged UNTRACKED in this disposable worktree; DO NOT stage/commit/delete it.

## 5. Build script gate
- `crates/opentui-bridge/build.rs:58` panics if libopentui artifact missing for target triple.
- Host triple: `aarch64-apple-darwin` (confirmed via `rustc -vV`).
- Dylib staged at `crates/opentui-bridge/native/lib/aarch64-apple-darwin/libopentui.dylib`.

## 6. Diagnostic build result
- Command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo build --locked -p opencode-rk-cli --bin oc2 --features native`
- Result: exit 0. `Finished dev` (458 pre-existing warnings, no errors).
- Binary at `target/debug/oc2`: Mach-O 64-bit executable arm64, 45,447,208 bytes.
- The `.to_string()` fix on line 491 is the sole product-code change and resolves the E0308.

## 7. Frozen RED test
- File: `tests/e2e/native_compile.rs` at base 5c5f03e.
- SHA256 (frozen): `3bf19f05ebfcc8258e8d6416b9031ecfa66adb5b32a7527b5071906ea557fb28`
- RED behavior: E0308 at tui_entry.rs:491 causes build to fail (exit 101), so the test's exit-0/binary-exists assertions do not hold.

## 8. Read-only gate baseline (BEFORE any writes)
- `python3 tools/convergence_gate.py`: repo-wide FAIL (backlog exhaustion exit=1, total=84). Pre-existing, repo-wide, not lane-specific. Not edited per task constraints.
- `python3 tools/validate_repository.py`: FAIL (backlog exhaustion exit=1). Pre-existing, repo-wide. Not edited per task constraints.
- These guards are baseline-blocked and out of lane authority. Lane owns only `crates/cli/src/tui_entry.rs` + this scratchpad + its ledger row.

## 9. Why BLOCKED (despite diagnostic GREEN)
- The frozen RED test `tests/e2e/native_compile.rs` uses `Stdio::piped()` for stdout/stderr combined with a `try_wait()` polling loop in `wait_or_kill`. Per the task briefing and docs/SECURITY.md, this pattern can deadlock on GREEN: `try_wait` returns `Ok(None)` (still running) and the parent calls `child.wait()` (or `wait_with_output`) which blocks reading the piped child stdout/stderr pipe buffers that are full, while the child is blocked writing to those full pipes — a classic pipe-buffer deadlock. The test does not drain pipes in the `try_wait` loop. Running the frozen test on GREEN would risk this deadlock and produce unreliable results.
- The task explicitly forbids running the frozen RED test ("no frozen RED run because its Stdio::piped streams are not drained until after try_wait loop and can deadlock on GREEN").
- The implementer is NOT the verifier (AGENTS.md section 9 / docs/TDD.md section 6). The frozen test must be run by an independent verifier on the exact integrated revision.
- Per AGENTS.md and the task briefing: "confirm .to_string() fix, update scratchpad honestly (currently stale 'fix pending'), set own ledger status BLOCKED with exact reason: frozen test unsafe pending trusted review and guard baseline, despite diagnostic direct native build GREEN; NEVER completed/accepted parent."

## 10. Status
- Claimed via `cc.claim` (session ses_f22744358ffeaoWa02madmxm7M).
- RED reproduced: E0308 at tui_entry.rs:491 (confirmed at base 5c5f03e).
- Fix applied: `.to_string()` appended to line 491 literal in working tree.
- Diagnostic native build: GREEN (exit 0, binary produced).
- Frozen test NOT run: unsafe deadlock risk per task forbiddance; implementer is not verifier.
- Ledger status: BLOCKED — frozen test unsafe pending trusted independent review and repo-wide guard baseline, despite direct native build GREEN.
- This is NOT completed/accepted parent. Parent TUI-011 remains blocked on rpath/cross-arch/SBOM/signing.

## 11. Decisions
- D1: One-line fix `.to_string()` applied — minimal, preserves semantics, matches compiler suggestion.
- D2: Do NOT run frozen test (deadlock risk + not verifier).
- D3: Do NOT commit staged genuine dylib (untracked, disposable).
- D4: Do NOT edit guard/policy/status files (out of lane authority).
- D5: Commit ONLY owned Rust file + this worklog + own ledger row.
- D6: Push branch without force.

## 12. Remaining unknowns
- U1: Whether the frozen test will be redesigned to drain pipes (e.g. async reader thread) before independent verification.
- U2: Parent TUI-011 rpath/cross-arch/SBOM/signing blockers remain open (out of scope for this repair lane).

## 13. Commands run
1. `rtk git log --oneline -10` — confirmed HEAD 5c5f03e, branch lane/TUI-011-NATIVE-COMPILE-FIX.
2. `git hash-object tests/e2e/native_compile.rs` → `a58a06f...` (blob); `sha256sum tests/e2e/native_compile.rs` → `3bf19f05...` (frozen hash matches).
3. `rtk rustc -vV` → host `aarch64-apple-darwin`.
4. `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo build --locked -p opencode-rk-cli --bin oc2 --features native` → exit 0, binary at `target/debug/oc2`.
5. `python3 tools/convergence_gate.py` → repo-wide FAIL (pre-existing).
6. `python3 tools/validate_repository.py` → FAIL (pre-existing).

## 14. Verification (implementer self-check, not acceptance)
- Diagnostic native build GREEN: confirmed exit 0, binary exists and is arm64 Mach-O.
- No test edits: frozen test untouched (SHA256 unchanged).
- Product code change: single one-line `.to_string()` on `crates/cli/src/tui_entry.rs:491`.
- Memory budget: well under 2 GiB (cached 0.08s build).
