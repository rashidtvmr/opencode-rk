# PAR-008 wadapter — worklog

Claim: `crates/server/src/web_turn_adapter.rs` implements web turn-adapter
types mirroring real availability, with ingest/transmit and edit/run gates.

Source evidence (repo 5af7884):
- `crates/server/src/lib.rs:139-199` `web_capabilities`: tools/plugins/
  approvals `available_for_web_turn:false`; attachments `draft_ingest:true`,
  `available_for_web_turn:false`; search/deep_research/voice
  `available:false`; artifacts `available:true, editing_available:true,
  run/apply:false`.
- `crates/server/src/web_attachments.rs`: bounded draft ingest store.
- `crates/server/src/web_artifact.rs:400-411` `authorize_run`: executor AND
  grant required. `web_tool_chooser.rs` approvals gate tool use.
- `crates/server/src/voice_capture.rs:230-243`: no-adapter start fails.

Target boundary: OWNED FILE ONLY `crates/server/src/web_turn_adapter.rs`.
No `lib.rs` wiring (integrator owns shared routes). `#![forbid(unsafe_code)]`,
std only.

Tests (in-file `#[cfg(test)]`, 5 total):
- capability_matches_current_server_reality — report matches lib.rs flags.
- capability_report_not_hardcoded — all-up flags flip to Available.
- attachment_reaches_adapter_only_when_authorized — denied paths leave sink
  empty; authorized forwards exact digests.
- artifact_edit_version_allowed_but_run_apply_gated — edit OK on store;
  run/apply NoExecutor then Denied then Ok.
- unsupported_journeys_report_honest_unavailable — voice/search/research
  return non-empty adapter reasons; report agrees.

Decisions:
- `WebTurnAvailability::current()` encodes HEAD reality; `capability_report`
  derives every field via `gate()` — no literal success.
- Transmit uses permit type: sink unreachable without `TransmitPermit`.
- `validate_ingest` pure validator; `voice/search/research_status` return
  `FeatureStatus` as error so reason string reaches client.

Remaining unknowns: integration wiring into `lib.rs` router (integrator);
real provider/search/voice adapters (blocked, external authority).

Evidence:
- GREEN: `rustc --edition 2021 --test crates/server/src/web_turn_adapter.rs
  -o /tmp/opencode/wt && /tmp/opencode/wt` → 5 passed.
- RED: hardcoded-stub variant `/tmp/opencode/wt_red.rs`
  (sha256 abc2229d…) → `capability_report_not_hardcoded` FAILED, 4 passed.
- File sha256 48783262e7ecd829bdeb5b5510eb42709c5f33996946da2c026e1c95c3ba1f3c,
  516 lines.
