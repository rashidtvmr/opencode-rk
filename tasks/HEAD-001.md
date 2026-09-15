# HEAD-001 - Headless run with inline and block renderers

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-048.
Dependencies: none newly introduced by this slice.
Test obligations: HEAD-001-T01, HEAD-001-T02, HEAD-001-T03, HEAD-001-T04, HEAD-001-T05.
Ownership locks: crates/cli/src/run_headless.rs only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/cli/src/run_headless.rs (new module in crate opencode-rk-cli; lib.rs wiring left to integrator).

## User-observable outcome

Callers running one headless session observe a complete prompt turn: `run(session, prompt)` streams the prompt uncompressed, renders the answer through Inline or Block renderers, converts text/file attachments into bounded message parts, spills oversize previews to a caller-supplied dir instead of growing memory, and exits with a typed code (0 ok, 1 usage/model error, 2 internal/limit error). No TTY, no interactive picker, no persistence beyond the caller spill dir.

## Source evidence

- Observed upstream checkout `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb` versus plan pin `95daf90670b7c039c436c85537da5fbfe2205b41` (PLAN.md:36, sources/upstream.lock.json:9); never equated; pinned-blob reconciliation outstanding before parity claims.
- worklog/UPSTREAM-V2-web-acp-integrations.md: `cli/cmd/run.ts` headless run (session plus prompt plus renderer selection).
- worklog/UPSTREAM-V2-core-architecture.md: `tool/truncate.ts` preview/truncation discipline (bounded preview with spill, not unbounded retention).
- docs/research/UPSTREAM-V2-PARITY-SYNTHESIS.md section 9 (F40, class unverified): no local headless-run module found; export parity likewise unverified.
- Local absence: `crates/cli/src/run_headless.rs` does not exist (verified by directory listing; cli src holds only main.rs).
- Local partial (not re-owned): HEAD-002 owns only session export (no run, no renderer, no attachment conversion); crates/server/src/turn_parts.rs covers turn part shapes, not headless run orchestration or preview spill.
- PLAN.md sections 5-6: slice independence rules, mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: lifecycle order, compiling RED, frozen hash, GREEN minimum.
- tasks/WEB-005.md: task-card model mirrored here. tasks/HEAD-002.md: sibling export slice (this card owns only run plus renderers plus attachments; HEAD-002 owns export_json plus redaction).
- Classification: discovered scope under REQ-048; deliberate safer/resource-bounded deviation (explicit 2000-line/50KB preview caps with caller spill dir; upstream states no such bound).

## Observable contract

- `run(session: &str, prompt: &RunPrompt, out: &mut dyn RunSink, opts: &RunOpts) -> RunExit`: session 1..=128 chars; prompt text 1..=65_536 chars; prompt streamed to the model port uncompressed (no gzip/deflate framing).
- `Renderer { Inline, Block }`: Inline emits answer text lines inline; Block wraps answer in one fenced block; renderer choice is caller-supplied per call.
- `Attachment { Text { name, text }, File { path } }`: names/paths 1..=512 chars, no `..`, no absolute; text max 256 KiB; file content resolved only via the caller-supplied `FilePort` over a disposable fixture dir.
- Preview rule: answers/attachments beyond PREVIEW_MAX_LINES or PREVIEW_MAX_BYTES render the head plus a `…truncated (N more bytes spilled)` marker; the full bytes land in exactly one file under the caller-supplied `spill_dir`; never retained in memory past the cap.
- `RunExit { code: 0 | 1 | 2, message: String }`: 0 ok; 1 usage/model error (bad input, model refusal); 2 internal/limit error (oversize, port failure); message carries variant text only.
- Bounds as explicit public constants: `PREVIEW_MAX_LINES: usize = 2000`, `PREVIEW_MAX_BYTES: usize = 51_200` (50 KiB).
- Deterministic: same session plus prompt plus fixture transcript => byte-identical sink bytes and spill files; no wall-clock, no globals.
- Suggested module boundary: `crates/cli/src/run_headless.rs` owning RunPrompt, RunOpts, RunSink, RunExit, Attachment, Renderer, FilePort, PREVIEW_MAX_LINES, PREVIEW_MAX_BYTES, run; shared lib.rs wiring left to integrator.
- Explicitly excluded: session export (HEAD-002), ACP framing (ACP-001), interactive TUI/picker, compression, network transport (caller model port owns FDs).

