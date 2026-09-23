# BRG-MOUSE - Mouse event parsing (opentui-bridge)

## Claim
- Task: BRG-MOUSE, session ses_brg_mouse, status: in-progress.

## Source evidence (upstream, read-only)
- `packages/core/src/lib/parse.mouse.ts`:
  - `MouseEventType = "down" | "up" | "move" | "drag" | "drag-end" | "drop" | "over" | "out" | "scroll"`
  - `ScrollInfo { direction: "up"|"down"|"left"|"right"; delta: number }`
  - `RawMouseEvent { type; button: number; x: number; y: number; modifiers: {shift,alt,ctrl}; scroll?: ScrollInfo }`
  - `parseMouseEvent(data: Buffer|Uint8Array): RawMouseEvent | null`
  - SGR: ESC [ < B ; X+1 ; Y+1 M/m
  - X10: ESC [ M Cb Cx Cy  (6 bytes, Cb=button+32, Cx/Cy=x+33)
  - SGR button bits: 0-1 button, 2=shift, 3=alt, 4=ctrl, 5=button2, 6=wheel, 7=motion
  - SGR scroll: bit6 set, button 0=up,1=down,2=left,3=right
  - SGR motion (bit5, 0x20): button==3 => "move"; else drag if buttons pressed
  - SGR press/release: M=down, m=up; scroll release NOT classified as scroll
  - X10: isScroll = (bb & 64), isMotion = (bb & 32)
- `parse.mouse.test.ts` (545 lines):
  - `encodeBasic(buttonByte, x, y)`: ESC [ M (bb+32) (x+33) (y+33)
  - `encodeSGR(buttonCode, x, y, press)`: ESC [ < code; x+1; y+1 + (M|m)
  - Left button down: SGR 0 ; X10 0. Middle 1. Right 2. Release 3 (X10) / press=false (SGR).
  - SGR scroll: 64 up, 65 down, 66 left, 67 right. SGR motion+scroll (96/97) => move.
  - SGR drag: press(0,_,true) then motion(32,_,false) => drag.
  - SGR motion w/o prior press => move.
  - SGR release+motion bit (e.g. 32 release) => move (isMotion true, pressRelease == 'm').
  - Modifiers: shift bit2(4), alt bit3(8), ctrl bit4(16). Combined 28 = all.
  - Origin: SGR x+1/y+1 => x=0,y=0 from 1,1. X10 x+33 => x=0 from byte 33.
  - SGR large coords work (no 223 limit). X10 coords >=95 corrupt under utf8 but OK under latin1.
  - Returns null for empty buffer, unrelated escape, incomplete SGR.
- `renderer.ts`:
  - `dispatchMouseEvent(target, attributes)`: builds MouseEvent from RawMouseEvent, autofocus on "down"+LEFT.
  - `recheckHoverState` (~3823): after hit grid change, re-evaluate hover: same element = no-op, else out on old + over on new. Uses `_latestPointer` (x,y,modifiers) stored from `processSingleMouseEvent`.
  - `hitTest(x,y)` returns renderable number; `_hasPointer` set true on event; `_lastPointerModifiers` stored.
  - `processSingleMouseEvent`: sets _latestPointer, _hasPointer, _lastPointerModifiers before dispatch.
- `scroll-acceleration.ts`:
  - `LinearScrollAccel.tick` => always 1. `reset` no-op.
  - `MacOSScrollAccel`: streakTimeout=150ms, minTickInterval=6ms, historySize=3, exponential curve: vel=ref/avgInterval, x=vel/tau, mult=1+A*(exp(x)-1), cap maxMultiplier(6). First tick / timeout => 1. dt<minTickInterval => 1 (ignore). reset => lastTickTime=0, history=[].

## Target boundary
Owned file ONLY: `crates/opentui-bridge/src/mouse.rs` (does not exist yet).
Std-only, `forbid(unsafe_code)`. Provide real, functional code wired into a standalone
rustc --test harness (crate is rlib; tests inline in mouse.rs).

## Markers (required)
- `pub struct RawMouseEvent`
- `pub enum MouseEventType` (SGR-only subset; drag/drag-end/drop/over/out are renderer-side promotions — keep parse-level enum aligned to parse mouse.ts: down/up/move/scroll. Use the 7 parse-level variants; renderer promotions are noted.)
- `pub fn parse_mouse_event`
- `pub struct ScrollAccel` (Rust port of MacOSScrollAccel)

## Tests (>=6, TDD RED first)
1. SGR left press: code 0 => down, button 0, x,y from (x+1,y+1).
2. SGR left release: code 0 press=false => up.
3. SGR scroll up: code 64 press=true => scroll up.
4. SGR scroll+motion 96 => move, scroll None.
5. SGR drag: press(0) then motion(32) => drag (button-press tracking).
6. SGR motion w/o press => move.
7. X10 left press: bb 0 => down, button 0.
8. X10 release byte 3 => up.
9. X10 scroll 64 => scroll up.
10. Modifiers: SGR 28 => shift+alt+ctrl.
11. Null/empty/incomplete => None.
12. OOB clamp: wheel delta exceeds i32 range handled; large coords (SGR 500,300).
13. ScrollAccel: first tick=1; timeout(>150ms)=1; rapid ticks accelerate >1; minTickInterval <6ms ignored; reset.
14. Hover coords: parse returns x,y consistent for recheckHoverState reconstruction (latestPointer fields present).

## Decisions
- MouseEventType: include all 9 parse-level variants from TS (down, up, move, drag, drag-end, drop, over, out, scroll) for fidelity, but only down/up/move/scroll are produced by parse (renderer promotes). This mirrors upstream where parse mouse.ts defines the enum but renderer builds MouseEvent variants.
- Modifiers stored as struct {shift,alt,ctrl: bool}.
- Coordinates stored as signed i32 (x,y) to allow -1 sentinel and large SGR values; parse mouse.ts uses JS numbers. Clamp to i32.

## Remaining unknowns
- None blocking; standalone rustc --test harness is green path per DISC precedent.
