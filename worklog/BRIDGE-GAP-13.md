# BRIDGE-GAP-13

- Claim: `BRIDGE-GAP-13`; session `ses_f28c0df04ffdnzWb4iVFI94SUN`.
- Owned product file: `crates/opentui-bridge/src/theme_resolve.rs` (new only).
- Candidate repository revision inspected: `d460eb965b200e1454941db5be200b548746dea9`.
- Upstream pin: OpenCode `95daf90670b7c039c436c85537da5fbfe2205b41` (`sources/upstream.lock.json:9`).

## Source evidence

- Native target fields/defaults: `crates/opentui-bridge/src/theme.rs:33-88`, especially `selected_list_item_text`, `background`, `background_element`, `background_menu`, and `thinking_opacity`; current default opacity is `0.6` at line 176.
- Existing native string formats: `crates/opentui-bridge/src/theme_registry.rs:22-34`, `transparent`/`none`, hex, decimal ANSI.
- Reference behavior: pinned TS `packages/tui/src/theme/index.ts:115-118` defines Theme/ThemeJson shapes; `resolveTheme` around lines 241-300 selects `dark`/`light`, resolves `defs[c] ?? theme.theme[c]`, detects circular/missing references, passes numeric ANSI through `ansiToRgba`, then applies `selectedListItemText -> background`, `backgroundMenu -> backgroundElement`, and `thinkingOpacity ?? 0.6`.
- Local recorded gap: `BRIDGE_MIGRATION_DETAIL.md:280-284` lists missing defs/ref chain, variants, and these fallbacks.
- Representative pinned asset `packages/tui/src/theme/assets/opencode.json` uses `{ dark, light }` variants and defs references.

## Observable contract

- `resolve_color` returns resolved source strings; no parsing/conversion. Literal, transparent, and digit-only ANSI specs pass through.
- Variant encoding is `dark:<spec>,light:<spec>`, selected by `mode_dark`.
- Reference lookup is defs first, then theme. Repeated reference name returns `ResolveError::Circular`; absent name returns `ResolveError::Missing`.
- Optional helpers apply the two compatibility fallbacks. Opacity reads `thinkingOpacity` and defaults to `0.6`.
- No persistence, ownership, allocation growth, or dependency surface. Resolution work is bounded by supplied slices and current chain.

## Tests

In-file tests cover dark/light variants, defs/theme chain precedence, numeric ANSI, circular chain, missing ref, both optional fallbacks, explicit selected text, opacity override/default.

Command:

`rtk rustc -D warnings --edition=2021 --test crates/opentui-bridge/src/theme_resolve.rs -o /home/rashid/.cache/bun-tmp/opencode/theme_resolve_tests && rtk /home/rashid/.cache/bun-tmp/opencode/theme_resolve_tests`

Result: 6 passed, 0 failed. `rtk rustfmt --check` and `rtk git diff --check` passed.
Owned-file SHA-256: `4bde3f4437ca265fedbcceba152c04debeb4d8d2c560a397ccc0f5b6b079975f`.

## Decisions / boundaries

- `ResolveError` contains `Circular(String)` and `Missing(String)` only, matching requested fail-closed categories.
- `lib.rs` intentionally untouched. Orchestrator must add `pub mod theme_resolve;` and run crate integration checks.
- Cargo prohibited by lane request. Standalone std-only `rustc --test` used.
- No unresolved behavior in owned file.
