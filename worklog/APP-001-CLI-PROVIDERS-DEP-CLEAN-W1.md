# APP-001-CLI-PROVIDERS-DEP-CLEAN-W1
Claim: ses_f1d3ebf5cffen1x70lfx5n7Xp9, ledger in-progress -> completed.
Own: crates/cli/Cargo.toml only. Base: f22e0e8 (File0600 receipt).
Context: recovery of rejected APP-001-CLI-PROVIDERS-DEP-W1 (product e068f0b ok;
claim/worklog 7eecdd0 landed on unrelated release branch = mixed landing).
Evidence: e068f0b diff = 1 line, opencode-rk-providers path dep inserted
alphabetical between contracts and sessions, crates/cli/Cargo.toml:27.
Applied: `opencode-rk-providers = { path = "../providers" }` same style/order.
Cycle check: DFS over crates/*/Cargo.toml path deps, cli cycle: None.
providers/Cargo.toml deps = contracts, security only; no cli ref anywhere.
Validation: `cargo verify-project` success:true; git diff vs e068f0b on owned
file empty (identical). Cargo.lock auto-touched by cargo metadata probe,
reverted; not committed. No features, no source/test/root-manifest edits.
Commands: rtk cargo verify-project; python3 DFS cycle scan; git diff e068f0b -- crates/cli/Cargo.toml.
Unknowns: none. Branch prewire/APP-001-CLI-PROVIDERS-CLEAN-W1 pushed; merge to main = orchestrator.
