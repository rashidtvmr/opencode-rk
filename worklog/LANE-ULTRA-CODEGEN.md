# LANE-ULTRA-CODEGEN scratchpad

## Claim
- Task: ULTRA codegen lifecycle state machine (roadmap 3.2)
- Session: ses_worker_ultra
- Status: completed

## Source evidence
- crates/agents/src/ultra_codegen.rs (NEW, 220 lines)
- crates/agents/tests/ultra_codegen.rs (NEW, 291 lines, 20 tests)

## RED phase
- RED test sha256: 306d78cfb93b53aa968cbcca43c8da9990fa33c265a14ad3fbd94f3162dd6366
- Tests: 20 written, 20 fail (source file absent as expected)

## GREEN phase
- GREEN impl sha256: (see commit)
- Tests: 20/20 pass
- Gate: CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 300 rtk cargo test -p opencode-rk-agents --test ultra_codegen

## State machine design
States: Requested → DraftEmitted(source_hash) → CompileRequested → Compiled | CompileFailed(attempt n) → Executed(bounds) | Denied(reason) → Fallback(after N failures)

Denial taxonomy:
- UnsafeWithoutApproval
- ForbiddenApiHit { pattern }
- BoundsExceeded { bytes, limit }
- BuildFailureN { attempts }

Denylist (const): std::process::Command, std::fs::remove_dir_all, std::fs::remove_file, std::env::set_var, std::env::remove_var, unsafe, transmute, ptr::read, ptr::write

Cache: content-addressed via hash of generated source; cache hit skips compile.

## Integrator follow-up notes
1. Add `pub mod ultra_codegen;` to crates/agents/src/lib.rs
2. The sandboxed executor module (separate lane) should compose this state machine:
   - Call `emit_draft()` with LLM-generated source
   - Check denylist automatically via emit_draft
   - On DraftEmitted, check cache before calling external compile
   - On Compiled, call execute() with sandbox bounds
   - On Denied/Fallback, report to user
3. No cargo/process execution in this module — pure state machine only
4. The `generate_manifest()` method emits Cargo.toml content with `#![forbid(unsafe_code)]` — integrator should write this to a temp crate for compilation
5. Future: wire `check_denylist()` to runtime policy for dynamic denylist updates

## Decisions
- Simple hash (DefaultHasher) for test determinism; real impl would use sha2 crate
- `request_compile()` accepts DraftEmitted and CompileFailed states for retry loops
- `compile_failure()` from CompileFailed state allows re-request without explicit state reset
- Cache uses HashSet<String> of known hashes for O(1) lookup

## Remaining unknowns
- Actual binary_path handling (sandboxed executor concern)
- Dynamic denylist updates (future policy integration)
- Source hash algorithm upgrade to SHA-256 (currently uses DefaultHasher for determinism)