## State and persistence transitions

- Run lifecycle: idle -> streaming (prompt chunks to port) -> rendered (sink bytes plus optional spill file) -> exited with RunExit; failed runs emit no partial answer to the sink (atomic render per answer).
- Attachment conversion: Text embeds directly; File resolves via FilePort then follows the same preview/spill rule; missing file => `RunExit { code: 1 }` with nothing rendered.
- Spill: one file per oversize answer/attachment under `spill_dir`, caller-owned; success reports the spill path in the marker; failure to spill => `RunExit { code: 2 }` with nothing rendered.
- Persistence: spill files only, inside the caller-supplied dir; this module retains nothing between calls.

## Failure states

- Empty session or empty/oversize prompt: `RunExit { code: 1 }`; sink receives zero answer bytes, no spill file created.
- Path escape (`..`, absolute, empty, > 512 chars) on attachment: `RunExit { code: 1 }`; nothing rendered, fixtures unchanged.
- Oversize text attachment (> 256 KiB) or missing file: `RunExit { code: 1 }` / `RunExit { code: 1 }`; nothing rendered.
- Spill failure or model-port failure: `RunExit { code: 2 }`; sink holds no partial answer (asserted by byte-count test).
- Unchanged-state guarantees: every failing run leaves the sink byte-count and the spill dir listing identical to before except the single atomic spill file on oversize-success.
- Secret safety: exits, markers, and logs carry variant names and byte counts only, never prompt/attachment content. Tests use fixture prompts only.

## Resource bounds

- Preview caps: at most PREVIEW_MAX_LINES lines and PREVIEW_MAX_BYTES bytes rendered per answer/attachment; the remainder spills to disk under the caller dir; no unbounded retained output.
- Attachment caps: text max 256 KiB; one file per File attachment per call; no glob, no recursion.
- Owner/cancel path: caller owns session, sink, spill dir, and model/file ports; `run` is synchronous on the ports, spawns no thread; abort via the port reclaims the in-flight turn; no detached task, no background stream, no socket held after return.
- Zero hidden cost: no statics, no cache; idle cost is zero.

## Test obligations (frozen)

- HEAD-001-T01 (inline): fixture transcript answers `hello`; `run` with Inline => sink bytes equal `hello\n`, exit code 0; port observes the prompt bytes uncompressed (no gzip magic).
- HEAD-001-T02 (block): same transcript with Block => sink bytes equal one fenced block wrapping `hello`, exit code 0; byte-exact fence asserted.
- HEAD-001-T03 (attachments): Text attachment embeds; File attachment resolves fixture bytes via FilePort; `../escape` attachment => exit 1 with zero sink bytes and fixtures unchanged.
- HEAD-001-T04 (oversize spill bounded): 60 KiB answer => sink holds head plus spill marker within PREVIEW_MAX_BYTES, full bytes in exactly one spill file; sink byte-count asserted under cap.
- HEAD-001-T05 (exit codes): empty prompt => 1; port failure => 2 with zero partial sink bytes; ok => 0; each leaves prior sink bytes and spill listing unchanged on failure.

## TDD steps

1. Inspect: PLAN.md 5-6, TDD.md 2-5, worklogs and synthesis evidence above (done).
2. Contract: defined above.
3. Author tests HEAD-001-T01..T05; establish compiling RED (fail: no run_headless module).
4. Freeze test hash + command manifest.
5. Implement minimum native Rust headless run with renderers plus spill.
6. GREEN, refactor, rerun; negative tests (escape, oversize-atomic, no-partial-render, uncompressed-proof).
7. Evidence + patch; verifier decides acceptance.

## Verification

```bash
git status --short -- tasks/HEAD-001.md
cargo test -p opencode-rk-cli run_headless
cargo check --workspace
```

## Ownership note

Session export stays with HEAD-002; turn part shapes stay with turn_parts; ACP framing stays with ACP-001. This card specifies the headless run plus renderer plus attachment contract only. DISC-003 remains IN PROGRESS / NOT ACCEPTED.
