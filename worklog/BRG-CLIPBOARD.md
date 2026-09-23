# BRG-CLIPBOARD scratchpad

- claim: BRG-CLIPBOARD, session ses_brg_clip, ledger in-progress
- sources:
  - `/Users/mymac/Projects/opentui/packages/core/src/lib/clipboard.ts` (ClipboardService, HostClipboardBackend, TerminalClipboardAdapter, destinations terminal-only/host-only/best-available/all-available, validateClipboardText non-empty+NUL+byte-limit, selection clipboard|primary, ClipboardTarget Clipboard=0/Primary=1/Select=2/Secondary=3, OSC52 via native lib)
  - `host-clipboard.internal.ts` (DEFAULT_CLIPBOARD_MAX_BYTES=8MiB, validateClipboardText with surrogate check, normalizeSelection, limit-exceeded on read)
  - `host-clipboard.native.ts` (NativeClipboardBackend read/write/clear/dispose, track/drain)
  - `packages/native/src/terminal.zig` lines 163-175 (ClipboardTarget toChar c/p/s/q), 1654-1765 (clipboardSequenceSize, writeClipboard, writeClipboardSequence `ESC ] 52 ; <c> ; <base64> ST(ESC\)`, clear=empty payload, canWriteClipboard = osc52_support != unsupported, std.base64.standard)
  - `packages/native/src/renderer.zig` 3289-3312 (copyToClipboardOSC52/clearClipboardOSC52 bool)
- target: `crates/opentui-bridge/src/clipboard.rs`, std-only, forbid(unsafe_code), no spawn
- tests: null errors cleanly, OSC52 roundtrip, oversize reject, fallback order
- decisions: MAX_CLIP_BYTES=8MiB mirror; std-only base64 impl; fallback helper mirrors composeMutation
- RED: stub impl, expect failures. GREEN: full impl.
