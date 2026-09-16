# RED-validity PROV-017..024

Rev: 248f519d53ed29cbf7fef7ceb2b5010fb1a4224b
Scope: PROV-017/018/019/020/021/022/023/024 only. No edits to frozen tests, lib.rs, ralph.json.
Serial execution, JOBS=1, THREADS=1, `timeout 120`, `free -h` checked before runs (2.4-2.7 GiB avail).

## PROV-017 claude_oauth.rs — READ-ONLY, no stub
- :32 `LOOPBACK_REDIRECT_URI = "http://127.0.0.1:1455/oauth/callback"` present.
- :462 consent URL interpolates `{LOOPBACK_REDIRECT_URI}` raw (workdir 1-line diff vs HEAD
  encoded form `http%3A%2F%2F127.0.0.1%3A1455%2F...`; pre-existing, not mine, left untouched).
  Both forms contain literal `127.0.0.1`; T01 asserts `url.contains("127.0.0.1")` so either passes.
- fix-present: YES. GREEN 5/5: /tmp/opencode/zD-PROV-017-green.log

## Behavior-stub-restore RED (018-022), each restored identical (sha256 matches pre-stub = HEAD)
| ID | Stub | RED log | RED result | GREEN |
|----|------|---------|------------|-------|
| 018 | validate_schema early `Err(BadSchema)` | /tmp/opencode/zD-PROV-018-red.log | 2 pass 3 fail (T01,T02,T04) | 5/5 green log |
| 019 | profile_for_with_options early `Err(UnknownProvider)` | /tmp/opencode/zD-PROV-019-red.log | 0 pass 5 fail | 5/5 green log |
| 020 | dispatch shadow always `Err(EmptyProvider)` | /tmp/opencode/zD-PROV-020-red.log | 2 pass 3 fail (T01,T03,T04) | 5/5 green log |
| 021 | record early `Err(UnknownProvider)` | /tmp/opencode/zD-PROV-021-red.log | 1 pass 4 fail (T04 ok) | 5/5 green log |
| 022 | validate_dest allowlisted dests forced `PathNotAllowed` | /tmp/opencode/zD-PROV-022-red.log | 1 pass 4 fail (T02 ok) | 5/5 green log |

All RED failures are genuine assertion failures on frozen tests (compiling RED), not compile errors
(first 018/019/020 attempts that broke compilation were discarded and redone as behavior stubs).

## Fixture-rename RED (023/024), restored identical (sha256 match)
- 023: `docs/provider-compatibility.json` temp-moved away → 0/5 fail
  (`catalog file exists: NotFound`), restored sha 9e66d038. RED: /tmp/opencode/zD-PROV-023-red.log, GREEN 5/5.
- 024: `fixtures/provider_contracts/` temp-moved away → 0/5 fail (fixture panics),
  restored all 7 files sha-identical. RED: /tmp/opencode/zD-PROV-024-red.log, GREEN 5/5.

## Final state
- `git diff --stat` on owned paths: only pre-existing claude_oauth.rs 1-line encoding diff.
  local_credential_import.rs, request_profile.rs, auth_commands.rs, usage_status.rs,
  auth_store.rs, docs/provider-compatibility.json, fixtures/provider_contracts/ all clean vs HEAD.
- GREEN 5/5 all 8 targets (40/40). Green logs: /tmp/opencode/zD-PROV-{017..024}-green.log.
