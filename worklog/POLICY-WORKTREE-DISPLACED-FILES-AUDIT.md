# POLICY-WORKTREE-DISPLACED-FILES-AUDIT

## Claim

- Task: `POLICY-WORKTREE-DISPLACED-FILES-AUDIT`
- Session: `ses_f2dd38ef5ffenjC41vDYzM1VgD`
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/structured-subagent-prompts`
- Branch: `docs/structured-subagent-prompts`
- Owned file: this audit worklog; ledger row is the only other edited path.
- Claim acquired through `tools/completion_claims.py` before this file was created.

## Authority and boundary

Git objects, current Git index/tree, exact SHA-256 values, filesystem inventory,
and cited task worklogs are evidence. Prior displacement prose is untrusted until
reproduced. No product, test, policy, Git history, config, stash, or unrelated
worklog bytes were edited. No Cargo, network, credentials, database, reset,
checkout, clean, delete, or overwrite operation was run.

## Inventory

The reported source paths are present in the current worktree and tracked by the
current tree. The displacement directory contains exactly four regular files,
with flattened basenames rather than the reported relative directories:

| Reported path | Stash artifact | Size | Stash SHA-256 | Current path | Current SHA-256 |
|---|---|---:|---|---|---|
| `crates/cli/build.rs` | `build.rs` | 4950 | `12f7dcb2f414edb596e1b2a7443cb64481266c3b23929685f74404122ce3da19` | present | `14191733ddbc40cea72b3d020190d6d914df0698cc3a50526c06a79263343d59` |
| `crates/server/tests/app012_tool_journey_red.rs` | `app012_tool_journey_red.rs` | 14206 | `53d8086ec2de0ebacc12f65338f27bec771c68db4755b58aace3e8b8ab6acdbd` | present | `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50` |
| `worklog/APP-010-REVISION-RECEIPT.md` | `APP-010-REVISION-RECEIPT.md` | 3118 | `948fc1be73229b5059c08b3bae587cd6a4eb6562512fe443ffdf81112e5b5d54` | present | `5f70b9d275442da51155f1c20fa12f8ab275c238541062d74330632d157cdba5` |
| `worklog/APP-012-TOOL-RED.md` | `APP-012-TOOL-RED.md` | 2866 | `af67886deee1f61140146be95b7ff03037fefae09cef6d311cd6a38c4fcac6d2` | present | `dbf8ef96a11bca3bd25dadb25a5e45426f8a62436d34b68912f9aec167186ef7` |

`find .../stash-untracked -maxdepth 8 -type f -print | sort` returned only
those four artifacts. No unexpected additional stash file exists. The basename
mapping is recorded; the stash copies remain intact.

## Provenance

- `build.rs` is attributable to APP-010 from its embedded task references and
  the matching APP-010 receipt. Git history has APP-010 commits `c4325e4` and
  `5547d31`; the current tracked implementation is the later 214-line version
  at `c4325e4`/`5547d31`, not the 109-line stash artifact. The current file's
  blob is `f880e89704d7a7be658d5d65e3461223a202b925`; current commit history
  proves the implementation is committed elsewhere.
- `app012_tool_journey_red.rs` is attributable to APP-012 from its header and
  matching APP-012 worklog. Git history has frozen RED commits `7358e3e` and
  `eed2bdf`; `eed2bdf:crates/server/tests/app012_tool_journey_red.rs` hashes to
  the current frozen SHA-256 `945236c4...c8c50`. The stash artifact differs by
  formatting from that committed frozen test; it is not restored or edited.
- Both worklog stash artifacts are attributable by their task headers and
  matching task names. Current tracked worklogs contain later claim, evidence,
  verification, and remaining-unknown sections. APP-010 current worklog is in
  `c4325e4`; APP-012 current worklog is in `40d56d5`.

## No-overwrite proof

Immediately before disposition, all four original paths tested present. Each
current SHA-256 differs from its mapped stash SHA-256, so none qualifies as
"already present and identical." Since every original path exists, the required
restoration precondition (original path absent) is false for every artifact.
No copy-back was performed. No source or destination was deleted or overwritten.
The current files are tracked/committed revisions, not absent untracked files.

## Disposition matrix

| Reported path | Disposition | Recovery action |
|---|---|---|
| `crates/cli/build.rs` | committed elsewhere/superseded | Do not restore. Preserve stash `build.rs`; current tracked APP-010 implementation remains untouched. |
| `crates/server/tests/app012_tool_journey_red.rs` | committed elsewhere/superseded | Do not restore. Preserve stash test; current frozen RED test remains untouched. |
| `worklog/APP-010-REVISION-RECEIPT.md` | committed elsewhere/superseded | Do not restore. Preserve stash receipt; current committed receipt remains untouched. |
| `worklog/APP-012-TOOL-RED.md` | committed elsewhere/superseded | Do not restore. Preserve stash RED evidence; current committed worklog remains untouched. |

No artifact is safely restored. Stash copies are the recovery source if a
rightful lane later proves a byte-preserving recovery need.

## Validation evidence

- `git status --short --branch`: only this audit worklog and this task's ledger
  row became edits; no product/test path was changed.
- `git log --all -- [path]`: found APP-010 commits `c4325e4`, `5547d31`; APP-012
  commits `7358e3e`, `eed2bdf`, and later integration `40d56d5` for the worklog.
- `git diff --check`: clean before this worklog was added.
- Frozen APP-012 test SHA-256 on current path and `eed2bdf` object:
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- No product test applies to this preservation-only verification task. No heavy
  process ran; elapsed time was bounded shell/read/hash/git inspection only.

## Remaining gaps

The four stash files use flattened names, so their original directory metadata
is unavailable. Byte content and task attribution are sufficient to map each
one uniquely to the four reported paths, but the stash directory itself must
remain available for any later forensic recovery.
