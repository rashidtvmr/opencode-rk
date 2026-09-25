# TURN-RED scratchpad

Claim: TURN-RED, session ses_f3c4de578ffelQv59xDXmOs03B.

Base: `7262682e31c1f473912c9483804c9430eb4ee4c5` on
`red/PHASE1-LIVE-TURN`.

Source evidence:

- Current state machine, `crates/server/src/turn_service.rs:247-465`, already
  provides bounded in-memory Streaming/AwaitingApproval/Executing/Settled/
  Cancelled/Uncertain transitions, denial receipts, cancellation, safe retry,
  and ambiguous-effect refusal. Unit tests at `:520-765` cover those isolated
  transitions.
- Real stream caller evidence in
  `worklog/PHASE1-VERTICAL-PRODUCT-MACOS-MAP.md:39-42` says
  `create_turn_stream` bypasses that state machine when broker authorization
  returns `RequireHuman`, emits terminal error text, and has no durable approval
  record/channel/resume caller.
- The approved product outcome at the same map `:161-171` requires the real
  stream to create durable AwaitingApproval, allow a second authenticated client
  to observe it, deny with zero side effect, approve/resume exactly once, and
  avoid replay after disconnect.
- That outcome's RED path is `crates/server/tests/macos_approval_resume.rs`, but
  synthesis renames it to `crates/server/tests/phase1_live_turn.rs` without
  defining a replacement public route/API.
- Synthesis assigns implementation only to `turn_service.rs`; actual route and
  stream wiring belongs to a later serialized `server/src/lib.rs` stage and the
  approval coordinator remains behind `G-APPROVAL-PREWIRE`.

Status: BLOCKED before test authoring. Public `Turn` calls can only duplicate
already-GREEN unit behavior. The real caller exposes no compiling endpoint to
list, approve, deny, or resume a pending streamed turn, no durable approval ID
schema/opaque representation, and no atomic claim seam. Inventing those imports
or routes would be a compile failure or test-owned protocol, not behavioral RED.

Required authority action: complete `G-APPROVAL-PREWIRE` with the coordinator,
durable state/ID contract, bounded observation channel, principal/origin and
expiry semantics, atomic approve/deny claim, and real `create_turn_stream`
caller wiring. Then freeze a black-box RED against that compiling seam with a
fake bounded grant issuer and disposable storage, asserting one effect and zero
side effects on deny/disconnect ambiguity.

No test hash was frozen. No product, test, schema, manifest, controller, or
verifier file was changed. This blocker keeps `V1-FREEZE-RED` open.
