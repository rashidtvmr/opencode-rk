# BRG-STDIN scratchpad

## Claim
- Task: BRG-STDIN
- Session: ses_brg_stdin
- Owned file: crates/opentui-bridge/src/stdin_parser.rs

## Source evidence (upstream OpenTUI, read-only)
- `packages/core/src/lib/stdin-parser.ts:587-636` — `StdinParser` class skeleton: pending ByteQueue, events[], timeoutMs, maxPendingBytes, armTimeouts, onTimeoutFlush, useKittyKeyboard, clock, protocolContext, timeoutId, destroyed, pendingSinceMs, pendingTimeoutPaused, suspendedPixelResolutionPrefixLength, forceFlush, justFlushedEsc, state (ParserState), cursor, unitStart, paste (PasteCollector|null).
- `stdin-parser.ts:113-132` — DEFAULT_TIMEOUT_MS=20, DEFAULT_MAX_PENDING_BYTES=64*1024, INITIAL_PENDING_CAPACITY=256, ESC=0x1b, BEL=0x07, BRACKETED_PASTE_START=ESC[200~, BRACKETED_PASTE_END=ESC[201~, DEFAULT_PROTOCOL_CONTEXT {kittyKeyboardEnabled, privateCapabilityRepliesActive, pixelResolutionQueryActive, explicitWidthCprActive, startupCursorCprActive} (all false).
- `stdin-parser.ts:136-210` — ByteQueue: length, capacity, view(), take(), append, consume, clear, reset, ensureCapacity (compact+doubling).
- `stdin-parser.ts:254-260` — utf8SequenceLength: <0x80 ->1, 0xc2-0xdf ->2, 0xe0-0xef ->3, 0xf0-0xf4 ->4, else 0.
- `stdin-parser.ts:262-274` — bytesEqual.
- `stdin-parser.ts:278-311` — isMouseSgrSequence: length>=7, ESC [, <, final M/m, 3 semicolon-separated digit groups.
- `stdin-parser.ts:313-315` — isAsciiDigit (0x30-0x39).
- `stdin-parser.ts:349-372` — parseKittyFirstFieldCodepoint: first colon, digits before, digits/colons remaining.
- `stdin-parser.ts:374-425` — canDeferParametricCsi (kitty u/special, CPR, pixel res), canStillBe* helpers.
- `stdin-parser.ts:428-470` — canCompleteDeferredParametricCsi.
- `stdin-parser.ts:472-478` — classifyParametricCsiProtocol: semicolons==1,segments==1,hasDigit,final R -> "cpr" else "csi".
- `stdin-parser.ts:480-494` — canDeferPrivateReplyCsi / canCompleteDeferredPrivateReplyCsi (final c/n/y for $-forms, or $y/$n or u).
- `stdin-parser.ts:496-516` — concatBytes, withEscPrefix.
- `stdin-parser.ts:518-538` — indexOfBytes.
- `stdin-parser.ts:544-550` — decodeLatin1 / decodeUtf8.
- `stdin-parser.ts:552-558` — createPasteCollector {tail:EMPTY, parts:[], totalLength:0}.
- `stdin-parser.ts:560-577` — joinPasteBytes (single-part fast path).
- `stdin-parser.ts:587-787` — push(): ground+empty -> scan paste start marker; append through marker; scanPending; consumePasteBytes for remainder; bounded overflow flush.
- `stdin-parser.ts:792-820` — read()/drain(): read pops event; drain loops read invoking callback, stops on destroy.
- `stdin-parser.ts:826-850` — flushTimeout(now): tryForceFlush if pendingSinceMs set and elapsed >= timeoutMs.
- `stdin-parser.ts:844-850` — tryForceFlush: sets forceFlush when not in paste and pending non-empty.
- `stdin-parser.ts:906-1825` — scanPending() switch: ground/utf8/esc/csi/ss3/esc_recovery/esc_less_mouse/esc_less_x10_mouse/csi_sgr_mouse/csi_sgr_mouse_deferred/csi_parametric/csi_parametric_deferred/csi_parametric_ignored/csi_private_reply/csi_private_reply_deferred/osc/dcs/apc. forceFlush resolves incomplete -> emitKeyOrResponse/emitOpaqueResponse/emitMouse.
- `stdin-parser.ts:1833-1849` — emitKeyOrResponse: parseKeypress -> key event, else response event.
- `stdin-parser.ts:1851-1864` — emitMouse: parseMouseEvent -> mouse event or opaque unknown.
- `stdin-parser.ts:1870-1886` — emitLegacyHighByte: parseKeypress([byte]) -> key event (meta path).
- `stdin-parser.ts:1888-1894` — emitOpaqueResponse: response {unknown|osc|dcs|apc|csi} + latin1 sequence.
- `stdin-parser.ts:1898-1906` — consumePrefix: pending.consume(end), reset cursor/unitStart/pendingSinceMs/forceFlush.
- `stdin-parser.ts:1911-1918` — takePendingBytes.
- `stdin-parser.ts:1923-1937` — flushPendingOverflow.
- `stdin-parser.ts:1941-1943` — markPending: pendingSinceMs = clock.now().
- `stdin-parser.ts:1952-1979` — consumePasteBytes: concat tail+chunk, indexOf END; if found pushPasteBytes + emit paste + clear; else keep END.length-1 tail.
- `stdin-parser.ts:1981-1991` — pushPasteBytes: copy (alias-safe).
- `stdin-parser.ts:1993-2011` — reconcileDeferredStateWithProtocolContext.
- `stdin-parser.ts:2017-2046` — reconcileTimeoutState: clear+arm setTimeout via clock.
- `stdin-parser.ts:2060-2073` — resetState.
- `parse.keypress.ts:214-479` — parseKeypress(s, {useKittyKeyboard}). Handles: mouse filter, DA/DA2, CPR, window size, mode report, focus, OSC, bracketed paste markers, ctrl keys (0x00->space+ctrl, 0x01-1a->ctrl+letter, 28-31 -> ctrl+char, 0x7f->backspace), esc sequences, meta+char, modifyOtherKeys, fnKeyRe (ANSI/SS3/double-bracket), named keys, shift/ctrl letter, fallback.
- `parse.keypress.ts:125` — nonAlphanumericKeys; `terminalNamedSingleStrokeKeys`.
- `parse.keypress.ts:131-137` — isShiftKey/isCtrlKey.
- `parse.keypress-kitty.ts:1-419` — parseKittyKeyboard(s): parseKittySpecialKey (CSI 1;mod:ev LETTER/~), then CSI codepoint[:shift[:base]] ; mod:event u. kittyKeyMap, functionalKeyMap, tildeKeyMap. super from mod bit 8, hyper bit 16, baseCode for layout fallback.
- `parse.mouse.ts:1-232` — MouseParser: parseMouseEvent / parseAllMouseEvents / parseMouseSequenceAt (CSI < X10 / CSI SGR). decodeSgrEvent / decodeBasicEvent. Modifiers bitfields, scroll direction by button 0-3, motion bit 32.
- `stdin-parser.test.ts:1-2579` — full test suite vectors.

## Observed scenarios (contract)
- push empty -> emits one empty-name key (k("")).
- Printable ASCII a-z -> key a..z. A-Z -> shifted lowercase. 0-9 -> key. symbols -> key. space -> key "space".
- Control 0x00->ctrl+space, 0x08->backspace, 0x09->tab, 0x0a->linefeed, 0x0d->return, 0x01-1a->ctrl+letter, 0x1c-1f->ctrl+@, etc, 0x7f->backspace.
- ESC -> esc state; lone ESC pending; flushTimeout(>=timeoutMs) -> emit escape key (k("escape")), sets forceFlush.
- ESC [ A -> csi_sgr_mouse_deferred then up key. ESC O letter -> SS3. ESC ] ... (BEL|ESC\) -> OSC response. ESC P ... ESC\ -> DCS. ESC _ ... ESC\ -> APC.
- ESC [ digits ~ -> key via fnKeyRe. ESC [ 1 ; mod letter -> modifier key. ESC [ 27 ; mod ; code ~ -> modifyOtherKeys.
- Kitty (when useKittyKeyboard): ESC [ codepoint u / ESC [ codepoint ; mod u / ESC [ 1 ; mod : ev LETTER / ~.
- Bracketed paste: ESC [ 200 ~ ... ESC [ 201 ~ -> paste bytes event (body bytes preserved, may contain ESC). Tail window for split end marker.
- SGR mouse: ESC [ < b ; x ; y (M|... ) m -> mouse event. X10: ESC [ M cb cx cy (3 payload bytes).
- UTF-8: 2/3/4 byte sequences split across pushes reassemble; invalid leads -> legacy meta path; 0xC0 followed by non-continuation -> @ + restart (k("@") ... actually k(i) meta on timeout).

## Target boundary (Rust port, std-only)
- `pub struct StdinParser` with `pub fn push(&mut self, &[u8]) -> SmallVec<[StdinEvent;4]>` (return drained events directly; simpler than internal queue + drain for a single-owner test harness) and `pub fn read(&mut self) -> Option<StdinEvent>` / `pub fn drain(&mut self, impl FnMut(&StdinEvent))` / `pub fn flush_timeout(&mut self, now_ms) -> SmallVec` / `pub fn reset` / `pub fn destroy` / `pub fn update_protocol_context(&mut self, ...)` / `pub fn abort_pending_startup_cursor_cpr` / `pub fn pause_pending_timeout` / `pub fn resume_pending_timeout` / `pub fn has_pending_pixel_resolution_response` / `pub fn buffer_capacity`.
- `pub enum StdinEvent { Key { raw, key }, Mouse { raw, encoding, event }, Paste { bytes }, Response { protocol, sequence } }` with `protocol: StdinResponseProtocol` = csi|cpr|osc|dcs|apc|unknown.
- `pub struct ProtocolContext { kitty_keyboard_enabled, private_capability_replies_active, pixel_resolution_query_active, explicit_width_cpr_active, startup_cursor_cpr_active }`.
- Key parsed fields: name, ctrl, meta, shift, option, super, hyper, number, raw, sequence, base_code (Option<u32>), event_type (press/repeat/release), source (raw|kitty).
- Mouse event: type, button, x, y, modifiers {shift,alt,ctrl}, scroll (Option).
- std-only (no crates.io deps), forbid(unsafe_code). Bounded pending queue (MAX_QUEUE_BYTES default 64*1024, evict-or-error via flushPendingOverflow as unknown response).
- TDD: >=7 tests, min 120 lines. byte-at-a-time invariance, split escapes across pushes, kitty image reply routing (cpr/csi/private-reply/pixel-resolution), capability routing, bracketed paste assembly, bounded queue eviction.

## Decisions
- Parse keypressed natively in Rust (port parseKeypress + kitty), not by shelling out.
- parseMouseEvent ported inline (SGR + X10), small.
- Clock: inject FnMut()->u64 for nowMs; timeout via flush_timeout(now) external call (no threads). Tests pass explicit now.
- push returns Vec<StdinEvent> drained immediately (matches test pattern p.push + snap which drains synchronously). Simpler than TS queue+drain but preserves semantics. Actually keep an internal event queue + read/drain/push returning nothing? Spec says markers pub fn drain, pub fn read, pub fn push. To honor "bounded queue eviction" tests need overflow path on push. We'll keep internal Vec<StdinEvent> + pending ByteQueue, push appends events, drain/read pops. push returns nothing. Tests call drain().
- ByteQueue: Vec<u8> with start/end + compact.
- State enum ported from ParserState union.

## Remaining unknowns
- kitty functional `codepoint:shifted:base` (colon form) in field 1 with event type. Tests cover `\x1b[97;2u` (simple), `\x1b[1;1:3A` (functional), `\x1b[97:65;2u`? not in vectors but parseKittyFirstFieldCodepoint handles. Port what vectors require.
