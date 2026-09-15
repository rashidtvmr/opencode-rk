# HR-SLICE-02 - Telemetry report renderer (`stats` output)

Status: PROPOSED. Kind: product. Runtime optional: False.
Mandatory for full declared release: no (lean-tooling proposal).
Requirements: none (new user requirement, headroom telemetry display).
Dependencies: none (reads caller-supplied snapshot only; no store I/O).
Test obligations: HR-RPT-T01, HR-RPT-T02, HR-RPT-T03, HR-RPT-T04, HR-RPT-T05.

## User-observable outcome

Bounded text renderer for the `stats` command: totals, savings, recent
entries. Exact deterministic text format. Secrets never emitted (absence
asserted, not format-only). Default-on with opt-out; opt-out prints a fixed
message and no data.

## Source evidence

- PLAN.md sections 5-6: slice template + mandatory RED/GREEN lifecycle.
- docs/TDD.md sections 2-5: contract, RED, freeze, GREEN rules.
- docs/SECURITY.md section 2: no secret logging, no inherited env, fixtures only.
- tasks/TOOL-014.md: task-card model (bounded store, byte budget, eviction).
- tasks/OPS-010.md: doctor-report precedent (diagnostics without log spelunking).

## Observable contract

- `TelemetryEntry { label: String, saved_tokens: u64 }` caller-supplied.
- `TelemetrySnapshot { total: u64, saved_tokens: u64, total_tokens: u64, recent: Vec<TelemetryEntry> }`.
- `ReportConfig { enabled: bool, max_entries: usize, max_bytes: usize }`.
- `Default`: `enabled: true, max_entries: 5, max_bytes: 8192`.
- `render(snapshot: &TelemetrySnapshot, cfg: &ReportConfig) -> String`.
- Golden format (exact, `\n` terminated, no trailing spaces):
```text
telemetry: {total} compressions, {saved} tokens saved ({pct}%)
recent:
- {label}: {saved} saved
```
- `pct = 100 * saved / max(total_tokens, 1)` (integer, no float).
- Empty store (`total == 0`): output is exactly `no telemetry recorded yet\n`.
- Disabled (`enabled == false`): output is exactly
  `telemetry disabled (opt-out set)\n`; snapshot ignored.
- Redaction first: every `label` scrubbed before format; secret bytes never
  reach output. Recent capped to `max_entries` oldest-dropped, then
  byte-capped with marker.

## Failure states

- Empty store: fixed message, no header, no recent block.
- Opt-out: fixed message, no totals, no entries, no secrets.
- Secret-like label (`sk-`, `ak-`, `bearer `, `api[_-]?key`, `-----BEGIN`):
  replaced with `[redacted]`; raw substring absent.
- Over `max_entries`: keep newest N, append `\n... [truncated {dropped} entries]`.
- Over `max_bytes`: cut at `char_boundary`, append
  `\n... [truncated {dropped} bytes]`; output `len <= max_bytes + MARKER_MAX`.
- Non-UTF8 impossible (String inputs); empty label renders as `[empty]`.

## Resource bounds

- Pure library: no I/O, no network, no clock, no env read inside `render`
  (caller maps env to `enabled`). Time O(recent), memory O(max_bytes).
- Deterministic: same snapshot + cfg => byte-identical output.

## Suggested module boundary

- Proposed owner: `crates/headroom/src/report.rs` (new module `report`).
- Integrator wires `pub use` / CLI call site; worker does not edit shared
  `lib.rs`, `Cargo.toml`, schemas. Additive only.

## Frozen test obligations (exact asserts)

- HR-RPT-T01 golden output: fixed 2-entry fixture (total=3, saved=300,
  total_tokens=1000): `assert_eq!(render(..), "telemetry: 3 compressions, 300 tokens saved (30%)\nrecent:\n- gzip: 200 saved\n- rtk: 100 saved\n")`.
- HR-RPT-T02 empty store: `total=0, recent=[]`:
  `assert_eq!(render(..), "no telemetry recorded yet\n")`.
- HR-RPT-T03 redaction absence: labels `sk-live-abc123`, `api_key=ZZZ`,
  `-----BEGIN PRIVATE KEY-----`: for each secret `s`:
  `assert!(!out.contains(s))`, `assert!(out.contains("[redacted]"))`.
- HR-RPT-T04 truncation marker: 10 entries, `max_entries=5`:
  `assert_eq!(out.lines().filter(|l| l.starts_with("- ")).count(), 5)`,
  `assert!(out.ends_with("... [truncated 5 entries]\n"))`.
- HR-RPT-T05 opt-out message: `enabled=false` with non-empty snapshot:
  `assert_eq!(render(..), "telemetry disabled (opt-out set)\n")`.

## TDD steps

1. Inspect PLAN.md 5-6, docs/TDD.md 2-5, SECURITY.md 2, TOOL-014 card.
2. Define contract/failures/bounds (above); open discovery for gaps.
3. Author RED: 5 tests compile, fail on missing `headroom::report::render`.
4. Freeze test hash + command manifest; controller verifies on disk.
5. Implement minimum native Rust; GREEN; refactor; rerun full suite.
6. Regressions: pct div-zero, char-boundary cut, `[empty]` label, determinism.
7. Submit evidence + patch, never acceptance.

## Verification

- `cargo test -p headroom` passes HR-RPT-T01..T05 on frozen hash.
- `cargo check --workspace` clean; `cargo fmt --check` clean.
- `cargo test -p headroom -- --nocapture` shows golden text byte-identical.
