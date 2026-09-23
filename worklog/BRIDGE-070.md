# BRIDGE-070 scratchpad

Claim: console/renderer options mirror (app.tsx:194-242, bg-pulse fps).
Source: TS a0d9b6c app.tsx:196 targetFps 60, :199 useKittyKeyboard {}, :204 keyBindings y+ctrl copy-selection, :242 waitForThemeMode(1000), :437 onCopySelection, :849 console.toggle; bg-pulse.tsx:74-86 maxFps=30 writable; dialog.tsx:152 currentFocusedRenderable (context only).
Target: renderer_config.rs additive only.
Tests: 6 new (defaults, action roundtrip, bounds, cap16, fps range, kind) + 4 existing = 10.
Decisions: reuse KittyFlags via crate::core_events (no redefine); note kitty 0 vs 5 divergence in doc comment; MaxFps(u16) set_max_fps 1..=240; ConsoleAction CopySelection/Toggle consts.
Unknowns: toggle action id "toggle" inferred from console.toggle (no TS action string found); dialog focus renderable not modeled here.
