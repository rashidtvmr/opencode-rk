# BRIDGE-PAR-423 (unclaimed, file-only)

Claim: skipped ledger per orchestrator override; file written without claim.
Source: files.tsx:14-52 (Modified Files diff list); sidebar.rs FileRow prior art.
Target: crates/opentui-bridge/src/side_files_full.rs, SideFiles only.
Tests: add_grows_len, bounds_reject, remove_bounds (inline).
Decisions: Vec<String> cap 32, path cap 512, remove(usize)->bool.
Unknowns: lib.rs wiring left to orchestrator.
