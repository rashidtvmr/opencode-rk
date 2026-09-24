# APP012-PROTECTED-PATH-VERIFY

## Claim
- Task: `APP012-PROTECTED-PATH-VERIFY`
- Session: `ses_f2b570c08ffeqHpagV0MtJ03Db`
- Branch: `verify/APP012-PROTECTED-PATH`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/verify-app012-protected-path`
- Owned artifact: `worklog/APP012-PROTECTED-PATH-VERIFY.md` + own ledger row only. No source/test edits.
- Candidate revisions: broker `7463dbe74980b9a0098d245a275d2d03728925e5` (BLOCKED partial), fileops `c2697156e3078b4dff50cb8093bd502840c12a4c` (HEAD, completion).
- Frozen RED base: `dc1a84d17b05daa001225e2e94dd9a17c47b86fe`.
- Scope: containment + redacted denial only. Hardlink identity + descriptor-bound authorize/open TOCTOU explicitly OPEN; parent APP-012 cannot close.

## Source evidence
- `crates/security/src/lib.rs:257-295` authorize_file: secret deny fixed reason, system Read deny fixed reason, write-outside human gate, delete human gate, Read canonical containment deny. Pre-existing write/human ordering unchanged.
- `crates/security/src/lib.rs:352-365` is_within_readable_root: fs::canonicalize(path) fail-closed; roots = project_roots + explicitly_allowed_roots; root canonicalize fail maps to `lexical_normalize(root).as_os_str().len() == 0` (empty-normalized root only).
- `crates/tools/src/file_ops.rs:181-217` execute_authorized: Read (+List via Read mapping) Deny -> fixed `"file read denied"` literal (no broker reason passthrough); non-read Deny keeps `file operation denied: {reason}`; RequireHuman unchanged.
- `crates/tools/src/file_ops.rs:220-266` execute/read_file: raw `fs::File::open(path)` post-authorize, no descriptor binding (TOCTOU OPEN).
- `crates/server/tests/app012_protected_path_red.rs:33-47` assert_denied: success=false, content="", error==Some("file read denied"), one audit, Deny decision.
- Contract: `worklog/APP012-PROTECTED-PATH-CONTRACT.md:117-167`; contract-verify `...-CONTRACT-VERIFY.md:127-185`; RED `...-RED.md:1-56`; broker GREEN `...-GREEN.md:1-129`; fileops GREEN `...-FILEOPS-GREEN.md:1-67`.

## Target boundary
- Containment: canonical outside/traversal/symlink/broken deny, fail-closed on resolution error. Secret/system mandatory deny before permission evaluation. One audit per attempt. system_readable=true /etc/hosts Allow preserved. Read-class fixed path/reason/bytes-free denial. Non-read formatting unchanged. No policy expansion.
- Explicitly OPEN: hardlink alias identity (canonicalize cannot detect same-inode alias; no nlink/identity check), descriptor-bound authorize-then-open TOCTOU (authorize path, open raw path separately).

## Tests (sole slot, sequential, jobs1/threads1)
- Frozen protected: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_protected_path_red -- --test-threads=1` -> 7 passed, 0 failed.
- Security lib: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-security --lib` -> 148 passed, 0 failed.
- Frozen journey: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_tool_journey_red -- --test-threads=1` -> 1 passed, 0 failed.
- file_ops module: `cargo test -p opencode-rk-tools --lib file_ops` -> 5 passed, 0 failed.
- tool_allow: `cargo test -p opencode-rk-tools --test tool_allow -- --test-threads=1` -> 5 passed (alw_t01-t05), 0 failed.
- tool_audit: `cargo test -p opencode-rk-tools --test tool_audit -- --test-threads=1` -> 5 passed (tad_t01-t05), 0 failed.
- cargo check: `CARGO_BUILD_JOBS=1 cargo check -p opencode-rk-tools -p opencode-rk-security -p opencode-rk-server` -> 0 errors (4 pre-existing server lib warnings only).
- Full tools lib (informational): 110 passed, 1 failed: `mcp_spawn::tests::disc111_t04_crash_bounded_retry_no_silent_restart` -> `SpawnFailed("No such file or directory (os error 2)")` at mcp_spawn.rs:752. Fixture hardcodes `/bin/false`; host has no `/bin/false` (verified `ls: /bin/false: No such file or directory`; only `/usr/bin/false` exists). Unrelated to APP012 diff: no APP012 file touches mcp_spawn; `git status --short -- crates/tools/` clean; fixture path predates candidate (mcp_spawn.rs last touched c1866be). Classified host-fixture failure, not caused by candidate. No baseline stash run needed; no test edit made.

