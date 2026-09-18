# APP-011-diag worklog

Claim: `crates/cli/src/diagnostics.rs` implements diagnostics types slice.
Commit: 5af7884. Only owned file created; no other edits.

Source evidence:
- `crates/cli/src/main.rs:218-256` doctor builds DiagnosticReport + checks inline.
- `crates/contracts/src/lib.rs:311-323` CapabilityReport/DiagnosticReport.
- APP-011 card (`tools/completion_plan.py --card APP-011`) T03/T04/T05 map to
  last-valid fallback, redacted export, zero-workers-when-disabled.
- `crates/server/src/runtime_settings.rs` absent (sibling lane owns it).

Target boundary: owned file only. `#![forbid(unsafe_code)]`, std only
(BTreeMap), no deps. Bounds: MAX_EXPORT_FIELDS=128, value 8KiB,
transcripts 256.

Tests (frozen in-file, RED=missing module, GREEN shown below):
- invalid_config_keeps_last_valid / validator_closure variant.
- export_redacts_secret_by_default.
- export_excludes_transcripts_unless_explicit.
- disabled_service_reports_zero_workers.
- config_precedence_cli_beats_env_beats_file.

Decisions: ProbeKind provider/daemon/tools; Precedence default<file<env<cli;
LastValid.try_update(+_with); ExportOptions default excludes both.

Unknowns: wiring into doctor()/daemon UI left to integrator (out of lane).
