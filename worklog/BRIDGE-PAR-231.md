# BRIDGE-PAR-231

Claim: ledger in-progress, session ses_par231.
Evidence: run_types.rs:40 StreamCommit{id,text}+validate 1024B; run_stream.rs:15 StreamCommit{id c-<n>,text}+StreamBuf 64KiB cap.
Boundary: new file only stream_commit_unify.rs; no lib.rs/Cargo.toml/run_types/run_stream edits; no cargo/commit.
Tests: 5 in-file (role cap, body cap, line cap, final flag, verbatim).
Decision: char-based caps (32/64KiB/512), std-only, forbid(unsafe_code).
Verify: rustfmt --check PASS.
