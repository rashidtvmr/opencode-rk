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
but is excluded from both retention queries. The added retention tests exercise
the real pending → `expire_sweep` transition, verify its current null
`resolved_at_us` lifecycle gap, then stamp the existing resolved-age field only
to test the retention API. No new cutoff/default is introduced.

## Frozen tests

Owned paths are the two new test files and this worklog. Assertions preserve the
historical expiry semantics and bounded 500-row retention behavior, adding only
the necessary real transition control. This is source preparation only; parent
must run the current-canonical RED before assigning product implementation.
