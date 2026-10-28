# Decoder Repair Integration Plan

Updated: 2026-10-26; role: integration preparation researcher; session: ses_e86a7e2c1ffeey2oM3HVq8iIuK

## Base/Candidate SHA
- Base (canonical): 6f6872bf87e02f05afe9c1a5f63e55fe4c040499
- Upstream pin: 450e7b92412c780a7b5a75226bb8d49aaca24e22

## Absolute paths
- Canonical decoder (RED, missing tab-nav): `/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/cli/src/native_input_decoder.rs`
  - hash: 363e070883b89370e4443777a659283b91733b7872e728efedade9ef98a7c065
- RED snapshot baseline: `/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/cli/src/native_input_decoder.rs.red_snapshot.rs`
  - hash: 8f1f3439ccfc7d41ee08053a65a4594f8846fe2ca93450f1465ee5258fb2232c
- Private GREEN candidate (already implemented): `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/concurrency20/tab-decoder/native_input_decoder.rs`
  - hash: 934a631c40bc44e7178c0ae120209dc543b315228121d448b4ac392778a27a71
- Dispatch site (needs InputEvent match arms): `/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/cli/src/tui_entry.rs`
  - lines 5333-5422: ArrowUp/ArrowDown dispatch block (insert TabNext/TabPrevious/TabSlot before Paste at line 5424)
  - lines 4798-4809: debug event kind match (needs TabNext/TabPrevious/TabSlot arms)
- Navigation API: `/Users/mymac/Projects/opencode-rk-recovery-20261005/crates/cli/src/native_navigation.rs`
  - `step_tab(&mut self, forward: bool) -> Option<usize>` (line 444) - equivalent to upstream `sessionTabs.cycle(1/-1)`
  - `select_tab_slot(&mut self, slot: usize) -> Option<usize>` (line 457) - equivalent to upstream `local.session.quickSwitch(slot)`
- Upstream reference: `/Users/mymac/Projects/tmp/opencode/packages/tui/src`
  - `config/keybind.ts` line 113-114: `session.tab.next` => ctrl+tab,alt+down; `session.tab.previous` => ctrl+shift+tab,alt+up
  - `context/local.tsx` line 596-601: `quickSwitch(slot)` -> `local.session.quickSwitch(slot)` selects tab by 1-based slot
  - `context/session-tabs.tsx` line 479-483: `cycle(direction)` calls `route.navigate({type: "session", sessionID: tab.sessionID})`

## Goal
Integrate tab-navigation InputEvent variants into canonical `native_input_decoder.rs` and dispatch them in `tui_entry.rs`.

## Source evidence

### Upstream keybind mapping (keybind.ts:113-114)
- `session.tab.next`: ctrl+tab, alt+down
- `session.tab.previous`: ctrl+shift+tab, alt+up

### Kitty encoding (from GREEN candidate + RED snapshot)
- Kitty modifier byte = (sum of flag bits) + 1
  - bit0(1)=Shift, bit1(2)=Alt, bit2(4)=Ctrl
  - So: 1=none, 2=Shift, 3=Alt, 4=Shift+Alt, 5=Ctrl, 6=Ctrl+Shift
- Ctrl+Tab => CSI 9;5u => TabNext (codepoint 9, modifier 5)
- Ctrl+Shift+Tab => CSI 9;6u => TabPrevious (codepoint 9, modifier 6)
- Alt+Down => CSI 9;3u => TabNext (codepoint 9, modifier 3)
- Alt+Up => CSI 8;3u => TabPrevious (codepoint 8=backspace, modifier 3)
- Ctrl+1..9 => CSI 49-57;5u => TabSlot(0..8)
- Ctrl+0 => CSI 48;5u => TabSlot(9) (wrapping)
- Release events (event_type=2 after colon) dropped; only press (event_type=1) or repeat accepted

### Upstream dispatch semantics (app.tsx:847-861, local.tsx:65-70, 596-601)
- `cycle(direction)` -> navigate to next/prev tab by sessionID
- `quickSwitch(slot)` -> navigate to tab in 1-based slot (tab index + 1)

### Native Navigation API
- `NavigationState::step_tab(forward: bool) -> Option<usize>` — wraps around, returns new active index
- `NavigationState::select_tab_slot(slot: usize) -> Option<usize>` — 1-based slot, returns new index or None
- `select_tab_transition()` (tui_entry.rs:7616) — existing transition used for tab selection via mouse/leader; takes session reference, saves active state, switches tab, binds session

## Plan: exact minimal paths/order

