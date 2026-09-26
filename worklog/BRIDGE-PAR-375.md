# BRIDGE-PAR-375 scratchpad (UNCLAIMED - orchestrator owns claims.json, no claim made)

Claim: new file crates/opentui-bridge/src/ctx_directory_full.rs, pure dir label + branch.
Source: packages/tui/src/context/directory.ts:7-17 (useDirectory memo); runtime.tsx:3-8 (abbreviateHome).
Observed: directory=project.path.directory||cwd; out=abbreviateHome(dir,home); branch? out+":"+branch.
Target: dir_label(dir,home)->String tilde cap 128; dir_with_branch(dir,branch)->String cap 192. std-only, forbid unsafe, <80 lines, >=3 tests.
Tests: tilde, passthrough, branch, caps (4 tests).
Decisions: boundary-safe prefix strip mirrors path.relative child check; trim trailing slash; char-count caps; empty dir fail-closed to "".
Unknowns: none. No lib.rs/Cargo.toml edits. No cargo per scope; rustfmt --check only.
