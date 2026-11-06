# PAR-06 — Capture environment baseline and terminal-specific isolation

**Recorded:** 2026-11-06  
**Source revision:** `2c477c8afb06d6b34d53353c7686bc4c40d67c04`  
**Upstream oracle:** `/Users/mymac/Projects/tmp/opencode` at
`450e7b92412c780a7b5a75226bb8d49aaca24e22`  
**OpenTUI kit pin:** `8d6065a3e60039d16c6ee697692dcfbc46d3c786`, version 0.6.8

This is the reproducibility baseline for cell captures. It is intentionally
not a pixel/font claim: the PTY capture protocol records terminal variables and
cell geometry, not the host terminal emulator's raster font.

## Host baseline observed

| Field | Value | Evidence/method |
| --- | --- | --- |
| OS | macOS 27.0, build 26A428 | `sw_vers` on the capture host |
| Kernel/architecture | Darwin 27.0.0, arm64 (`arm64`) | `uname -a`, `uname -m` |
| Rust toolchain | `rustc 1.99.1 (58cf8f9c 2026-10-13)` | `rustc --version` |
| Cargo | `cargo 1.99.1 (5f94df478 2026-08-27)` | `cargo --version` |
| TERM | `xterm-256color` | shell observation; adapter forces same value |
| COLORTERM | `truecolor` | shell observation; adapter forces same value |
| Locale | observed `LANG=en_IN.UTF-8`; isolated adapter sets `LANG=C.UTF-8`, `LC_ALL=C.UTF-8` | `tools/ui_parity/renderers/oc2_pty.py:75-91` |
| Time zone | adapter sets `TZ=UTC` | `oc2_pty.py:86-91` |
| Viewport | fixture-defined; adapter passes `cols`/`rows` | `oc2_pty.py:293-294,367-373` |
| Native library | `crates/opentui-bridge/native/lib/aarch64-apple-darwin` via `DYLD_LIBRARY_PATH` | `docs/ui-parity/CAPTURE.md:17-22` |

## Font baseline and limitation

The host has system fonts including `.SF NS Mono` (observed at
`/System/Library/Fonts/SFNSMonoItalic.ttf`) and `.SF Compact`; however, the
PTY adapter does not select or observe a raster font. Therefore the font
identity is **UNKNOWN / not applicable to cell-schema captures**, not “SF
Mono” by assumption. `CAPTURE.md:57-61` states that the adapter captures
cells/styles rather than pixels. A pixel-capture lane must record the terminal
emulator name/version, font family, size, cell width/height, DPI/scale, and
theme before claiming PAR-06 pixel evidence.

## Deterministic adapter settings

`tools/ui_parity/renderers/oc2_pty.py:75-99` creates an isolated HOME/XDG/data/
state/TMP tree and sets `TERM=xterm-256color`, `COLORTERM=truecolor`, UTF-8
locale, and UTC. `:350-377` records adapter, product version/SHA-256, terminal
variables, viewport, platform, machine, executable, and normalization rules
inside every screen document. The schema and required viewport/row shape are
also documented in `tools/ui_parity/README.md:71-100`.

## Terminal-specific isolation cases

* **PTY vs emulator:** the PTY has no pointer/focus channel; the adapter emits
  `hit_targets=[]` and `focus.value=null`, as documented by `CAPTURE.md:102-104`.
  These fields must not be compared as if they came from a mouse-capable
  terminal.
* **Working directory:** the adapter redacts the work directory to
  `<WORKDIR>` (`CAPTURE.md:97-101`); fixtures where clipping prevents that
  replacement are not portable and should be excluded or resized.
* **OpenTUI version:** upstream capture is blocked while the checkout has
  OpenTUI 0.4.5 rather than pinned 0.6.8; `CAPTURE.md:128-143` records the
  exact mismatch and does not treat 0.4.5 frames as valid oracle evidence.
* **Geometry:** compare only at identical fixture viewport dimensions. The
  capture harness requires exactly `viewport.rows` rows
  (`tools/ui_parity/README.md:110`) and records `cols`/`rows` in the frame.

## Reproduction commands

```sh
export DYLD_LIBRARY_PATH=/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/opentui-bridge/native/lib/aarch64-apple-darwin
cargo build --release -p opencode-rk-cli --bin oc2
python3 tools/ui_parity/ui_parity.py --record --filter 'par-static-*' --force --bin target/release/oc2
python3 tools/ui_parity/ui_parity.py --check --filter 'par-static-*' --bin target/release/oc2
```

These commands are the existing capture path (`CAPTURE.md:24-37`); they do not
produce pixel/font acceptance evidence.
