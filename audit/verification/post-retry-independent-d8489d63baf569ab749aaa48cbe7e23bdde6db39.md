# Independent productionRetry candidate audit

**Verdict: GAP — not READY, and not product acceptance.**

## Scope and immutable references

- Authoritative base: `refs/heads/main-v2` = `8d5866e328306dbf8041d91bb41fed9a38c0971c` (also the checkout `HEAD` at audit time).
- Candidate: `d8489d63baf569ab749aaa48cbe7e23bdde6db39`; tree `dd618dfc3c1717ddc8a06807c149caab1dbf8a42`; parent commits `189fc21fc8143df0fcaee5d7e42797152c38e3cf` and `257c01e5dc2838d057fb115a2d7e25ae9ff73c81`.
- The final-result receipt is `partial`; the candidate ref/tag are present. This was a read-only audit; no production Cargo command, ref update, source edit, reset, deletion, prune, or push was performed.

## Accounting and graph evidence

- Raw accepted tips: **14**; newly accepted beyond the initial 10: **4** (`a881522a1986f6558ece4e5f00b9824a0a4808d1`, `5f35e070c85c36445e4532fc71d7c0372f33606f`, `6e796088dc1f522e87797960cb6fab7239adafde`, `257c01e5dc2838d057fb115a2d7e25ae9ff73c81`).
- Quarantined candidate tips: **207**; retry attempts: **211** (4 accepted + 207 quarantined). The run receipt also retains 205 prior quarantines and 207 unmerged child tips as separate historical/child metadata; these must not be added to the retry's unique quarantined-tip count.
- Expected frozen candidate set: 221. Thus 14 accepted + 207 retry quarantined = 221. Accepted set and retry-quarantined set are disjoint; all 14 accepted tips are ancestors of candidate `d8489d...` per its final proof. Retry order audit records 211 candidates, 1,173 related ancestor pairs, zero reversals, exact set/no duplicates. Upstream-lock blob remains unchanged.
- Accounting sources: `.../state/engine-retry-merge-run-20261001-20261001T213133-25441-final-pass-v1/final-result.json` and `.../audit/engine/L4/pair-00-final-accounting-v3-run2/result.json` under `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/merge-run-20261001-20261001T213133-25441/`.

## Four newly accepted tips: path/membership audit

- `a881522a1986f6558ece4e5f00b9824a0a4808d1` — installer archive fixture test repair: `crates/cli/tests/linux_native_installer_contract.rs`; associated contract/worklog `worklog/APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT.md`.
- `5f35e070c85c36445e4532fc71d7c0372f33606f` — permission live-test fixture compilation repair: `crates/server/tests/session_permission_live.rs`.
- `6e796088dc1f522e87797960cb6fab7239adafde` — bracketed-paste cleanup behavior in `crates/opentui-bridge/src/safe_renderer.rs`; associated contract/worklog `worklog/V2-NATIVE-BRACKETED-PASTE.md`.
- `257c01e5dc2838d057fb115a2d7e25ae9ff73c81` — live interrupt test fixture: `crates/server/tests/session_interrupt_live.rs`; associated `worklog/V2-LIVE-INTERRUPT-CONTRACT.md`.
- Candidate also contains source changes in `crates/cli/src/native_input_decoder.rs`, `crates/cli/src/tui_entry.rs`, and `crates/opentui-bridge/src/safe_renderer.rs`, with dependency edit `crates/opentui-bridge/Cargo.toml`.
- Test paths in the candidate: `crates/cli/tests/linux_native_installer_contract.rs`, `crates/cli/tests/startup_readiness.rs`, `crates/server/tests/session_interrupt_live.rs`, `crates/server/tests/session_permission_live.rs`, `tests/e2e/local_application.rs`, and `release/test_release_contract.py`. Contract/evidence documentation includes the four worklogs above plus `worklog/APP-012.md`, `worklog/APP-017.md`, `worklog/SHIP-001.md`, and `worklog/V2-LIVE-PERMISSION-CONTRACT.md`; `tasks/completion/claims.json` is claims prose, not acceptance evidence. No `crates/contracts/**` path changes.

