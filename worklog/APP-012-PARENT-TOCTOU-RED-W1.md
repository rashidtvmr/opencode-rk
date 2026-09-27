# APP-012-PARENT-TOCTOU-RED-W1 Scratchpad

## Claim
Task: APP-012-PARENT-TOCTOU-RED-W1
Session: ses_f1dfb40b8ffedbfukYQM6uYy5B
Status: blocked (RED test written, needs implementation)
Owned file: crates/tools/src/file_ops.rs (test-only additions)

## Source Evidence
Commit: 5d07e20 (seam commit, branch red/APP-012-PARENT-TOCTOU-W1)
File: crates/tools/src/file_ops.rs
Seam: execute_with_preopen_hook at line ~212, fires after path_has_symlink_component + fs::create_dir_all, before open_and_write
open_and_write: line ~291, uses path-based fs::OpenOptions with O_NOFOLLOW on leaf only

## Observed Scenario
Hook atomically renames intermediate parent dir aside, replaces it with symlink to outside dir.
path_has_symlink_component already passed (checked real parent).
open_and_write resolves through swapped parent symlink, writes ATTACK_PAYLOAD into outside sentinel file.
Sentinel bytes changed from SENTINEL_UNCHANGED to ATTACK_PAYLOAD -- proven escape.

## Target Boundary
Contract: write_file must fail/deny before any I/O through a swapped parent directory.
Fix requires fd-relative open or re-validation of parent components after hook/seam boundary.

## Tests
New test: write_file_parent_toctou_symlink_swap_must_not_escape (#[cfg(unix)])
New hook: hook_swap_parent_to_symlink (#[cfg(unix)])
Existing tests: 12/12 pass when new test is skipped

## Decisions
Deterministic fixture paths derived from hook's path argument (no globals, no closures).
outside_dir = grandparent.join("outside_escape_dir"), sentinel = outside_dir/leaf_name.
No threads, sleeps, FUSE, privileges, or unsafe.

## Remaining Unknowns
Implementation approach for GREEN (fd-relative open vs re-check).
HOOK_SHA=c792c88f93de005e7368c1697f806955d2ae371c2ea4b96b5077f1b1be71c906
TEST_SHA=48f866866b12685df3b99df99e99e148ad58a3cd3fc3b72a574d830631456c12
