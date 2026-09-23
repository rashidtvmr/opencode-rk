#!/usr/bin/env python3
"""Deterministic lane gate for the TS-bridge -> Rust parity wave.

Scope: port the TypeScript bridge of pinned OpenTUI (commit 4954312d) to
native Rust in crates/opentui-bridge. The Zig native core is NOT ported;
Rust declares/calls it. Each lane owns exactly one file.

Usage:
    python3 tools/bridge_lane_gate.py          # check files on disk
    python3 tools/bridge_lane_gate.py --json   # JSON only

A lane is PASS only when its file exists, is non-trivial, contains every
required marker symbol, and contains no stub markers (todo!, unimplemented!,
"filled by a dedicated lane"). Test execution is NOT done here: lanes verify
their own file with a bounded single-target command and the orchestrator
re-runs everything serially after integration (8 GiB host budget).
"""
from __future__ import annotations

import argparse
import json
import pathlib

ROOT = pathlib.Path(__file__).resolve().parents[1]
BRIDGE = ROOT / "crates" / "opentui-bridge"
SRC = BRIDGE / "src"

STUB_MARKERS = ("todo!(", "unimplemented!(", "filled by a dedicated lane")

# id -> (path, min_lines, required markers)
LANES: list[dict] = [
    {"id": "BRG-FFI", "path": "src/ffi.rs", "min_lines": 60,
     "markers": ["pub enum FfiType", "pub struct FfiFunction", "Pointer", "pub struct CallbackHandle"]},
    {"id": "BRG-RUNTIME", "path": "src/runtime.rs", "min_lines": 80,
     "markers": ["pub enum Runtime", "pub fn detect", "pub struct AssetPath", "pub fn resolve"]},
    {"id": "BRG-TYPES", "path": "src/render_context.rs", "min_lines": 100,
     "markers": ["pub struct CliRendererConfig", "pub struct TerminalCapabilities", "pub enum WidthMethod",
                 "pub trait RenderContext", "pub enum CliRenderEvent"]},
    {"id": "BRG-KEYS", "path": "src/input.rs", "min_lines": 120,
     "markers": ["pub struct ParsedKey", "pub fn parse_keypress", "pub fn parse_kitty_keypress",
                 "pub struct KeyHandler", "pub enum KeyDecision"]},
    {"id": "BRG-MOUSE", "path": "src/mouse.rs", "min_lines": 80,
     "markers": ["pub struct RawMouseEvent", "pub enum MouseEventType", "pub fn parse_mouse_event",
                 "pub struct ScrollAccel"]},
    {"id": "BRG-STDIN", "path": "src/stdin_parser.rs", "min_lines": 120,
     "markers": ["pub struct StdinParser", "pub fn push", "pub fn drain", "pub enum StdinEvent",
                 "pub struct ProtocolContext"]},
    {"id": "BRG-CAPS", "path": "src/capabilities.rs", "min_lines": 100,
     "markers": ["pub struct CapabilityProbe", "pub enum CapabilityState", "pub fn process_response",
                 "pub struct TerminalPalette", "TIMEOUT"]},
    {"id": "BRG-BUFFER", "path": "src/buffer.rs", "min_lines": 150,
     "markers": ["pub struct Cell", "pub fn set_cell", "pub fn draw_text", "pub fn fill_rect",
                 "pub fn push_scissor_rect", "pub fn push_opacity", "pub fn draw_grid"]},
    {"id": "BRG-TEXTBUF", "path": "src/text.rs", "min_lines": 120,
     "markers": ["pub struct TextBuffer", "pub fn set_text", "pub fn set_styled_text",
                 "pub struct StyledText", "pub struct TextBufferView", "pub fn measure_for_dimensions"]},
    {"id": "BRG-TREE", "path": "src/renderable.rs", "min_lines": 150,
     "markers": ["pub struct RenderNode", "pub enum LifecyclePass", "pub fn insert_child",
                 "pub fn remove_child", "pub fn render_list", "pub fn validate"]},
    {"id": "BRG-LAYOUT", "path": "src/layout.rs", "min_lines": 120,
     "markers": ["pub struct LayoutNode", "pub fn calculate_layout", "pub fn set_measure_func",
                 "pub enum FlexDirection", "pub struct LayoutRect"]},
    {"id": "BRG-FRAME", "path": "src/frame.rs", "min_lines": 150,
     "markers": ["pub struct FrameLoop", "pub enum FrameState", "pub fn start", "pub fn stop",
                 "pub fn suspend", "pub fn resume", "pub fn step_frame", "pub struct Timeline"]},
    {"id": "BRG-FEED", "path": "src/feed.rs", "min_lines": 80,
     "markers": ["pub struct SpanFeed", "pub fn attach", "pub fn drain_once", "pub fn is_backpressured"]},
    {"id": "BRG-TERMINAL", "path": "src/terminal.rs", "min_lines": 100,
     "markers": ["pub struct TerminalSession", "pub fn kitty_keyboard_flags", "pub fn resize",
                 "pub fn enable_mouse", "pub fn query_pixel_resolution"]},
    {"id": "BRG-WIDGETS", "path": "src/widgets.rs", "min_lines": 150,
     "markers": ["pub struct BoxWidget", "pub struct TextWidget", "pub struct InputWidget",
                 "pub struct SelectWidget", "pub struct ScrollBoxWidget", "pub fn render_to_grid"]},
    {"id": "BRG-POST", "path": "src/post.rs", "min_lines": 60,
     "markers": ["pub fn apply_scanlines", "pub fn apply_invert", "SEPIA", "pub struct ColorMatrix"]},
    {"id": "BRG-CLIPBOARD", "path": "src/clipboard.rs", "min_lines": 60,
     "markers": ["pub trait Clipboard", "pub struct NullClipboard", "pub enum ClipboardError"]},
    {"id": "BRG-ZIGDECL", "path": "src/zigdecl.rs", "min_lines": 100,
     "markers": ["pub fn create_renderer", "pub fn render_native", "pub fn setup_terminal",
                 "pub fn destroy_renderer", "pub fn resize_renderer"]},
    {"id": "BRG-GOLDEN", "path": "tests/bridge_golden.rs", "min_lines": 80,
     "markers": ["fn golden_chat_80x24", "fn golden_resize", "fn golden_wide_chars", "SNAPSHOT"]},
    {"id": "BRG-ABI", "path": "build.rs", "min_lines": 80,
     "markers": ["provenance", "PROVENANCE", "expected_artifact"]},
]


