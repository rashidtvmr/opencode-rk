# OC2 UI parity capture

Exact commands to capture styled cells from three sources and compare them. This
document describes the harness that already exists at `tools/ui_parity/`; it does
not define a second capture path.

- Driver: `tools/ui_parity/ui_parity.py` (`--record`, `--check`, `--filter`,
  `--golden-dir`, `--bin`, `--renderer-profile`, `--evidence-bundle`).
- oc2 adapter: `tools/ui_parity/renderers/oc2_pty.py` (`--bin`, `--min-ms`,
  `--quiet-ms`, `--max-ms`). Launches the real binary with `--native` under an
  isolated PTY, feeds the ANSI stream through
  `tools/ui_parity/renderers/vt_screen.py`, emits `oc2.ui-parity.screen/v1`.
- Goldens: `tests/fixtures/ui-parity/goldens/<fixture-id>.golden.json`.
- Cases: `tests/fixtures/ui-parity/cases/*.json`; index in
  `tests/fixtures/ui-parity/manifest.json`.

Set the environment once (the vendored OpenTUI dylib must resolve):

```sh
export DYLD_LIBRARY_PATH=/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/opentui-bridge/native/lib/aarch64-apple-darwin
cd /Users/mymac/Projects/opencode-rk-recovery-20261005
```

## (a) Capture OC2 styled cells (the Rust app under test)

Build the exact revision first, then record against that binary. The product
SHA-256 travels inside every golden, so a rebuild invalidates old goldens.

```sh
cargo build --release -p opencode-rk-cli --bin oc2
python3 tools/ui_parity/ui_parity.py --record --filter 'par-static-*' --force \
  --bin target/release/oc2
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*' \
  --bin target/release/oc2
```

Expected: `recorded` per fixture, then `pass` per fixture, exit 0.

Single-fixture round trip at 120x40, writing to an external golden dir (no repo
edit):

```sh
TMPG=$(mktemp -d)
python3 tools/ui_parity/ui_parity.py --record --filter 'par-static-home-80x24-dark' \
  --bin target/release/oc2 --golden-dir "$TMPG"
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-home-80x24-dark' \
  --bin target/release/oc2 --golden-dir "$TMPG"
```

One non-home view (the command palette, opened with Ctrl-P):

```sh
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-palette-120x40-dark' \
  --bin target/release/oc2
```

Isolation is built into `oc2_pty.isolated_env()`: it clears `OPENCODE_RK_HOME`,
`OPENCODE_CONFIG*`, `XDG_*` and repoints `HOME`, `XDG_CONFIG_HOME`,
`XDG_DATA_HOME`, `XDG_STATE_HOME`, `OPENCODE_RK_HOME`, `TMPDIR` at a disposable
temp tree. The user's database (`~/.local/share/opencode-rk`) and daemon are never
touched. Captures are cell/style only; there is no pixel/raster path.

## (b) Capture the installed terminal (the `oc2` on `PATH`)

Identical procedure with the installed binary path. Use `--bin` to point at it.

```sh
INSTALLED=/Users/mymac/.local/bin/oc2
"$INSTALLED" --version            # records which build is being captured
shasum -a 256 "$INSTALLED"
TMPG=$(mktemp -d)
python3 tools/ui_parity/ui_parity.py --record --filter 'par-static-*' \
  --bin "$INSTALLED" --golden-dir "$TMPG" --force
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*' \
  --bin "$INSTALLED" --golden-dir "$TMPG"
```

Attach the installed capture to a SHA-bound evidence bundle (PAR-22):

```sh
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*' \
  --bin "$INSTALLED" --golden-dir "$TMPG" \
  --evidence-bundle "$TMPG/evidence.json" \
  --evidence-command "python3 tools/ui_parity/renderers/oc2_pty.py --bin $INSTALLED" \
  --installed-capture "$TMPG/par-static-home-80x24-dark.golden.json"
```

The bundle records `rust_sha`, every fixture/golden SHA-256, the reproduction
commands, per-fixture outcomes and the hash of each installed capture.

Notes:

- macOS has no `timeout`/`gtimeout`; the adapter bounds work internally with
  `select(...)` deadlines plus `--min-ms`/`--quiet-ms`/`--max-ms` and always reaps
  the child (`SIGINT`, then `SIGKILL` to the process group). Do not wrap in
  `timeout`.
- The footer cwd is redacted to `<WORKDIR>` at capture time
  (`redact_workdir`, recorded as `capture.normalized: ["workdir_path"]`). When the
  path is clipped to a truncated form (for example `...gyc000`), redaction cannot
  match the token and the raw fragment survives in that row. Prefer viewports
  where the footer shows the full path, or a view where the cwd is not visible.