### Step 1: native_input_decoder.rs — InputEvent enum (canonical lines 21-31)
Replace:
```rust
pub(crate) enum InputEvent {
    Byte(u8), Escape, ArrowUp, ArrowDown, ArrowLeft, ArrowRight, Paste(String),
    Mouse { x: u16, y: u16, kind: u8, modifiers: u8 },
}
```
With:
```rust
pub(crate) enum InputEvent {
    Byte(u8), Escape, ArrowUp, ArrowDown, ArrowLeft, ArrowRight,
    TabNext, TabPrevious, TabSlot(usize),
    Paste(String), Mouse { x: u16, y: u16, kind: u8, modifiers: u8 },
}
```

### Step 2: native_input_decoder.rs — decode_kitty tab nav logic (insert before existing printable behavior)
Insert into `decode_kitty` method (before line 494 in canonical, i.e., before the existing printable/UTF-8 section). Logic from GREEN candidate lines 441-491:

```rust
const CTRL: u32 = 0b100;
const SHIFT: u32 = 0b001;
const CAPS_LOCK: u32 = 0b1_000_000;
const NUM_LOCK: u32 = 0b10_000_000;
let harmless_locks = CAPS_LOCK | NUM_LOCK;

if codepoint == 9 && modifier >= 1 {
    let modifier_bits = modifier - 1;
    if modifier_bits & CTRL != 0 {
        if modifier_bits & SHIFT != 0 {
            self.emit(InputEvent::TabPrevious);
        } else {
            self.emit(InputEvent::TabNext);
        }
        return;
    }
    if modifier_bits & 0b010 != 0 && modifier_bits & !0b010 & !harmless_locks == 0 {
        self.emit(InputEvent::TabNext);
        return;
    }
}
if codepoint == 8 && modifier >= 1 {
    let modifier_bits = modifier - 1;
    if modifier_bits & 0b010 != 0
        && modifier_bits & !(0b010 | harmless_locks) == 0
    {
        self.emit(InputEvent::TabPrevious);
        return;
    }
}
if (48..=57).contains(&codepoint) && modifier >= 1 {
    let modifier_bits = modifier - 1;
    if modifier_bits & CTRL != 0
        && modifier_bits & !(CTRL | SHIFT | harmless_locks) == 0
    {
        let digit = (codepoint - 48) as usize;
        let slot = if digit == 0 { 9 } else { digit - 1 };
        self.emit(InputEvent::TabSlot(slot));
        return;
    }
}
```

### Step 3: native_input_decoder.rs — tab_nav_tests module (copy from GREEN candidate lines 709-792)
Add the full `#[cfg(test)] mod tab_nav_tests` block with all 9 tests:
- ctrl_tab_emits_tab_next
- ctrl_shift_tab_emits_tab_previous
- alt_down_emits_tab_next
- alt_up_emits_tab_previous
- ctrl_digit_1_emits_tab_slot_0
- ctrl_digit_0_emits_tab_slot_9
- ctrl_digit_5_emits_tab_slot_4
- ctrl_digit_9_emits_tab_slot_8
- ctrl_digit_release_is_dropped
- plain_arrows_preserved (incomplete in GREEN candidate line 790-792; complete as:)
  ```rust
  fn plain_arrows_preserved() {
      let mut d = TerminalInputDecoder::default();
      feed(&mut d, b"\x1b[A");
      assert_eq!(d.pop(), Some(InputEvent::ArrowUp));
      feed(&mut d, b"\x1b[B");
      assert_eq!(d.pop(), Some(InputEvent::ArrowDown));
  }
  ```

### Step 4: tui_entry.rs — debug event kind match (canonical lines 4798-4809)
Add arms:
```rust
native_input_decoder::InputEvent::TabNext => ("tab_next", 0, None),
native_input_decoder::InputEvent::TabPrevious => ("tab_prev", 0, None),
native_input_decoder::InputEvent::TabSlot(slot) => ("tab_slot", *slot, None),
```

