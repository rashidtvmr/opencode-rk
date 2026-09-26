# BRIDGE-PAR-412 (unclaimed, file-only)

Claim: none. Orchestrator owns tasks/completion/claims.json. Proceeded file-only per spawn orders.
Status: implemented, rustfmt pending.

Source evidence:
- TS truth packages/tui/src/util/presentation.ts:1-38 (wordmark/sessionEpilogue only, no truncate/count units).
- Style mirror crates/opentui-bridge/src/presentation.rs:23-35 (ellipsize char-safe) and present_util_full.rs:13-27 (truncate_middle).
- Boundary: new file only crates/opentui-bridge/src/present_ts_full.rs. No lib.rs/Cargo.toml/presentation.rs/present_util_full.rs edits. No cargo, no commit.

Target boundary: present_line(s,width)->String char-clip cap 512; present_count(n)->usize clamp 999. std-only, forbid(unsafe_code), under 60 lines.

Tests: 3 in-file (clip char-safe, cap512+zero, count clamp). Frozen on write.

Decisions:
- present_line = take(min(width,512)) chars; passthrough when short; "" on 0.
- present_count = n.min(999).
- ponytail note for lib.rs wiring left in file docs.

Unknowns: none. Integration wiring out of scope.
