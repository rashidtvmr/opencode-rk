# OC2 UI parity harness (P01)

Stdlib-only Python harness that turns "the UI looks right" into a checkable,
versioned comparison: it renders a fixture through an injectable renderer,
captures styled cells, cursor, focus and semantic action traces, and reports a
structured diff against a recorded golden.

## Exact commands

Run from the repository root:

```sh
# Validate the specification kit (223 obligations, PAR-01..PAR-24, 196 fixtures)
python3 tools/ui_parity/ui_parity.py --validate-kit

# List all 201 fixtures (196 kit + 5 local) and whether a golden exists
python3 tools/ui_parity/ui_parity.py --list

# Check every fixture that has a golden (kit fixtures without one are reported
# as missing_golden, not failures)
python3 tools/ui_parity/ui_parity.py --check

# Check only the five recorded static fixtures
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*'

# Same, with a machine-readable report
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*' \
  --json-report /tmp/oc2-ui-parity-report.json

# Re-record the local static fixtures (refuses to overwrite without --force)
python3 tools/ui_parity/ui_parity.py --record --filter 'par-static-*' --force

# Comparator negative-control self-test
python3 tools/ui_parity/selftest.py
```

Exit codes: `0` all checked fixtures passed, `1` a comparison failed, `2`
usage/configuration/renderer error.

## What is recorded

The five local fixtures in `tests/fixtures/ui-parity/cases/` are captured from
the **real OC2 native TUI** (`target/release/oc2 --native`) running under an
isolated PTY at 80x24 dark, 80x24 light, 100x30, 60x20 and 80x10. Each golden
stores:

- viewport, default foreground/background, per-row text plus merged style spans
  (`fg`, `bg`, `attrs`), including explicit wide-cell continuation columns;
- terminal cursor row/column/visibility;
- the focus object reported by the adapter;
- the semantic action trace;
- product pins: binary version + SHA-256, upstream commit, OpenTUI commit,
  reference runtime and default theme.

Goldens are deterministic JSON (`sort_keys`, one-space indent). The
environment-specific working-directory path in the footer is redacted to
`<WORKDIR>` at capture time and recorded as `capture.normalized`.

## Renderer protocol

A renderer is an executable. The harness sends one JSON request on stdin:

```json
{
  "schema": "oc2.ui-parity.request/v1",
  "fixture": { "...": "kit or local fixture case" },
  "viewport": {"cols": 80, "rows": 24},
  "theme": {"name": "opencode", "mode": "dark"},
  "pins": {"upstream_commit": "450e...", "...": "..."},
  "environment": {"TERM": "xterm-256color"},
  "renderer": {"name": "oc2-pty", "command": ["..."]}
}
```

The renderer writes one JSON screen document on stdout:

```json
{
  "schema": "oc2.ui-parity.screen/v1",
  "fixture_id": "par-static-home-80x24-dark",
  "viewport": {"cols": 80, "rows": 24},
  "default_style": {"fg": "#ffffff", "bg": "#000000"},
  "theme": {"name": "opencode", "mode": "dark"},
  "cursor": {"row": 14, "col": 6, "visible": true},
  "focus": {"value": null, "source": "..."},
  "rows": [{"text": "Chat ...", "w": 80, "spans": [{"c": 0, "n": 35, "fg": "#fab283", "bg": "#141414", "attrs": ["bold"]}]}],
  "actions": [],
  "environment": {"adapter": "oc2-pty", "product": {"version": "...", "sha256": "..."}}
}
```

`rows` must contain exactly `viewport.rows` entries; `w` is the cell width of the
row. Text may omit trailing default-styled spaces; the comparator pads to `w`.
A wide character occupies one leading cell plus one continuation column.

Configured profiles live in `tests/fixtures/ui-parity/manifest.json`:

- `oc2-pty` (default): native styled capture from a real binary.
- `oc2-once`: `oc2 --once` plain-text snapshot; marked `unstyled`, so it cannot
  silently satisfy a styled golden. Useful when the OpenTUI dylib is missing.
- `static`: deterministic JSON renderer used by `selftest.py`.

Use `--renderer-command '["python3","path/to/renderer.py"]'` to inject another
renderer, or `--renderer-profile NAME` to select a manifest profile.

## Golden format

`tests/fixtures/ui-parity/goldens/<fixture-id>.golden.json`:

```json
{
  "schema": "oc2.ui-parity.golden/v1",
  "fixture_id": "par-static-home-80x24-dark",
  "capture": {"pins": {}, "product": {"sha256": "..."}, "normalized": ["workdir_path"]},
  "viewport": {"cols": 80, "rows": 24},
  "default_style": {"fg": "#ffffff", "bg": "#000000"},
  "cursor": {"row": 14, "col": 6, "visible": true},
  "focus": {"value": null, "source": "terminal_has_no_focus_channel"},
  "rows": [{"text": "Chat ...", "w": 80, "spans": [{"c": 0, "n": 35, "fg": "#fab283", "bg": "#141414", "attrs": ["bold", "underline"]}]}],
  "actions": []
}
```

Styles are merged into runs instead of repeating per cell, which keeps an 80x24
frame around 12 KB while remaining readable and diffable.

## Comparison and pins

`--check` compares viewport, default style, theme, cursor (when visible), focus,
action trace and every cell's character/foreground/background/attributes.
`--max-diffs` caps the emitted diff list; the summary always counts all diffs.

Mixed-version captures are rejected: golden pins and the product binary SHA-256
must match the manifest/actual renderer. `--allow-pin-drift` downgrades those to
warnings. Because the binary hash participates, rebuild `oc2` and re-record after
an intentional UI change.

## Current coverage and limits

- `201` fixtures are loaded: `196` kit specifications + `5` local captures.
- `5` fixtures can be checked right now; all five pass and reference 14
  checklist obligations (PAR-01/03/04/05/14 plus layout/theme/composer/sidebar
  items). `--json-report` records the exact obligation coverage.
- The other `196` kit fixtures are `golden_status: not_captured`; they need
  state/event adapters before they can be recorded, so they report
  `missing_golden` and are not claimed as passing.
- The native adapter captures the **Rust OC2** production renderer only. An
  upstream TypeScript/OpenTUI capture adapter has not been built, so PAR-03 is
  only partially satisfied (Rust side done, upstream differential side open).
- The terminal exposes no focus channel; native goldens record
  `focus.value = null` with an explicit source. Focus comparison is exercised by
  the deterministic `static` renderer and its negative control.
- Cursor parity records terminal cursor state. The native app paints its own
  block cursor, which is encoded in cell styles rather than the terminal cursor.
- Goldens were recorded from the prebuilt `target/release/oc2` present in this
  workspace (`sha256 6fd34d77...`), not a fresh build of `main-v2`. Re-record
  after building the exact candidate under test.

## Self-test

`tools/ui_parity/selftest.py` records a synthetic fixture through the injected
`static` renderer and proves the comparator fails on:

1. one-cell foreground color change (exactly one `fg` diff),
2. cursor movement (`cursor`),
3. width truncation (`viewport` + `row_width`),
4. focus movement (`focus`),
5. an extra semantic action (`action_trace`),
6. mixed product pins (`pin_mismatch`).

It also proves deterministic re-recording, wide-cell continuation handling and
that the 223/196 kit catalog loads.
