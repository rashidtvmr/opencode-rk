# BRIDGE-PAR-126

Claim: session ses_par126, scratchpad worklog/BRIDGE-PAR-126.md.
Source evidence: packages/tui/src/routes/session/permission.tsx:20 PermissionStage = "permission"|"always"|"reject"; :115-117 store stage init "permission".
Target boundary: new file crates/opentui-bridge/src/permission_view.rs only. No lib.rs/Cargo.toml edits.
Tests: 6 in-file #[cfg(test)]: default ask, cycle order, wrap, tool trunc 64, note trunc 512, labels.
Decisions: Ask=permission (label "ask"). advance always true. char-count trunc. std-only, forbid(unsafe_code).
Unknowns: none.
