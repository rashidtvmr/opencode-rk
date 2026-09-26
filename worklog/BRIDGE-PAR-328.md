# BRIDGE-PAR-328

Claim: attempted via completion_claims.claim (session ses_par328). Ledger
claims.json holds 501 rows (dict with Bounded? no, MAX_ROWS=500 exceeded),
load_ledger raises ClaimError "claims must be a bounded mapping". Blocked
from claiming; proceeding per lane orders to deliver owned file only.
No lib.rs/Cargo.toml edits, no cargo, no commit.

Source evidence:
- /home/rashid/projects/opencode/packages/tui/src/component/dialog-workspace-create.tsx:1 `WorkspaceSelection::new` workspaceName entry
- Pattern: crates/opentui-bridge/src/fork_dialog_full.rs:1-59 (forbid unsafe, consts, struct, setters, confirm/submit, in-file tests)

Target boundary: crates/opentui-bridge/src/dialog_ws_create_full.rs only.
WorkspaceCreate {name cap128, path cap512, done} + set_name + set_path +
submit(&mut self)->Option<String> (done, None on blank name). std-only,
forbid(unsafe_code), under 100 lines.

Tests: 5 in-file (name trunc, path trunc, blank submit None/not-done,
ok submit Some+done, blank-path still ok).

Verification: rustfmt --check only (no cargo per scope).
