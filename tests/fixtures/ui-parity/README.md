# UI parity fixtures and goldens

Owned by UI-parity package P01. The catalog is split in two:

- `docs/ui-parity/opencode-ui-parity-kit/fixtures.json` — the supplied 196
  specification cases. All are still `golden_status: not_captured`; the harness
  reports them as `missing_golden`.
- `cases/*.json` + `goldens/*.golden.json` — five local, deterministic captures
  from the real OC2 native TUI, listed by `manifest.json`.

## The five recorded fixtures

| Fixture | Viewport | Theme | Purpose |
|---|---|---|---|
| `par-static-home-80x24-dark` | 80x24 | dark | baseline home shell |
| `par-static-home-80x24-light` | 80x24 | light | theme resolution via `OPENCODE_CLI_CONFIG_CONTENT` |
| `par-static-home-100x30-dark` | 100x30 | dark | wider layout |
| `par-static-home-60x20-dark` | 60x20 | dark | narrow viewport |
| `par-static-home-80x10-dark` | 80x10 | dark | vertical clipping |

Each case has empty `events`/`state_overrides`, so no state adapter is needed.
The native adapter runs `target/release/oc2 --native` in a disposable HOME /
`OPENCODE_RK_HOME` / cwd, reads the ANSI stream through a PTY, and redacts the
environment-specific workdir path to `<WORKDIR>`.

## Manifest

`manifest.json` holds:

- `pins`: upstream commit `450e7b92...`, OpenTUI `8d6065a3...`, runtime pins and
  the default theme; copied into every golden and checked on `--check`;
- `renderers`: `oc2-pty` (default, styled), `oc2-once` (plain fallback) and
  `static` (self-test);
- `fixtures`: the local case files.

`oc2_bin` defaults to `{REPO}/target/release/oc2`; override with `--bin` or
`OC2_BIN`.

## Commands

```sh
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*'
python3 tools/ui_parity/ui_parity.py --record --filter 'par-static-*' --force
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*' \
  --json-report /tmp/oc2-ui-parity-report.json
```

## Case format

A local case is a kit-compatible object plus an optional `environment` map:

```json
{
  "id": "par-static-home-80x24-dark",
  "purpose": "...",
  "viewport": {"columns": 80, "rows": 24},
  "theme": {"name": "opencode", "mode": "dark", "new_designs": false},
  "settings": {},
  "state_overrides": {},
  "events": [],
  "clock_ms": 0,
  "expected_rules": ["..."],
  "compare": ["styled_cells", "cursor", "layout_and_clipping", "focus", "semantic_actions"],
  "environment": {"TERM": "xterm-256color", "COLORTERM": "truecolor"}
}
```

Supported environment keys: `inline_cli_config` (sets
`OPENCODE_CLI_CONFIG_CONTENT`) and `env` (extra string variables). The adapter
always overrides `HOME`, `XDG_*`, `OPENCODE_RK_HOME` and `TMPDIR`-dependent cwd
with an isolated temp tree.

## Re-recording after an intentional UI change

1. Build the exact candidate: `cargo build --release -p opencode-rk-cli --features native`.
2. Run `--check`; inspect the JSON report and confirm every diff is intended.
3. Run `--record --filter 'par-static-*' --force`.
4. Review the golden diff (rows/spans are readable) and commit the new binary
   SHA pin with the code change.

Never hand-edit a golden to make a check pass. Re-record from the real renderer.
