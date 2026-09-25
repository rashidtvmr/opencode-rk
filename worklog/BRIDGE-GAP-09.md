# BRIDGE-GAP-09

- Claim: `BRIDGE-GAP-09`
- Session: `ses_f28c0df1dffe5aMOPDeP9aDPJg`
- Owned file: `crates/opentui-bridge/src/keymap_resolve.rs`
- Scope: resolve key plus mode stack to action and event flags; no `lib.rs` edits.
- Status: claimed; source inspection pending.

## Source evidence

- Current revision: `6d63263` (`prod/native-tui-parity`).
- `crates/opentui-bridge/src/keymap.rs:118-165`: `ModeStack` owns a bottom `base` mode, `push` appends, `pop` removes one non-base entry, `current` returns the top, and `depth` is bounded. The existing type exposes no parent iterator; the new resolver therefore consumes an ordered mode snapshot.
- `crates/opentui-bridge/src/keymap.rs:77-116`: `BindingValue::Disabled`/`Unbound` do not resolve; stroke values do. `keymap.rs:173-213`: `KeymapConfig` stores named values.
- `crates/opentui-bridge/src/keybind_tables.rs:66-78`: `BindingObject` carries `prevent_default` (default true) and `fallthrough` (default false). `:122-149` confirms defaults. `:198-207` records the per-definition prevent-default value; `:397-562` maps names to command actions. `input_paste` is `ctrl+v`, action `prompt.paste`, `prevent_default=false` (`:319-321`, `:506-508`).
- `/home/rashid/projects/opencode/packages/tui/src/keymap.tsx:53-100`: mode stack is LIFO, push returns an id-based disposer, pop removes the matching id, and dispose clears the stack and mode data. `:214-243` registers addon disposers and disposes mode stack.
- `/home/rashid/projects/opencode/packages/tui/src/config/keybind.ts:17-33`: binding object has `preventDefault` and `fallthrough`; `:161-162` confirms `input_paste` is the false-prevent-default exception.
- OpenTUI keymap docs: `fallthrough` continues matching later bindings; `preventDefault` independently controls host delivery; both default to `false`/`true` respectively.

## Contract

- `KeyQuery { seq, mode }` identifies a key sequence and active mode.
- Resolver walks the mode snapshot from newest to oldest, always trying the query mode first when it is present, then each parent.
- A matching non-fallthrough binding wins. A matching fallthrough binding is retained as a candidate and lookup continues to parents; the first non-fallthrough parent wins. If no terminating parent match exists, return the newest matched action and report `fallthrough=true`.
- No match returns `action=None`, `prevent_default=false`, `fallthrough=false`.
- Match flags come from the selected binding; omitted/default flags are `prevent_default=true`, `fallthrough=false`.

## RED evidence

- Test file SHA-256 before implementation: `b0dd5c161248bda1822f2345d99cceeace69366612b214d93264d5e36017bc89`.
- Command: `rtk rustc --edition=2021 --test crates/opentui-bridge/src/keymap_resolve.rs -o /home/rashid/.cache/bun-tmp/opencode/keymap_resolve_red && rtk proxy /home/rashid/.cache/bun-tmp/opencode/keymap_resolve_red --nocapture`.
- Result: compiling RED, 1 passed, 4 failed; failures were the missing resolver behavior, not compile errors.

## Decisions / unknowns

- Keep this file self-contained and std-only because `lib.rs` is integrator-owned and existing `ModeStack` cannot expose its private parent vector.
- Use a small explicit `Binding`/`ModePath` model; integrator can adapt `keybind_tables` rows and `keymap::ModeStack` snapshots without changing existing modules.
- No `lib.rs`, `keymap.rs`, or `keybind_tables.rs` edits.

## Final verification (2026-09-25 lane close)

- `rustfmt --edition 2021 --check crates/opentui-bridge/src/keymap_resolve.rs` clean (fixed blank-line-after-`{` diffs at 20/49/76/110/117/136).
- In-file tests: 5 (`direct_hit`, `fallthrough_walks_to_parent`, `no_match_is_none`, `input_paste_keeps_prevent_default_false`, `newest_mode_wins_in_lifo_order`). Impl + tests present, no cargo run per role (no cargo).
- Status: done, ledger flip to completed.
