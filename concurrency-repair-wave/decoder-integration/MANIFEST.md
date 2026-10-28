# Decoder Repair Integration Manifest

## Summary
Exact integration plan for adding TabNext/TabPrevious/TabSlot InputEvent variants
to canonical `native_input_decoder.rs` and dispatching them in `tui_entry.rs`.

## Source evidence
- Upstream keybinds: `packages/tui/src/config/keybind.ts:113-114`
  - `session.tab.next`: ctrl+tab,alt+down
  - `session.tab.previous`: ctrl+shift+tab,alt+up
- Upstream dispatch: `packages/tui/src/context/session-tabs.tsx:479-483` (cycle)
  - `packages/tui/src/context/local.tsx:596-601` (quickSwitch, 1-based slot)
- Native NavigationState API: `crates/cli/src/native_navigation.rs:444-464`
  - `step_tab(forward: bool) -> Option<usize>`
  - `select_tab_slot(slot: usize) -> Option<usize>` (1-based)
  - `tab_at(index: usize) -> Option<&Tab>` (Tab.session is pub)

## Candidate file
- GREEN candidate: `/private/var/folders/b0/dj81nc_j2yq2yqkmg0yd2sgyc0000gn/T/opencode/concurrency20/tab-decoder/native_input_decoder.rs`
  - hash: 934a631c40bc44e7178c0ae120209dc543b315228121d448b4ac392778a27a71

## Changes required (exact paths/lines)

### File 1: crates/cli/src/native_input_decoder.rs
1. InputEvent enum (canonical lines 21-31): add 3 variants
2. decode_kitty method: insert tab nav logic before existing printable behavior (canonical line 494)
3. Add `#[cfg(test)] mod tab_nav_tests` with 10 tests (after mouse_protocol_tests at canonical line 707)

### File 2: crates/cli/src/tui_entry.rs
1. Debug event kind match (canonical lines 4798-4809): add 3 arms
2. Keyboard dispatch (insert before Paste arm at canonical line 5424): add TabNext/TabPrevious/TabSlot arm

## Compile blockers
1. Non-exhaustive InputEvent match — Step 4 and Step 5 fix this
2. TabSlot dispatch must convert 0-based (decoder) to 1-based (NavigationState::select_tab_slot)

## Verification commands
- `cargo test -p opencode_cli --lib native_input_decoder:: -- --nocapture`
- `cargo test -p opencode_cli --lib native_navigation:: -- --nocapture`
- `cargo check -p opencode_cli`
- `cargo test -p opencode_cli --features native`

## Scratchpad
- `decoder-integration/integration-plan-scratchpad.md`
- hash: 3b02a1f91794b83fdd492e2a42ab85310d35fb1187ce36b42ed8c1def9d68dd8
