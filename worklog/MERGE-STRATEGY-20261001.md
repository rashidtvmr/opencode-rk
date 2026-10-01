# Parallel merge-sort strategy run — 2026-10-01

## Snapshot and preservation

- Frozen base: `8d5866e328306dbf8041d91bb41fed9a38c0971c`.
- Source landing: `82308b9010caf459392ff65f8ee0dc135531e111`.
- Canonical publication snapshot: `4d1c42f74cb56953954a0567b57558ec13799575`.
- RAW final retry receipt: `state/engine-retry-merge-run-20261001-20261001T213133-25441-final-pass-v1/final-result.json`, SHA-256 `4e48d3a9d86cd98bed08aa407c14d1125962c1a5d64e3d197fcb313da30e9fcb`.
- The retry partition contains **221 frozen eligible tips**: **14 integrated-candidate tips** (10 initial plus 4 retry additions) and **207 remaining quarantined**. The prior six unmerged tips were subsequently classified by retry; they are not an additional final partition bucket.
- Historical L4 stage accounting is retained distinctly: **10 accepted + 6 unmerged + 205 quarantined**. It must not be presented as the final retry partition.
- Native `6e796088dc1f522e87797960cb6fab7239adafde`, interrupt `257c01e5dc2838d057fb115a2d7e25ae9ff73c81`, and permission `5f35e070c85c36445e4532fc71d7c0372f33606f` are ancestors of the final source tree; runtime acceptance remains pending.

## Complete ledger publication

`worklog/V2-WORKTREE-MERGE-STATUS.md` was regenerated from actual `main-v2` and all registered worktrees/refs using:

```sh
python3 /Users/mymac/Projects/opencode-rk-v2-integration-y52o0gi3/tools/update_worktree_merge_status.py \
  --repo /Users/mymac/Projects/opencode-rk-main-v2 \
  --output /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/merge-run-20261001-20261001T213133-25441/worktrees/merge-ready-20261001/worklog/V2-WORKTREE-MERGE-STATUS.md \
  --receipts-root /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-control-y52o0gi3
```

Result: snapshot `4d1c42f74cb56953954a0567b57558ec13799575` at `2026-10-01T19:54:40Z`; 195 worktrees, 420 local branches, 321 remote-tracking refs, 792 ledger rows, and 5 verified scoped acceptance receipts. Counts are separate; unique tips are reported independently in the ledger. Ancestry is checked with `git rev-list`/Git ancestry facts, not merge events or claims.

## Frozen-head cohort

Comparing `state/branch-tips.tsv` against the original base and source landing identifies **12 named original frozen heads newly integrated** (aliases/names retained in the generated ledger): lane/APP-010-LINUX-NATIVE-ARCHIVE-CONTRACT, lane/APP-017, lane/SHIP-001-phase1, red/APP-012-installed-restart-v1, v2/live-interrupt-contract-y52o0gi3, v2/live-permission-contract-y52o0gi3, v2/native-paste-repair-y52o0gi3, v2/native-portable-pty, v2/native-reset-diag-rootcause-y52o0gi3, v2/packaged-identity, v2/web-mechanical-build, and v2/web-tool-stream.

## Scope gate

This is metadata/strategy publication, not product acceptance or release completion. No Cargo/build/test/push was run. Remaining gate: independent runtime acceptance of the native, interruption, and permission contracts (and the broader release gates) on the exact integrated SHA.
