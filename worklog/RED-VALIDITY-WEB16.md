# RED-VALIDITY-WEB16 — WEB-001..006 temp-stub-restore validity

Rev `248f519`. Method: per-file temp stub (force `Err`/admit-false) in working
copy, compile via `rustc --edition 2021 --test` with prebuilt dep rlibs
(`serde_json-90b6e224d45e3544`, `serde-08143045a202d888`,
`tempfile-4fa9fdc1b05648aa`, `-L dependency=target/debug/deps`), run
`--test-threads=1`, restore from `/tmp/opencode-backup-web/*.bak`, verify
sha256 identical. `cargo test -p` unusable: unrelated `sessions/share_merge.rs`
pre-existing parse error. No repo test file edited. `ralph.json` untouched.
Voice (WEB-016) out of lease, not run.

## Rows

| id | file stubbed | stub kind | RED log | compile | fail | restored sha256 |
|----|----|----|----|----|----|----|
| WEB-001 | `crates/server/src/control_plane_inputs.rs` | `decode_move_session_input` force `Err(MissingField)` | `/tmp/opencode/zB-WEB-001-red.log` | ok | 5/5 FAILED | `64fea7f3…` identical |
| WEB-002 | `crates/server/src/control_plane_errors.rs` | `translate` force `status 0 / TEMP_STUB_RED` | `/tmp/opencode/zB-WEB-002-red.log` | ok (unreachable warn) | 4 FAILED, T04 ok (cardinality helper untouched) | `45f1e65e…` identical |
| WEB-003 | `crates/server/src/protocol_api.rs` | `check_served` force `Err(DuplicateRoute TEMP_STUB_RED)` | `/tmp/opencode/zB-WEB-003-red.log` | ok (unreachable warn) | 3 FAILED, T03+T04 ok (pure helpers untouched) | `5f9fce7a…` identical |
| WEB-004 | `crates/server/src/control_plane_exposure.rs` | `check_peer` admit-false `Ok(())` | `/tmp/opencode/zB-WEB-004-red.log` | ok (unreachable warn) | 3 FAILED, T01+T04 ok (allow-paths unaffected) | `d298627d…` identical |
| WEB-005 | `crates/server/src/event_stream.rs` | `encode_frame_with` force `Err(TooLarge)` | `/tmp/opencode/zB-WEB-005-red.log` | ok | 5/5 FAILED | `f9574703…` identical |
| WEB-006 | none (repo untouched); stub lock in `/tmp/opencode/web006_red.rs` | `acquire` always grants, `is_held` always true | `/tmp/opencode/zB-WEB-006-red.log` | ok | 2/2 FAILED (concurrent winners=2, stale unrecoverable) | n/a, repo file unmodified |

## GREEN (restored, prebuilt binaries, `--test-threads=1`)

- control_plane_inputs `5681f363a9094f85`: 5/5 ok
- control_plane_errors `291f487cbdb63708`: 5/5 ok
- protocol_api `308cf0fda15e007d`: 5/5 ok
- control_plane_exposure `556d6cd6f56dce68`: 5/5 ok
- event_stream `87a70b47209dbfbc`: 5/5 ok
- web_singleton_lock `d2aa65de5618c40f`: 2/2 ok
- Frozen `crates/cli/tests/web_singleton_runtime.rs` sha256 `5bdadc68…` untouched.

## Notes

- Partial-RED rows (WEB-002 T04, WEB-003 T03/T04, WEB-004 T01/T04) pass under
  stub because they assert helpers the stub does not break; the failing rows
  prove the load-bearing paths. Full-suite RED (0 pass) holds for WEB-001/005.
- `git status` M flags on `control_plane_exposure.rs`, `event_stream.rs`,
  `protocol_api.rs`, `daemon.rs`, `lib.rs` pre-date this lane (formatter/other
  lanes); session-start vs restored sha256 identical for all five owned files.
- Voice `crates/server/src/voice_capture.rs` never opened, per lease.