### Step 5: tui_entry.rs — keyboard dispatch (insert before Paste arm at line 5424)
Add a new match arm after the ArrowUp/ArrowDown block (after line 5422, before line 5424):
```rust
native_input_decoder::InputEvent::TabNext
| native_input_decoder::InputEvent::TabPrevious
| native_input_decoder::InputEvent::TabSlot(_) => {
    utf8.reset();
    if view.dialogs.intercepts_input()
        || children_panel.open
        || appearance.open
        || permission_editor.open
        || page_open
        || help_open
        || view.command_palette_open()
        || matches!(dialog, native_setup::Dialog::None)
    {
        continue;
    }
    // Tab navigation applies only on the Chat page with no overlays.
    if view.page() != crate::native_view_pages::Page::Chat {
        continue;
    }
    match event {
        native_input_decoder::InputEvent::TabNext => {
            if let Some(index) = navigation.step_tab(true) {
                if let Some(target) = navigation.tab_at(index).map(|t| t.session.clone()) {
                    let scroll = view.chat_scroll_offset();
                    match select_tab_transition(
                        &mut view, &mut transcript, &mut navigation,
                        &draft, draft_cursor,
                        live.as_ref().map(|s| s.origin.as_str()),
                        auth, binding_generation,
                        &target, selected.as_ref().map(|m| m.qualified()), scroll,
                    ) {
                        Ok(result) => {
                            apply_close_tab_result(
                                result, &mut view, &mut live, &mut history_window,
                                &mut binding_generation, &mut draft, &mut draft_cursor,
                                &mut selected, &models, &mut session_route,
                                &mut compact_worker,
                            );
                            completion.sync(&draft);
                        }
                        Err(error) => append_transcript(&mut transcript, format!("error: tab next: {error}")),
                    }
                    needs_paint = true;
                    continue;
                }
            }
        }
        native_input_decoder::InputEvent::TabPrevious => {
            if let Some(index) = navigation.step_tab(false) {
                if let Some(target) = navigation.tab_at(index).map(|t| t.session.clone()) {
                    let scroll = view.chat_scroll_offset();
                    match select_tab_transition(
                        &mut view, &mut transcript, &mut navigation,
                        &draft, draft_cursor,
                        live.as_ref().map(|s| s.origin.as_str()),
                        auth, binding_generation,
                        &target, selected.as_ref().map(|m| m.qualified()), scroll,
                    ) {
                        Ok(result) => {
                            apply_close_tab_result(
                                result, &mut view, &mut live, &mut history_window,
                                &mut binding_generation, &mut draft, &mut draft_cursor,
                                &mut selected, &models, &mut session_route,
                                &mut compact_worker,
                            );
                            completion.sync(&draft);
                        }
                        Err(error) => append_transcript(&mut transcript, format!("error: tab previous: {error}")),
                    }
                    needs_paint = true;
                    continue;
                }
            }
        }
        native_input_decoder::InputEvent::TabSlot(slot) => {
            // Ctrl+digit sends TabSlot(0..9). NavigationState::select_tab_slot uses
            // 1-based slots, so add 1 to the decoder's 0-based index.
            if let Some(index) = navigation.select_tab_slot(slot + 1) {
                if let Some(target) = navigation.tab_at(index).map(|t| t.session.clone()) {
                    let scroll = view.chat_scroll_offset();
                    match select_tab_transition(
                        &mut view, &mut transcript, &mut navigation,
                        &draft, draft_cursor,
                        live.as_ref().map(|s| s.origin.as_str()),
                        auth, binding_generation,
                        &target, selected.as_ref().map(|m| m.qualified()), scroll,
                    ) {
                        Ok(result) => {
                            apply_close_tab_result(
                                result, &mut view, &mut live, &mut history_window,
                                &mut binding_generation, &mut draft, &mut draft_cursor,
                                &mut selected, &models, &mut session_route,
                                &mut compact_worker,
                            );
                            completion.sync(&draft);
                        }
                        Err(error) => append_transcript(&mut transcript, format!("error: tab slot {slot}: {error}")),
                    }
                    needs_paint = true;
                    continue;
                }
            }
        }
        _ => {}
    }
}
```

## Compile blockers / dependencies
1. `tui_entry.rs` match on `InputEvent` is non-exhaustive — adding TabNext/TabPrevious/TabSlot to the enum WILL break compilation at:
   - Line 4798: debug kind match (Step 4 fix)
   - Line 5333: the main keyboard dispatch match (Step 5 adds arms)
   - Line 5487: Escape/ArrowLeft/ArrowRight fallback match — TabNext/TabPrevious/TabSlot must NOT fall through here. The new arm in Step 5 returns `continue` before reaching that match.
2. The `leader_followup_pending` check at line 5339 does NOT conflict — TabSlot uses Ctrl+digit (not leader+digit). No conflict.
3. `NavigationState::step_tab` and `select_tab_slot` are `pub` and return `Option<usize>` — no visibility issues.
4. `navigation.tab_at(index)` returns `Option<&Tab>` — Tab has `.session` field. Need to verify Tab struct has `pub session` field.

## Verification
1. `cargo test -p opencode_cli --lib native_input_decoder -- --nocapture` — tab_nav_tests
2. `cargo test -p opencode_cli --lib native_navigation -- --nocapture` — existing step_tab/select_tab_slot tests still pass
3. `cargo check -p opencode_cli` — compile check for non-exhaustive match exhaustiveness
4. Full test: `cargo test -p opencode_cli --features native`

## Tab struct verification needed
Need to confirm `Tab.session` is accessible (check native_navigation.rs Tab definition).

## Artifact hashes
- Canonical native_input_decoder.rs (RED): 363e070883b89370e4443777a659283b91733b7872e728efedade9ef98a7c065
- RED snapshot baseline: 8f1f3439ccfc7d41ee08053a65a4594f8846fe2ca93450f1465ee5258fb2232c
- GREEN candidate: 934a631c40bc44e7178c0ae120209dc543b315228121d448b4ac392778a27a71