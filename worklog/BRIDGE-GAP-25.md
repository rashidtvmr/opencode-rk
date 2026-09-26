# BRIDGE-GAP-25

- Claim: `ses_gap25`, session-owned.
- Source evidence: pinned OpenCode commit `95daf90670b7c039c436c85537da5fbfe2205b41`; `packages/opencode/src/cli/cmd/run/permission.shared.ts` inventory line 2692; `packages/opencode/src/cli/cmd/run/footer.permission.tsx` inventory line 2685; upstream test inventory line 3072. Blob objects are not present in this local Git object store.
- Contract: native `Ask`, `Always`, `Reject`; prompt approval is one-shot; deny becomes terminal reject; `Always` bypasses prompt decisions; reject notes bounded to 512 Unicode scalar values; empty tool names fail.
- Owned file: `crates/opentui-bridge/src/run_permission.rs` only.
- Tests: six inline tests: ask approve, ask deny, always bypass, terminal reject, Unicode note truncation, empty tool.
- Decision: approval does not silently persist `Always`; caller selects `Stage::Always` explicitly.
- Unknown: integration registration into `lib.rs` is outside leased scope.
