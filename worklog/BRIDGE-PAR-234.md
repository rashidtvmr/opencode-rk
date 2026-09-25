# BRIDGE-PAR-234
claim: ses_par234
src: dialog-fork-from-timeline.tsx:12 DialogForkFromTimeline option list; fork_dialog_full.rs ForkPick/ForkDialogFull (read-only)
boundary: ONE new file dialog_fork_full2.rs; no lib.rs/Cargo.toml/fork_dialog_full.rs edits; no cargo/commit
tests: 6 unit tests in-file; rustfmt --check PASS; 106 lines
decision: cursor-list ForkPick distinct from ForkDialogFull pick-confirm gate
