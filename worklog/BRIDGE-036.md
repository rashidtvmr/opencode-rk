# BRIDGE-036 solid_host

Claim: `@opentui/solid` used-boundary mirror.
Evidence: TS checkout a0d9b6c (NOT 95daf90).
- `app.tsx:1` import; `render` at `:245`; `<TimeToFirstDraw/>` at `:1108`; dims at `:1089-1090`.
- `plugin/slots.tsx:2,33-48` createSlot/createSolidSlotRegistry/SolidPlugin/JSX.
- `permission.tsx:4,714` Portal.
- `dialog-workspace-file-changes.tsx:2,46`, `error-component.tsx:2,60` useKeyboard.
- `bg-pulse.tsx:8,69` extend; fps save-restore `:74-86`.
- `register-spinner.ts:1-2` NOT solid root (components/spinner pkgs) - out of scope.
- `keymap.tsx:15` useBindings from `@opentui/keymap/solid`, NOT solid - excluded.
Target: dims-only local RenderOptions, SlotId bound 1024, TTFD helper.
Tests: 7 written (dims zero, dims ok/narrow/large, slot bound, ttfd ok, ttfd breached, caps, fps). RED then impl; NOT cargo-run (scope forbids). Logically green.
Unknowns: exact TimeToFirstDraw deadline default (flag-gated, no constant found); solid `JSX.Element` typing unneeded in Rust.
