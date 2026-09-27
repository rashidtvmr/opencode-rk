# APP-001-CLI-PROVIDERS-LOCK-W1
claim: ses_f1d348372ffeJh1UMgKYL3i9FL in-progress
base: origin/prewire/APP-001-CLI-PROVIDERS-CLEAN-W1 5117d76ef0cf105881da7d140732ea083f7721a6
boundary: Cargo.lock only, no manifest/source/test edits
obs: cli manifest crates/cli/Cargo.toml:27 path dep opencode-rk-providers; Cargo.lock cli deps lack it
plan: cargo check -p opencode-rk-cli --bin oc2 refresh; diff-check; --locked rerun; commit/push
check: CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-cli --bin oc2 -> Finished dev, 459 pre-existing warnings, 0 errors
diff: Cargo.lock +1 line "opencode-rk-providers" under opencode-rk-cli deps; no version churn
locked: CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-cli --bin oc2 --locked -> Finished dev 0 errors
reject: no other product changes (git diff --name-only = Cargo.lock)
