# decoder minimal alternative - kitty modifier-1 over-return bug
Updated: 2026-10-27; role: minimal decoder optimizer (security evaluator carryover)
Base/candidate SHA: 6f6872bf87e02f05afe9c1a5f63e55fe4c040499 canonical
Upstream SHA: 450e7b92412c780a7b5a75226bb8d49aaca24e22
Absolute source: /Users/mymac/Projects/opencode-rk-recovery-20261005/crates/cli/src/native_input_decoder.rs
Candidate SHA: 363e070883b89370e4443777a659283b91733b7872e728efedade9ef98a7c065
Private review dir (live, workspace scratch): /Users/mymac/Projects/opencode-rk-recovery-20261005/.scratch.RArhl2/decoder-minimal-alternative/
NOTE: original /tmp/opencode/... path vanished/permission-denied post-restart; used workspace scratch as AGENTS.md-approved substitute.

## Goal and success criteria
Fix decode_kitty lines 412-418: the `if modifier == 1` block emits known arrow codepoints (57350/57351) but then unconditional `return` drops ALL other codepoints under Shift, including printable codepoints (e.g. 99 -> 'c'). Minimal fix: return ONLY in the known left/right arms; let `_ => {}` fall through existing private-use rejection / scalar validation / modifier-0 guard / UTF-8 byte emit. Preserve all existing semantics; add tests for ESC[99;1u (byte 'c'), ESC[233;1u (e-acute UTF-8), 1:2 repeat inert, 1:3 release inert, modifier-0 inert, existing horizontal keys preserved. No new InputEvent variants, no tab shortcuts, no existing semantic test changes.

## Evidence read
- native_input_decoder.rs lines 381-455 (decode_kitty), 478-505 (horizontal_cursor_regressions). Candidate SHA above.
- docs/AGENT_WORKER_PRACTICES.md and docs/SECURITY.md read first.

## Confirmed defect
```rust
if modifier == 1 {                       // Shift only
    match codepoint {
        57350 => self.emit(InputEvent::ArrowLeft),
        57351 => self.emit(InputEvent::ArrowRight),
        _ => {}
    }
    return;                                // BUG: drops every other Shift codepoint
}
```
ESC[99;1u: codepoint 99 is 'c'. Not in 0xE000..=0xF8FF, not > 0x10FFFF, char::from_u32(99)=Some('c'), modifier=1 -> modifier_bits=0, passes harmless_locks/CTRL checks -> else-if emits UTF-8 bytes for 'c'. Currently unreachable due to early return.

## Fix applied (minimal, in proposed-v6)
Replace unconditional `return;` with explicit returns in the two known arms only:
```rust
if modifier == 1 {
    match codepoint {
        57350 => { self.emit(InputEvent::ArrowLeft); return; }
        57351 => { self.emit(InputEvent::ArrowRight); return; }
        _ => {}
    }
}
```

## Verification actually executed
- Source inspection only. Exact canonical file read (SHA confirmed); defect located and arithmetic verified (codepoint 99 not in private-use range, from_u32 succeeds, emits 'c').
- No Cargo/rustc/PTY/network/DB/Git mutation. Tests NOT run by author; proposed tests written but unexecuted.
- Proposed source + diff written to private review dir; sha256 computed after write.

## Artifacts and hashes
- Scratchpad: .../decoder-minimal-alternative/scratchpad.md
- Proposed v6 source: .../decoder-minimal-alternative/proposed-v6/native_input_decoder.rs
  sha256: 8abd2e4fdc6b0ac6bbe18aeb3afe874bc372be8a004628e6b8bb37001110ce70
- Proposed v6 diff: .../decoder-minimal-alternative/proposed-v6.diff
  sha256: 31a8c8370859b49340559f2f5eac3fef6b9b83c83e0ee5345ca83aac7558c059
- Canonical source SHA: 363e070883b89370e4443777a659283b91733b7872e728efedade9ef98a7c065

## Verdict
NEEDS_INTEGRATION. Defect confirmed (over-return drops all non-arrow Shift codepoints). Minimal, security-preserving fix applied in proposed-v6; 6 new tests added covering the previously-dropped codepoints and release/repeat/modifier-0 inerts plus existing-horizontal preservation. Not self-approved: must pass installed harness (frozen RED SHA 47680638...) on exact integrated SHA. Author is not the main security verifier.

## Remaining uncertainties and next steps
- Main security verifier owns focused + heavy validation; proposed v6 expected to turn Byte99 GREEN (ESC[99;1u -> 'c'); release acceptance requires running on exact integrated SHA.
- If /tmp/opencode path is restored by orchestrator, mirror artifacts to canonical private dir.
