# BRG-FEED scratchpad

- Claim: `BRG-FEED` via `tools/completion_claims.py`, session `ses_brg_feed`. Status: in-progress.
- Owned file ONLY: `crates/opentui-bridge/src/feed.rs` (standalone, `rustc --test` compiled; no `crate::` imports).
- Source evidence:
  - `/Users/mymac/Projects/opentui/packages/core/src/NativeSpanFeed.ts` (375 lines, entire): `attach`/`create` register+attach handshake fails closed on status!=0 (L35-65); `drainOnce` FIFO over 256-slot drain buffer via `streamDrainSpans` (L288-364), skips `len==0`, resolves chunkMap/chunkSizes fallback, bounds-checks `offset+len`, sync handlers decrement refcount inline while async handlers pin chunk until `allSettled` (L330-344); `isBackpressured = pendingAsync>0 || pendingDataAvailable || hasPinnedChunks` (L125-127); `idle()` resolves when `!inCallback && !draining && pendingAsync==0 && !pendingData && !pinned` (L182-205); `finalizeDestroy` clears all maps/buffers/handlers (L167-180).
  - `renderer.ts` 1090-1155: feed pipes frame bytes to real sink, returned Promise keeps chunk pinned until write callback (async backpressure); 1493+: render reschedules after feed idle; 411 area: screen/IO mode setup (context only).
- Target boundary: std-only bounded queue discipline (`MAX_SPANS`=256 mirror, `MAX_BYTES`=256KiB), `forbid(unsafe_code)`, copy-before-ack in `drain_once` (clone front before pop). No FFI here; native wiring is integration job. `lib.rs` has no `mod feed` — NOT touching it (owned-file boundary); orchestrator wires.
- Tests (frozen, in-file `#[cfg(test)]`): attach/detach, fifo order, backpressure at cap, eviction bounded, idle signal, copy-before-ack, oversize rejected (7 tests).
- Decisions: `push` evicts oldest on cap (slow-consumer eviction, counters kept); single span > MAX_BYTES rejected `TooLarge`; empty push no-op (mirrors `len==0` skip); `detach` clears queue+pins (mirrors `finalizeDestroy`).
- Remaining: done — no commit/push per task order. RED: stub impl 0/7 FAIL (log `/tmp/opencode/red_build.log`); GREEN: final file 7/7 PASS, markers ×4, 253 lines, sha `5c073432`. Ledger `completed`.
