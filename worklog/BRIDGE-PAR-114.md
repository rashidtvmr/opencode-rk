# BRIDGE-PAR-114 scratchpad

Claim: BRIDGE-PAR-114 via ses_par114. Owned file: crates/opentui-bridge/src/input_adapter.rs.

Source evidence:
- crates/opentui-bridge/src/native_input.rs:48 `pub fn map_event`, :34 `enum InputChunk { Quit, Page(Page), SubmitText, EditBackspace, Paste, Resize, Noop }`, :88 `pub fn decode_bytes(buf:&[u8])->(Vec<InputChunk>,usize)` fail-closed (invalid/incomplete => 0 consumed), :20 `MAX_TEXT=4096`.
- crates/opentui-bridge/src/loop_driver.rs:73 `pub fn step(state:&mut LoopState, chunk:LoopStep)`.
- Tests in native_input.rs: `decode_bytes(&[0x03])==(Quit,1)`, `decode_bytes(b"hi\r")==([SubmitText("hi")],3)`, `decode_bytes(&[0xff])==([],0)`.

Target boundary: ONE new file input_adapter.rs only. No lib.rs/Cargo.toml/native_input.rs/loop_driver.rs edits. No cargo/commit.

Decisions:
- feed_bytes caps at MAX_TEXT (4096, reuse native truth), keeps head, drops tail overflow.
- drain_step: decode whole buf; empty buf => empty vec; consumed==0 && chunks empty => clear + ["invalid"]; else drain(..consumed), map chunks to labels, filter Resize/Noop.
- Labels: Quit=>quit, Page(Palette/Context/Help/Chat)=>palette/context/help/chat, EditBackspace=>backspace, SubmitText("")=>submit, SubmitText(t)/Paste(t)=>per-char type:<char>, Resize/Noop=>skipped.
- Page(Chat)=>"chat" extra (spec list omits it; exhaustive match needs it; harmless).
- Paste=>per-char type labels (spec silent; preserves data).

Tests (6): feed_cap, feed_room_respected, drain_clears, submit_label, quit_label, type_label, invalid_safe = 7 actually.

Unknowns: none. Byte assumptions all backed by native_input.rs tests + map_key arms.
