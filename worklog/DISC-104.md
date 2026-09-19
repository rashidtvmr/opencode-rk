# DISC-104 — Shell and file tool timeout, cancel and output bounds

claim: session ses_main_gap3 owns DISC-104, owned file crates/tools/src/shell_bounds.rs, scratchpad worklog/DISC-104.md.

source evidence:
- crates/tools/src/shell_bounds.rs (682L, sha256 49f067944c066c4bb6667612be9973e5dc5580c33385e8f91fe8fd67047c370d): ShellBounds allowlist + timeout kill+reap + cancel + per-stream byte cap; gated_write_file; denial_event_json.
- crates/tools/src/lib.rs:59 `pub mod shell_bounds;` already wired (integrator commit c1866be).
- defect context per module docs: executor.rs::execute_shell awaits bash under timeout() that drops future w/o killing child, no allowlist, no byte bound; ShellTool reads pipes unbounded; file_ops::write auto-creates parents w/o root gate.
- file untouched vs HEAD (git status shows no modification on owned file); implementation pre-exists in tree.

observed scenario:
- `cargo test -p opencode-rk-tools --lib shell_bounds`: 10/10 pass (T01 deny+event, T02 timeout kill+mark, T03 cancel+reclaim, T04 trunc cap, T05 denied-write byte-identical+UI, T06 reclaim-after-timeout, 4 edges: empty-deny, nonzero-exit, stderr-trunc, both-pipes-flood). Log /tmp/opencode/disc104-green.log.
- `cargo test -p opencode-rk-tools --lib`: 92/92 pass incl shell_bounds 10. Log /tmp/opencode/disc104-fulllib2.log. (One earlier run showed 91+1 lsp flake; rerun + lsp-only run 6/6 green; final full-lib run 92 green.)
- RED: not re-runnable — tests+impl landed together before this lane session (git log: no commits on owned file in range; HEAD c1866be wiring only). Frozen test behavior verified GREEN in place, zero test edits by this lane.

target boundary: one file only (crates/tools/src/shell_bounds.rs) + scratchpad + ledger. No edits made to owned file (already complete); no test edits; no controller/state edits.

tests: frozen in-file `mod tests` (10 tests), GREEN as above, zero modifications.

decisions: no code change needed — implementation satisfies timeout fires (T02), cancel reclaims (T03/T06), output byte-capped (T04/edges), no unbounded retain (reserve cap.min(64k), capped drain threads, debug_assert len<=cap). Verify-by-run instead of rewrite per no-stub/real-code gate.

remaining unknowns: none for lane. Wiring into executor/turn path is integrator authority (outside file per module docs).