- The PTY exposes no pointer/focus channel, so `hit_targets` is `[]` and
  `focus.value` is `null`; compare those members only through the deterministic
  `static` renderer.

## (c) Compare against upstream OpenCode V2

Upstream checkout: `/Users/mymac/Projects/tmp/opencode`, HEAD
`450e7b92412c780a7b5a75226bb8d49aaca24e22` (matches the parity-kit pin in
`tests/fixtures/ui-parity/manifest.json`).

Capture path (once the blocker below is cleared): run the same fixture through a
TypeScript/OpenTUI adapter that emits `oc2.ui-parity.screen/v1`, record the
upstream golden with `--golden-dir upstream-goldens`, then compare:

```sh
TMPG=$(mktemp -d)
python3 tools/ui_parity/ui_parity.py --record --filter 'par-static-*' \
  --renderer-profile <upstream-profile> --golden-dir "$TMPG/upstream"
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*' \
  --bin target/release/oc2 --golden-dir "$TMPG/upstream" --allow-pin-drift
```

The `--check` comparator (ui_parity.py) enforces viewport, default style, fixture
class, pins, product version/SHA-256, adapter and theme, and - when the fixture
sets `compare` - cursor, hit targets, focus, rows and actions.

### Blocker for (c): OpenTUI version mismatch

No upstream capture can be produced on this host yet:

- `/Users/mymac/Projects/tmp/opencode/packages/tui/node_modules/@opentui/core` is
  version **0.4.5**, but the kit and the manifest pin OpenTUI **0.6.8**
  (`opentui_commit` `8d6065a3e60039d16c6ee697692dcfbc46d3c786`,
  `opentui_version` `0.6.8` in `tests/fixtures/ui-parity/manifest.json`).
- `bun test` in the upstream tree crashes with
  `SyntaxError: Export named 'NativeError' not found` until the deps are
  normalized to the pinned revision (same root cause recorded for PAR-03).
- Because the renderer differs, any upstream frame captured at 0.4.5 is not a
  valid parity basis for the pinned 0.6.8 contract, so no byte-exact upstream
  comparison is possible today.

Unblock: install OpenTUI 0.6.8 `8d6065a3` in the upstream tree (reconcile
`bun.lock`), then build a TypeScript adapter that constructs
`createCliRenderer({ width, height, stdout, bufferedOutput: "memory" })`,
mounts the same route `packages/tui/src/app.tsx` renders, and dumps the memory
frame as `oc2.ui-parity.screen/v1`. `CliRenderer.dumpBuffers()` /
`dumpOutputBuffer()` / `dumpHitGrid()` are the documented dumps
(`packages/tui/node_modules/@opentui/core/renderer.d.ts`).

Until then, "diff against upstream" means: record the installed/OC2 golden here
and hold it as the OC2-side expectation; the upstream side stays an open item.

## Current local coverage

- `9` `par-static-*` fixtures: `5` home views (80x24 dark/light, 100x30, 60x20,
  80x10) and `4` non-home views at 120x40 dark, all listed in `manifest.json`:
  - `par-static-commands-120x40-dark` - Ctrl-K Commands page.
  - `par-static-help-120x40-dark` - `/help` Help page.
  - `par-static-agents-120x40-dark` - `/agents` subagent roster.
  - `par-static-dialog-model-120x40-dark` - Ctrl-P palette -> Select-model
    dialog.
- `--check` compares every case that has a golden; `ui_parity_harness --check`
  runs them through a binary built at the lane HEAD.
- Mixed-version captures are rejected: golden pins and the product binary SHA-256
  must match the manifest/renderer, or `--allow-pin-drift` must be given. The
  cargo harness test passes `--allow-pin-drift` because `cargo test` links a
  debug binary while goldens record the release profile; cells, theme, viewport
  and class comparisons stay strict.

### Views that are not deterministically capturable here

- **Sidebar-open**: at the PTY adapter's reachable states the sidebar does not
  render rows. `sidebar_visible()` (`crates/cli/src/native_view.rs:2818`) is
  `session_route && !parent_session && rows >= 2 && (sidebar_open || (!sidebar_hidden && sidebar_wide))`,
  and toggling it (`toggle_sidebar`, `native_view.rs:2687`) depends on live
  session state (`session_route`, `parent_session`) that the PTY replay path does
  not let a fixture set (`supports_state: false` in `manifest.json`). Captures at
  120x40 and 160x50 with `/sidebar` and Ctrl-X then `b` produce the home frame
  unchanged, so the case is NOT_LANDABLE
  (`crates/cli/src/native_view.rs:2818`, state not settable via the `oc2-pty`
  adapter).
