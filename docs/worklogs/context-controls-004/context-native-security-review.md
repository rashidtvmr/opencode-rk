# Native context UI security review

Verdict: **APPROVE** - source review only; not installed-product/release acceptance.

Reviewed six UI paths against base `e3069eee04d9a3414517b2452df803c9c145c9d3`:

- `crates/cli/src/main.rs` `6a94374624219d3bf8dc99b6503420208cbb2f81c427f90be99bb482958492d1`
- `crates/cli/src/native_view.rs` `15533f73bd04a9627f908c5f4687ab5966de6352a1b7c2fb44e8037a775658a8`
- `crates/cli/src/tui_entry.rs` `945309c21ccbab8d3a00926eb821eaddaea424e1b9b2e069cb112d1835bbac74`
- `crates/cli/src/native_context.rs` `9bd943dbe32650cbc728f5991e6d02d3fa3454a82d0312f3b8bcf31da5064ad5`
- `crates/cli/src/native_context_tests.rs` `7b45c9140b6d77fa4e26b6a7fc8538755897dfa4e6d9936902be739f505bc7b5`
- `crates/cli/src/native_context_security_tests.rs` `d8903f585fc9dd8a91651089da6be7e7234315eb525b56c2a4ad9a9c4755b82e`

**Prior findings resolved.** Parser rejects selectable system, memory, checkpoint-represented, mandatory and reason-marked compacted items; enforces kind/role consistency. Per tool-record payload cap is 4096 bytes, independently of aggregate 112 KiB. New tests confirm atomic rejection/state retention and 4096 accepted/4097 rejected.

Previously reviewed controls remain: canonical loopback/session/cursor and fixed paths; shared validated bearer helper; exact scoped selection ACK before GET; no optimistic mutation; bounded atomic parsing; invalidation clears metadata; no checkpoint window payload in response display; memory points to `/memory`; display sanitizes controls and wraps bounded scrollable metadata.

**Evidence:** `context-native-security-frozen-red.json` matches current test SHA; RED records both findings failing. `context-native-security-green.log`: 769 passed, 0 failed. No tests run by reviewer. Backend mutation/installed journey acceptance remains separate.