## Protected paths

- `crates/providers/src/responses.rs` has identical blob `4ab7dca008fa838cb9fe73ac88caa6708c0f1262` at base and candidate.
- `crates/providers/src/responses_stream_semantics.rs` does not exist at either revision.
- `sources/upstream.lock.json` has identical blob `8c0ee629208924ba51e9607cfd00a5efca4319c5` at base and candidate.
- `git diff --check base..candidate` passed.

## Cargo gate: blocking evidence gap

The post-retry log exists and hashes to `1daba385922c8d4e07cebca76e7bc35930631a54dc02173922abcd18e4e7c475`; it reports success. It has **no adjacent receipt binding its command, manifest/workspace root, and source tree to candidate `d8489d...`**, so it is not valid source-bound Cargo evidence. The prior successful receipt binds Cargo check to different head `308fcdd1f31ef52256273710c2787b28cf3eafdc`, tree `127c052e17a8c7702034f7759fd85600ba35aaac`, and worktree `.../worktrees/final-combined-308fcdd-verify`; it cannot cover the retry candidate. No production Cargo rerun was authorized/performed here. Required gap closure: obtain a source-bound Cargo receipt for the exact candidate HEAD/tree with metadata manifest/workspace binding and clean before/after source checks, then reassess other gates.

## Packet fragments: classification and disposition

The nine one-line additions are opaque phase-1 package-transfer fragments, not identifiable source or test files. Their provenance is `origin/planning/phase1-package-transfer-20260921`; the salvage inventory records that branch as `PENDING_REVIEW`. No tracked product-code references were found. Since the packet payload has not been established as usable product, classify these as **unreviewed transfer/evidence artifacts**, retain the existing **PENDING_REVIEW** disposition, and do not import, decode, or count them as accepted product in this audit.

Exact candidate paths (with Git blob IDs):

| Path | Blob |
|---|---|
| `.phase1-package-transfer/part-02` | `e160b6bf8c3bb0319f11da986050acad5cecec46` |
| `.phase1-package-transfer/part-03` | `1f748a0a696b01a1aadda0733397733f2f5c007a` |
| `.phase1-package-transfer/part-04` | `61735ecb402c69da15c01fad97cbc77630d5981a` |
| `.phase1-package-transfer/part-05` | `6337f7fb83a87559c3313b2041a2cb05fce4fd25` |
| `.phase1-package-transfer/part-06` | `819e6eb3e8220b8c922019be711092d00edca251` |
| `.phase1-package-transfer/part-07` | `4cc25f85f7321e0e039323b3df98db3da00ebd45` |
| `.phase1-package-transfer/part-08` | `05f4bbb1149b21a420ad4a2a5ffa7e5df98a6b6c` |
| `.phase1-package-transfer/part-09` | `195659a020525340fe45814a43070c263196c6dd` |
| `.phase1-package-transfer/part-10` | `f63c83976f22622e1d32395d3c9703a388ff5ea8` |

## Candidate diff inventory

Other than the nine packet paths above, the candidate changes exactly:

```text
crates/cli/src/native_input_decoder.rs
crates/cli/src/tui_entry.rs
crates/cli/tests/linux_native_installer_contract.rs
crates/cli/tests/startup_readiness.rs
crates/opentui-bridge/Cargo.toml
crates/opentui-bridge/src/safe_renderer.rs
crates/server/tests/session_interrupt_live.rs
crates/server/tests/session_permission_live.rs
release/test_release_contract.py
tasks/completion/claims.json
tests/e2e/local_application.rs
worklog/APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT.md
worklog/APP-012.md
worklog/APP-017.md
worklog/SHIP-001.md
worklog/V2-LIVE-INTERRUPT-CONTRACT.md
worklog/V2-LIVE-PERMISSION-CONTRACT.md
worklog/V2-NATIVE-BRACKETED-PASTE.md
```

## Acceptance boundary

This is an independent audit finding, not a product acceptance record. The retry candidate remains **GAP / partial** until the exact-candidate source-bound Cargo gate and any remaining acceptance gates are completed and independently verified on the integrated revision.
