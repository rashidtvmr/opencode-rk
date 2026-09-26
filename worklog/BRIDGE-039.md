# BRIDGE-039 core_events

Claim: core event-lifecycle contract.
Source: crates/opentui-bridge/src/core_events.rs (new, sole touched file).
TS checkout a0d9b6c (not pinned 95daf90).
Observed: CliRenderEvents 16 members chunk-bun-bb3k0yt8.js:7070-7085; kitty flags bits+defaults chunk-bun:6968-6995; copy-selection consoleOptions app.tsx:199-205, onCopySelection app.tsx:436-446; bracketed paste prompt/index.tsx:1393-1417, markers chunk-bun:6427-6428; focus/blur attention.ts:126-134; MouseButton chunk-bun:7035-7042; scroll-up/down bindings chunk-bun:4662-4665.
Target: RenderEvent enum + KittyFlags + Paste + ConsoleBinding + route_event reusing events.rs/input.rs.
Tests: 6 in-file (kitty_from_empty, kitty_bits, paste_truncate, copy_binding, route_mapping, cli_members). RED written first, impl logically green; cargo NOT run per task scope.
Decisions: scroll separate (no wheel in MouseButton); MemorySnapshot->Paint; from_empty=5 with note.
Unknowns: exact palette/capabilities payload shapes (sized minimal); y keycode 121 assumed ASCII.
