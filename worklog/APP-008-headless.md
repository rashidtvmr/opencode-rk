# APP-008-headless worklog

Claim: headless-engine bridge types in owned file only.
Source evidence:
- repo 5af7884, crates/cli/src/run_headless.rs:1-19 (HEAD-001 bounds, no FS except spill dir),
  :49-54 (RunExit code u8), :63-66 (ModelPort), :211-216 (Refused->1, Failed->2).
- crates/cli/src/session_export.rs:1-5 (caller traits, single sink write), :53-61 (typed errors).
- crates/cli/src/native_transcript.rs:1-5 (std only, bounded), :42-49 (ToolState).
- card APP-008 via tools/completion_plan.py: T01 same normalized transcript, T02 exit
  codes, T03 capability passthrough, T04 typed failures, T05 disconnect Abort/Detach.
Observed scenario: created crates/cli/src/headless_engine.rs with RED tests (5 tests),
  ran rustc --test -> 5 FAILED (stubs returned input/false/None). Implemented real code,
  reran -> 5 passed.
Target boundary: owned file crates/cli/src/headless_engine.rs only. No lib.rs wiring,
  no edits to run_headless.rs/session_export.rs. Integrator wires module.
Tests:
- normalized_transcripts_match_across_clients (T01)
- normalization_trims_edges_and_controls (T01 edge)
- exit_codes_meaningful (T02: distinct codes, round-trip, non-empty meaning)
- capability_requires_passthrough_marker (T03)
- disconnect_follows_selected_contract (T05)
Decisions: ExitCode 0..5 Success/Usage/Internal/NotFound/Unauthorized/Unavailable;
  normalize: CRLF/CR->LF, ANSI CSI + lone ESC strip, drop controls except LF/TAB,
  rtrim spaces/tabs per line, cut edge blank lines, preserve interior blanks, 1MiB
  char-boundary cap; capability: passthrough marker required, hardcoded live_probed=false
  rejected; disconnect: pure Abort->Aborted / Detach->Detached mapping.
Remaining unknowns: integration wiring into run_headless/native TUI owned by integrator;
  T04 typed failure mapping across clients lives in server/slice integration.