## Integrity
- Frozen protected RED sha256 `ed524ec1a239e8403c76d133e6929683a5a751f758f662b91b0a8d5a2b687d77` unchanged.
- Frozen journey sha256 `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` unchanged.
- security/src/lib.rs `abcc41205734a42f2a72c7689e5556194654a8cceee2c353247a51bbb4644e00` (inherited candidate, untouched by verifier).
- tools/src/file_ops.rs `a98675aac6edb6055e58b32c7aaa4e77a9ac477bebd79f51ec04465336bbd1d1` (completion, untouched by verifier).
- Zero test edits by verifier. Source scope: none (worklog + own ledger row only). `git diff --check` clean; `git diff HEAD --stat` shows only claims.json row (+5).
- Survivors: `ps aux | grep -cE "cargo|rustc"` -> 2 (grep self-count only, no leaked workers).
- Memory: Pages free 20239 x 16KiB (~316 MiB free at check; post-check sequential slot released, no parallel Cargo).
- validate_repository: FAIL backlog exhaustion exit=1, pre-existing (stale EXT/INT ownership-gap rows, unrelated to APP012).

## Audit (both one-file diffs)
- Canonical path/root bounds: `is_within_readable_root` canonicalizes target, fail-closed on any error (broken/permissions/missing/traversal escape); roots = project_roots + explicitly_allowed_roots, each canonicalized. Symlink/traversal/outside denied at broker; broken symlink denies (not fs error leak). Symlink-to-.env doubly denied (secret check first).
- Secret/system ordering + non-bypass: secret check (lib.rs:259) and system check (lib.rs:264) run before permission evaluation in authorize(); broker GREEN confirms mandatory deny survives star/human-shaped allow. Reasons fixed to `"file read denied"`. System Read Allow only when `system_readable=true` (`/etc/hosts` compatibility test passes); system Read deny otherwise fixed; non-read system deny keeps distinct reason (compatibility).
- One audit: frozen `assert_denied` requires `audit_len()==1` + Deny decision per attempt; 7/7 green proves single-denial wiring through execute_authorized.
- Fixed read denial mapping: Read (+List via Read) Deny -> literal `"file read denied"`, path-free, reason-free, bytes-free (content=""), no broker pattern leak. Non-read Deny keeps `file operation denied: {reason}` (compatibility, out of scope). No policy expansion: secret/system classifiers untouched; writable-root/human gates untouched; `*` cannot lift mandatory deny.
- Availability/platform regression note: canonicalize requires target existence/readability; missing/unreadable target denies (fail-closed, intended). Roots must exist and be canonicalizable or reads deny: root canonicalize failure maps to empty-normalized-root-only match, so a missing/unreadable project root denies all reads. This availability behavior is OPEN (not covered by frozen RED; no missing-root test). No macOS regression observed: symlink/broken/traversal cases pass on this host; hardlink/TOCTOU remain OPEN as below.

## Verdict: scoped ACCEPT (containment + redacted denial only)
- ACCEPT: canonical containment (outside/traversal/symlink/broken deny, fail-closed), secret/system mandatory ordering with star/human non-bypass, system_readable compatibility, one-audit denial, exact fixed read denial with no path/reason/bytes leakage, non-read compatibility, no policy expansion.
- Explicitly OPEN (not accepted, parent APP-012 cannot close): hardlink alias identity (canonicalize blind to same-inode alias; no nlink/identity check anywhere on read path); descriptor-bound authorize/open TOCTOU (authorize path then raw `fs::File::open(path)` separately in file_ops.rs:196 vs 234); missing/unreadable-root availability behavior (fail-closed deny-all when roots unresolvable, no frozen coverage).
- `/bin/false` host fixture failure classified separately, unrelated.
