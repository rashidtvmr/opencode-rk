# HEAD-002 - Deterministic redacted session export

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-048.
Dependencies: none newly introduced by this slice.
Test obligations: HEAD-002-T01, HEAD-002-T02, HEAD-002-T03, HEAD-002-T04, HEAD-002-T05.
Ownership locks: crates/cli/src/session_export.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/cli/src/session_export.rs (new module in crate opencode-rk-cli; lib.rs wiring left to integrator).

## User-observable outcome

Callers exporting one session observe a deterministic redacted artifact: `export_json` with an explicit session id writes canonical JSON under a 16MB cap through a redacting writer, non-TTY callers without an explicit id get a typed MissingId error instead of an interactive picker, secret-shaped values are replaced with `***`, and oversize exports fail atomically with zero output bytes. No network, no picker, no ambient filesystem beyond the caller sink.

## Source evidence

- Observed upstream checkout `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb` versus plan pin `95daf90670b7c039c436c85537da5fbfe2205b41` (PLAN.md:36, sources/upstream.lock.json:9); never equated; pinned-blob reconciliation outstanding before parity claims.
- worklog/UPSTREAM-V2-web-acp-integrations.md: `cli/cmd/export.ts` session export (explicit id, JSON writer, non-TTY discipline).
- docs/research/UPSTREAM-V2-PARITY-SYNTHESIS.md section 9 (F40, class unverified): no local session-export module found; headless-run parity likewise unverified.
- Local absence: `crates/cli/src/session_export.rs` does not exist (verified by directory listing; cli src holds only main.rs).
- Local partial (not re-owned): HEAD-001 owns only headless run (no export_json, no redaction, no MissingId); crates/server/src/transcript_lane.rs covers transcript lanes, not CLI export framing or redaction.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/WEB-005.md: task-card model mirrored here. tasks/HEAD-001.md: sibling headless-run slice (this card owns only export plus redaction; HEAD-001 owns run plus renderers plus attachments).
- Classification: discovered scope under REQ-048; deliberate safer/resource-bounded deviation (explicit EXPORT_MAX_BYTES cap plus atomic no-output oversize rule plus `***` redaction; upstream states no such bounds).

## Observable contract

- `export_json(session_id: &str, store: &dyn SessionStore, out: &mut dyn ExportSink, opts: &ExportOpts) -> Result<ExportSummary, ExportError>`: session_id 1..=128 chars, explicit always; empty id with `tty: false` => `Err(MissingId)` (no picker); empty id with `tty: true` is still `Err(MissingId)` in this slice (picker owned by a later UI slice, never implied here).
- `SessionStore`/`ExportSink` are caller-supplied traits (fixture store/sink in tests); this module performs no direct fs I/O beyond the caller sink.
- Redacting JSON writer: keys matching `(?i)^(token|secret|password|api[_-]?key|authorization|bearer|session[_-]?token)$` plus values matching `(?i)^(sk-|bearer |xox[bpas]-)` are emitted as `"***"`; structure and key order otherwise preserved.
- `ExportSummary { session_id: String, bytes: u64, messages: u64 }`: bytes equals exactly the emitted length.
- Bounds as explicit public constant: `EXPORT_MAX_BYTES: usize = 16_777_216` (16 MiB); size estimated before any sink write.
- Deterministic: same store plus same id plus same opts => byte-identical output on every call; keys in first-seen order, no wall-clock, no globals.
- Suggested module boundary: `crates/cli/src/session_export.rs` owning SessionStore, ExportSink, ExportOpts, ExportSummary, ExportError, EXPORT_MAX_BYTES, export_json; shared lib.rs wiring left to integrator.
- Explicitly excluded: headless run/renderers (HEAD-001), interactive picker UI, compression, network upload, full history restore.

## State and persistence transitions

- Export lifecycle: validate id -> load session -> estimate size -> redact-write -> summary; any failure before write completion emits zero sink bytes (atomic).
- Missing session: `Err(NotFound)`; sink untouched.
- Non-TTY without explicit id: `Err(MissingId)` before any store call; store and sink untouched.
- Persistence: output lives only in the caller sink; this module retains nothing between calls.

## Failure states

- Missing explicit id (empty, non-TTY): `Err(ExportError::MissingId)`; zero store calls, zero sink bytes.
- Unknown session id: `Err(ExportError::NotFound)`; sink untouched.
- Oversize export (> 16 MiB estimated): `Err(ExportError::TooLarge)`; zero sink bytes emitted (atomic, asserted by byte-count test).
- Store failure mid-load: `Err(ExportError::StoreFailed)`; sink untouched.
- Unchanged-state guarantees: every error above leaves the sink byte-count identical to before (no partial JSON prefix).
- Secret safety: output and errors carry `***` in place of every secret-shaped value; errors carry variant names and ids only, never secret content.

## Resource bounds

- Export cap: EXPORT_MAX_BYTES bounds every export; estimated before retention/write (no unbounded queue/output).
- Id caps: session_id 1..=128 chars; message text per fixture store bounded by the same 16 MiB total.
- Owner/cancel path: caller owns the store, sink, and TTY flag; `export_json` is synchronous, spawns no thread; drop reclaims nothing hidden; no detached task, no background writer, no socket.
- Zero hidden cost: no statics, no cache; idle cost is zero.

## Test obligations (frozen)

- HEAD-002-T01 (happy path): fixture store holds session `s1` with 2 messages => `export_json("s1", ..)` writes canonical JSON, returns `{ session_id: "s1", bytes: <exact>, messages: 2 }`; bytes equal sink length.
- HEAD-002-T02 (non-TTY missing id): `export_json("", .., tty: false)` => `Err(MissingId)` with zero store calls and zero sink bytes.
- HEAD-002-T03 (oversize atomic): fixture store sized at 16 MiB+1 estimated => `Err(TooLarge)` with exactly zero sink bytes (no partial prefix).
- HEAD-002-T04 (leak scan): fixture values include `sk-abc`, `bearer xyz`, `password` keys => output contains zero of those bytes, contains `"***"` in their place; Debug of errors likewise clean.
- HEAD-002-T05 (deterministic bytes): same export twice => byte-identical sink bytes (assert `eq!` on full vecs); key order stable across runs.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, worklogs and synthesis evidence above (done).
2. Contract: defined above.
3. Author tests HEAD-002-T01..T05; establish compiling RED (fail: no session_export module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust redacted deterministic session export.
6. GREEN, refactor, rerun; negative tests (missing-id-no-store-call, oversize-atomic, leak-scan, determinism).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/HEAD-002.md
cargo test -p opencode-rk-cli session_export
cargo check --workspace
```

## Ownership note

Headless run stays with HEAD-001; transcript lanes stay with transcript_lane; ACP framing stays with ACP-001. This card specifies the export plus redaction contract only. DISC-003 remains IN PROGRESS / NOT ACCEPTED.
