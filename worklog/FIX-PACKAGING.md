# FIX-PACKAGING — post-copy identity gate (shell mirror)

Claim: ses_fix_pkg, scratchpad worklog/FIX-PACKAGING.md.
HEAD 62f7eb1ba1b744604762cd022118390784be8d41.

## Source evidence
- crates/cli/src/install_commands.rs:29 BINARY_NAME=oc2
- crates/cli/src/install_commands.rs:31 LEGACY_BINARY_NAME=opencode-rk
- crates/cli/src/install_commands.rs:176-178 packaged_output_names_oc2 = contains(oc2) && !contains(opencode-rk)
- scripts/install-oc2.sh:99-121 pre-existing --version gate (APP-010); missing --help gate
- crates/cli/tests/packaging_identity.rs frozen (aliases/download/stale refs); out of lane scope, untouched
- REVIEW_ITERATION_2.md:1674/1707-1714 minimal fix: verify --version/--help post-copy, exit 74

## Observed scenario
- install-oc2.sh already: checksum gate pre-write, legacy opencode adjacency refusal exit 73, post-copy --version gate exit 74 + rm bad binary.
- Gap vs SUCCESS: no --help gate. A binary with clean --version but legacy --help would pass.

## Target boundary
- Owned file ONLY: scripts/install-oc2.sh.
- Add post-copy --help identity gate mirroring --version gate + Rust predicate.
- Fail-closed: mismatch -> rm installed binary, exit 74. No Rust/test/commit/push.

## Tests
- bash -n scripts/install-oc2.sh
- cargo test -p opencode-rk-cli --bin oc2 install_commands
- Zero frozen-test edits.

## Decisions
- Mirror exact two-case structure (reject opencode-rk, require oc2) for --help.
- Run gate on installed copy (covers bad copy), same exit 74 + rm semantics.
- Keep POSIX sh, quoted expansions.

## Remaining unknowns
- None for this slice. Full FIX-PACKAGING (--aliases/download/validate-release) owned elsewhere.

## GREEN evidence (2026-09-20, rev 62f7eb1)
- `bash -n scripts/install-oc2.sh`: OK (also via rtk prefix).
- `cargo test -p opencode-rk-cli --bin oc2 install_commands`: 5 passed 0 failed (297 filtered), exit 0. Tail: `test result: ok. 5 passed; 0 failed`.
- Functional stubs (disposable /tmp/opencode/fixpkg): T1 good stub exit 0; T2 bad --version (opencode-rk) exit 74 + binary removed; T3 clean --version + legacy --help exit 74 + binary removed. New T3 fails pre-fix script (no --help gate), passes post-fix.
- Zero frozen-test edits; no Rust/test/commit/push.
