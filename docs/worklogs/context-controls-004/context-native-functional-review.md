# Context Controls 004 — Independent Native Functional Review

**Candidate base:** `e3069eee04d9a3414517b2452df803c9c145c9d3`
**Resolution verdict:** **APPROVE**

## Reviewed paths and exact SHA-256

| Path | SHA-256 |
|---|---|
| `crates/cli/src/main.rs` | `b5d8d88912e6f45a85b2a5ba09e212d7e32bd9ff0e0bda58acffd2dff3a87f29` |
| `crates/cli/src/native_view.rs` | `15533f73bd04a9627f908c5f4687ab5966de6352a1b7c2fb44e8037a775658a8` |
| `crates/cli/src/tui_entry.rs` | `945309c21ccbab8d3a00926eb821eaddaea424e1b9b2e069cb112d1835bbac74` |
| `crates/cli/src/native_context.rs` | `e990a33d0f88beafe9974c4687dbc30fd211a7d27bfcefebc8f3895e73e7305c` |
| `crates/cli/src/native_context_tests.rs` | `7b45c9140b6d77fa4e26b6a7fc8538755897dfa4e6d9936902be739f505bc7b5` |


## Prior finding and resolution

The prior review correctly found that long checkpoint metadata and selected source attribution were clipped by final `fit()`, because metadata was emitted as single rows and item detail omitted `source`.

The candidate now resolves this: `render` wraps every metadata row at the viewport width before detail scrolling, and `item_detail` wraps complete `Source:` and `Identity:` lines before content and tool-record headers/payloads. The added narrow-width test uses a long nested source and 180-character provider/model values, scrolls repeatedly, and verifies both source and model suffixes remain reachable.

The wrapping path is bounded: item/source/record fields are parsed under their existing limits, `wrap` uses the bounded display visitor, and final `fit` remains a safety bound. `f/b` changes `detail_offset`, so wrapped rows are navigable rather than merely clipped.

No remaining blocking functional defect was found in the five reviewed paths. Earlier transport, ACK-before-GET, canonical validation, atomic parsing, paging, selection, mandatory-history, checkpoint indivisibility, and session invalidation findings remain satisfied.

**Review status: APPROVE**, subject to the parent’s separately recorded final batch verification; this is an independent source review, not a claim that an unobserved command passed.

## Security follow-up candidate

**Current six-path verdict: APPROVE.** The new security model and tests were independently reviewed rather than inheriting the prior five-path approval. `load_response` now rejects contradictory selection authority for system, memory, compacted, mandatory, and checkpoint-indivisible entries; validates kind/role pairings; bounds record payload previews to 4096 bytes; and commits only after complete parsing, preserving prior state on rejection. The added security test module covers these cases and atomicity. Existing viewport wrapping remains intact.

Current six-path SHA-256 manifest:

| Path | SHA-256 |
|---|---|
| `crates/cli/src/main.rs` | `6a94374624219d3bf8dc99b6503420208cbb2f81c427f90be99bb482958492d1` |
| `crates/cli/src/native_view.rs` | `15533f73bd04a9627f908c5f4687ab5966de6352a1b7c2fb44e8037a775658a8` |
| `crates/cli/src/tui_entry.rs` | `945309c21ccbab8d3a00926eb821eaddaea424e1b9b2e069cb112d1835bbac74` |
| `crates/cli/src/native_context.rs` | `9bd943dbe32650cbc728f5991e6d02d3fa3454a82d0312f3b8bcf31da5064ad5` |
| `crates/cli/src/native_context_tests.rs` | `7b45c9140b6d77fa4e26b6a7fc8538755897dfa4e6d9936902be739f505bc7b5` |
| `crates/cli/src/native_context_security_tests.rs` | `d8903f585fc9dd8a91651089da6be7e7234315eb525b56c2a4ad9a9c4755b82e` |

The final native/security command is reported separately by the parent; this review does not convert its pending status into acceptance.
