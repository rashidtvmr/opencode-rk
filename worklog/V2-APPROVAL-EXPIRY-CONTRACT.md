# APP012 approval expiry and terminal retention contract

## Disposition

Historical `dc3331ee257dbb21197f5310b8455d32a38b34ed` is classified
`SALVAGE_PRODUCT + SALVAGE_TEST` for these two coherent repairs. Its refs and
the claims-authority proposal `00e2c26d2c4e1a95480afbc051ba08e7b2a231e6` remain
historical evidence; no blind tip merge or claims promotion is performed.

## Current authority

`Grant::covers` currently rejects only `expected.now > expires_at`. The approved
storage approval domain already uses inclusive expiry: `ApprovalsV2::expire_sweep`
transitions pending rows when `expires_at_us <= now_us` (`approvals_v2.rs:137-143`).
The APP012 grant contract therefore freezes equality as expired too, with no
new lifetime or timing choice. `GrantLedger` consumes only after `covers` succeeds
(`app_policy.rs:303-319`), so an expired grant must leave the ledger unchanged.

`RetentionV2::sweep_resolved_approvals` documents deletion of all terminal
approvals resolved at or before the caller's `older_than_us` (`retention_v2.rs:84-86`),
and `docs/STORAGE.md:263` identifies terminal approval retention as an existing
sweep. State 3 is explicitly the expired terminal state (`approvals_v2.rs:18-22`)
but is excluded from both retention queries. The corrected retention tests require
the real pending → `expire_sweep` transition to produce both state `3` and
`resolved_at_us == now_us`; no manual SQL repair is used. They then test
the existing resolved-age cutoff, bounded limit of one, resource cascade,
pending protection, and newer-expired protection. This authorizes the coherent
product repair to timestamp the state-0 → state-3 transition alongside the
state-3 retention and grant-boundary repairs. No new cutoff/default is introduced.

## Frozen tests

Owned paths are the two new test files and this worklog. Assertions preserve the
historical expiry semantics and bounded retention behavior, adding only the
necessary real transition control. This supersedes the prior `5e9d1f1` contract's
synthetic null-timestamp control without deleting or amending that history. This
is source preparation only; parent
must run the current-canonical RED before assigning product implementation.

## Frozen compiling RED

Controller formatting was completed before freezing at
`c84b4f741b28b57308ef75a248fb292a4f8e3ffa`. Exact hashes:

- Security contract: `834176d8e4aa26ce10bb6b161515c936dfd2537dfe038074da3dc5d33e923233`.
- Storage contract: `cba8bcef14de64b5278ba1b9761842cfa6abce9666b83ee6d41fc317bc07979d`.

The controller ran both real targets: the expiry contract fails because equality
returns `Ok(())`; all three retention controls fail because real expiry produces
`(3, None)` instead of a terminal resolution timestamp. Both compile and fail
at runtime, exit 101. Commands/source/test/log hashes are retained at
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-approval-lifetime-red-3tv4ajy2`.
