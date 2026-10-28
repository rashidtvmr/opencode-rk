# Artifact readiness audit 20261026
Updated: 2026-10-26; role: artifact readiness auditor; session: ses_e86a88f0bffeQwupgLr1YLfcy2
Base/candidate SHA: 6f6872bf87e02f05afe9c1a5f63e55fe4c040499; upstream SHA: 450e7b92412c780a7b5a75226bb8d49aaca24e22

Absolute source and artifact directories:
- canonical (STRICT READ-ONLY): /Users/mymac/Projects/opencode-rk-recovery-20261005
- upstream checkout (READ-ONLY): /Users/mymac/Projects/tmp/opencode
- source/work artifact dir (WRITE-ONLY target parent): /Users/mymac/Projects/opencode-rk-recovery-20261005/concurrency-repair-wave/artifact-readiness/
- worker scratch dirs read (READ-ONLY): /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/concurrency20/

## Goal and success criteria
Reconcile the stale dispatch roster (concurrency20/roster.json, 23 assignments, observed_at 17:00:05 UTC) against actual on-disk candidate artifacts. Produce an exact list of present vs missing task dirs, exact sha256 of every deliverable file, identify the 3 strongest integrate-ready candidates, and flag actual missing-dependencies. No active-count inference from dispatch state. No Cargo/PTY/network/DB/git mutation; no child subagents; no poll sessions.

## Evidence read
- docs/AGENT_WORKER_PRACTICES.md (read first per assignment).
- concurrency20/roster.json: 23 assignments, routes vyce-deepseek-v41 (8) + 9router-vyce-deepseek-v4.1-free-combined (15).
- Full directory listing of concurrency20/ (40 dirs observed).
- Every file under present task dirs hashed (sha256 via hashlib, cross-checked spot with sha256sum).
- Canonical git ls-tree at 6f6872bf for key source paths: server_info.rs ABSENT; route_table.rs/main.rs/daemon_auth.rs/tui_entry.rs/lib.rs/catalog/lib.rs/storage/lib.rs/native_input_decoder.rs present.
- Upstream HEAD confirmed 450e7b92412c780a7b5a75226bb8d49aaca24e22.

## Findings (confirmed)
- 20 of 23 roster tasks have a matching on-disk directory under concurrency20/.
- 3 roster tasks have NO on-disk directory (true missing, no dir materialized):
  * catalog-refresh (session ses_e870ed5d3ffe..., route vyce-deepseek-v41)
  * credential-review (session ses_e86fd5902ffe..., route combined)
  * skill-v5-review (session ses_e87083439ffe..., route vyce-deepseek-v41) -- note: skill-prepare-v6-integrator exists separately and is NOT the same task key.
- LIVE-WRITE DRIFT: concurrency20/* are mutable worker write dirs, not frozen snapshots. Native tabs diff hash changed between two reads within this audit (77265842...e0 -> 132a3ca1...c1) while tab-controls-v2/native_tabs.rs hash was stable (2d188a50...). Hashes reflect instant of read only. This is evidence the wave is still active; any hash is a point-in-time witness.
- No scratchpad/VERDICT/review file under any present dir declares ACCEPTED or INTEGRATION_READY; all are read-only structural preverification notes. No independent approval artifacts found on disk.
- api-info-replacement carries a hash-manifest.sha256 declaring exact file set for its candidate patch.
- skill-prepare-v6-integrator (NOT on roster by that key) has executed-verification.json + skill-tests (9router skill, acfe3ebf... v5 source, a87d035a... v6 corrected). Out of roster-scope; recorded for context only.

## Decisions and rationale
- Candidate readiness = presence of a single coherent .diff/.patch against canonical 6f6872bf that is the integrate-ready form. Source .rs files alone are NOT integration-ready (need diff). Ranked by byte size desc.
- "Missing dependencies" = files referenced by a patch that do NOT exist in canonical at 6f6872bf (would block integrator application). Verified per-patch.
- Did NOT infer 20 active workers from roster; roster states are dispatch acks / redispatched-pending, not completion.

## Alternatives rejected
- Treating skill-prepare-v6-integrator as the rostered skill-v5-review (different dir key, different session id; not a match).

## Verification actually executed
- sha256sum cross-check on tab-controls-v2/native_tabs.rs and native_tabs.diff (two reads; observed one drift).
- git cat-file -e checks on canonical paths listed above.
- Full file-tree hash of all 20 present dirs (see inventory.json).
- No build, no test run (agent bar forbids Cargo/PTY).

## Artifacts and hashes
- inventory.json sha256: 0c98077ef56ffaa93368584e9e695814a8ee41ad01932784a0e9d224a8a387c6
- Top-3 integrate-ready candidate patches (from inventory.json integrate_ready_patches):
  1. api-info materialized.diff  d06293dd6d0fb981e84e04e4e1aac5a91fee02568348be93f28549d6b0332868  23000B  -- server_info.rs NEW (canonical ABSENT; new file, no deps)
  2. tab-controls-v2 native_tabs.diff  132a3ca145e4100d853859e690a11c4d8a834cf408e56407ff8810a0636d1  18717B  -- path crates/cli/src/native_input_decoder.rs EXISTS (canonical present, may need adjustment)
  3. shell-v4 PATCH.diff  8c2b353dd2256ad3663e1440f932874fa6807a926fea6f396e0e44f97d8e0524  14373B  -- lib.rs path exists (canonical present)

## Remaining uncertainties / blockers
- HASH DRIFT BLOCKER: candidate diffs may continue mutating. Auditor cannot certify final accept-ready hash. Hand to integrator to re-hash at preverify time.
- 3 roster tasks have zero on-disk output: catalog-refresh, credential-review, skill-v5-review. Workers must materialize before evaluation.
- NO candidate carries an executed verification.json asserting compilation/test PASS on canonical base; all are source/structural notes. Top3 are integration-form patches but UNVERIFIED by execution.
- api-info patch adds a NEW server_info.rs module; canonical does not have it -- integrator must confirm module wiring in lib.rs/Rust build (diff targets lib.rs:134-318 region).
- tab-controls-v2 native_tabs.diff targets native_input_decoder.rs path but edits tab strip geometry -- need integrator to confirm target path alignment (may need native_tabs.rs or tui_entry.rs target). Auditor cannot resolve ambiguity from diff header alone without executing.

## Next steps
- Integrator re-fetch exact diffs at preverify instant; hash drift invalidates any stale point-in-time witness.
- Resolve target-path ambiguity for tab-controls-v2 patch (diff header vs canonical file layout).
- Materialize dirs for the 3 missing tasks.
- Independent preverification (compile/test) required before INTEGRATION_READY.
