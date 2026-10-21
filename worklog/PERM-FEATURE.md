# PERM-FEATURE — approval popup + /permission command (user-directed)

Claim: user-directed repair, not a plan slice. No plan task card exists for the
popup accessibility/always-allow request or `/permission`; the nearest planned
surface is `docs/plans/phase1/tasks/P1-W2-06.md` (DRAFT, not activated), which
names `web/src/components/ApprovalDialog.tsx` and `crates/cli/src/native_approvals.rs`.
This work is deliberately CLI/TUI: the web composer reports "Slash commands ·
native endpoint unavailable" (`web/src/components/composer.tsx:464`) and the web
turn path has no approval bridge (`crates/server/src/lib.rs:222-224`), so a web
popup could not be wired to real approvals.

Source evidence:
- `crates/cli/src/native_approvals.rs:1-360` — pure ApprovalBoard: pending queue,
  digest+scope binding, two-step guard for Destructive/HumanOnly, bounded history.
- `crates/sessions/src/ui_008.rs:1-42` — ApprovalCard allow/deny only, no always-allow.
- `crates/cli/src/chat.rs:135-479` — REPL slash dispatch (/help,/model,/models);
  daemon spawn at :542.
- `crates/tools/src/permission.rs:57-110` — deny-unless-authorized gate; `*` rejected,
  reserved namespaces never grantable.
- `crates/foundation/src/config.rs:97-113` — PermissionMode (Ask default per domain).
- `crates/server/src/lib.rs:839-842` — `OPENCODE_RK_TURN_TOOLS` comma-separated allowlist.

Target boundary:
1. `crates/cli/src/native_approvals.rs`: numbered popup state + render + key/mouse
   handling, accessible `announce()`, AlwaysAllow grant set (human-only excluded,
   destructive two-step preserved, bounded MAX_GRANTS, scope+policy+expiry bound).
2. `crates/cli/src/permission_command.rs`: `/permission [list|allow|disallow|reset]`,
   deny wins, wildcard/reserved/invalid names rejected, bounded sets, atomic
   `permissions.json` persistence, fail-closed load.
3. `crates/cli/src/chat.rs`: dispatch + help + spawn env wiring.

Lanes:
- Lane A: glm-5.3-flash-free, owns native_approvals.rs (scratchpad worklog/PERM-POPUP.md).
- Lane B: deepseek-v4.1-flash-free, owns chat.rs (scratchpad worklog/PERM-COMMAND.md).
- Orchestrator: permission_command.rs + main.rs mod, integration, frozen-test rerun.

Remaining unknowns:
- `cargo test` currently fails before compiling: `rustc -vV` exit 1,
  "the 'rustc' binary ... is not applicable to the 'stable-aarch64-apple-darwin'
  toolchain". Toolchain repair required before GREEN can be claimed.
- Popup has no live TUI render loop yet (`chat.rs` is line-oriented); this slice
  delivers the tested popup model + renderer that the TUI host can call.
