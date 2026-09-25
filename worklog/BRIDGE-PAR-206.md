# BRIDGE-PAR-206 scratchpad

Claim: BRIDGE-PAR-206 via cc.claim session ses_par206. OK.
Source evidence: crates/cli/src/tui_entry.rs:638 `format!("memory: {} file(s) loaded", memory.len())`, guarded by `if !memory.is_empty()` (:637).
Observed scenario: transcript seed line only when memory non-empty.
Target boundary: ONE new file crates/opentui-bridge/src/mem_line.rs. No lib.rs, no Cargo.toml edits.
Tests: 5 unit tests in-file (none_zero, empty_vec, truth_match, single_entry, cap_128).
Decisions: format mirrors tui_entry truth verbatim; cap via chars().take(128); mem_lines delegates to mem_seed. Style follows toast_line.rs (sibling lane pattern).
Unknowns: none. Verification: rustfmt --check PASS.