def check_lane(lane: dict) -> dict:
    base = BRIDGE if lane["path"].startswith(("src/", "tests/")) else BRIDGE
    path = ROOT / "crates" / "opentui-bridge" / lane["path"]
    _ = base
    if not path.is_file():
        return {"status": "MISSING", "detail": "file does not exist"}
    text = path.read_text(encoding="utf-8")
    lines = text.splitlines()
    if len(lines) < lane["min_lines"]:
        return {"status": "STUB", "detail": f"only {len(lines)} lines, need >={lane['min_lines']}"}
    stubs = [m for m in STUB_MARKERS if m in text]
    if stubs:
        return {"status": "STUB", "detail": f"stub markers: {', '.join(stubs)}"}
    missing = [m for m in lane["markers"] if m not in text]
    if missing:
        return {"status": "INCOMPLETE", "detail": f"missing API: {', '.join(missing)}"}
    tests = text.count("#[test]")
    return {"status": "PASS", "detail": f"{len(lines)} lines, {tests} tests, all markers present"}


def main() -> int:
    parser = argparse.ArgumentParser()
    parser.add_argument("--json", action="store_true")
    args = parser.parse_args()
    results = [{"lane": l["id"], **check_lane(l)} for l in LANES]
    if args.json:
        print(json.dumps(results, indent=2))
    else:
        for r in results:
            print(f"{r['status']:10} {r['lane']:14} {r['detail']}")
        failed = [r for r in results if r["status"] != "PASS"]
        if failed:
            print(f"\n{len(failed)} lane(s) need attention: {', '.join(r['lane'] for r in failed)}")
    return 0 if all(r["status"] == "PASS" for r in results) else 1


if __name__ == "__main__":
    raise SystemExit(main())
