# V2 integration evidence — 2026-09-30

## Integrated revision

`0759e01464a7a0dbed614163a14b6f60673e2412`, pushed to `origin/main-v2`.
Integration writer: the main session in `/Users/mymac/Projects/opencode-rk-main-v2`.

## V2-SALVAGE-PRESERVATION

- Candidate: `3fd4b81`; branch base `fc2d201`, inventory context `0088fbf`.
- Independent verifier/test owner: Xkiro DeepSeek V4.1 Flash Free.
- Changed paths: `tools/convergence_v2_salvage.py`,
  `tests/bootstrap/test_convergence_v2_preservation.py`,
  `docs/convergence-v2-salvage.json`, `docs/CONVERGENCE_V2_SALVAGE.md`,
  `worklog/V2-SALVAGE-PRESERVATION.md`.
- Current-requirement authority: AGENTS.md non-destructive salvage and bounded
  resource policy; G0 in `docs/CONVERGENCE.md`. This is an rk preservation tool,
  not an upstream product-parity claim.
- Frozen test SHA-256:
  `9295af1d780b8865aca44b5b59518b32c14b07904cb0626742dd14d8a898a08b`.
- Compiling RED: all eight tests fail when loading the preserved `81dadbb` tool.
- Independent candidate GREEN: eight tests pass; source SHA-256
  `6098baac892ece3fdc5c87b692acd28193f5fe52c598d122c9f1534b6d3822d5`.
- Integrated command:

  ```text
  rtk env OC2_SALVAGE_TMP=/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode /usr/bin/arch -arm64 /usr/bin/python3 -m unittest tests.bootstrap.test_convergence_v2_preservation tests.bootstrap.test_convergence_v2_backpressure tests.bootstrap.test_convergence_v2_controller
  ```

  Result: **19 passed**, including eight preservation and eleven controller tests.
- Inventory/preservation rerun on the exact integrated SHA:

  ```text
  rtk /usr/bin/arch -arm64 /usr/bin/python3 tools/convergence_v2_salvage.py --base-sha 0759e01464a7a0dbed614163a14b6f60673e2412 --base-ref main-v2 --frozen-root /Users/mymac/Projects/opencode-rk-pre-v2-20260930-1130 --preserve-dirty --snapshot-dir /Users/mymac/Projects/opencode-rk-pre-v2-20260930-1130/v2-worktree-preservation-20260930-integrated-0759e01 --json /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-preservation-integrated-0759e01.json --markdown /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-preservation-integrated-0759e01.md
  ```

  Result: 659 refs = 331 tips + 328 aliases; 45/45 detached anchors valid;
  112/112 worktrees present; 18 dirty including two staged-only; zero
  analysis/status failures. Fresh snapshot created without overwriting earlier
  snapshots. Full G0 remains pending: sensitive live `opencode.json` is left in
  place, and historical product/test intake still requires content disposition.
- State: **ACCEPTED for the preservation correctness package on `0759e01`**.

## V2-WEB-MECHANICAL-BUILD

- Candidate: `78f585c`, base `fc2d201`; cherry-picked as `0759e01`.
- Independent source review: Muse Spark; runtime preverification/integration:
  main session.
- Changed paths: `web/src/lib/canvas-model.test.ts`,
  `worklog/V2-WEB-MECHANICAL-BUILD.md`.
- Authority: independent mechanical unused-import maintenance in AGENTS.md.
  Removes five unused imports; test bodies independently compared byte-for-byte.
- Candidate and exact integrated revision both passed:

  ```text
  rtk /usr/bin/arch -arm64 pnpm --dir web run typecheck
  rtk /usr/bin/arch -arm64 pnpm --dir web exec vitest run src/lib/canvas-model.test.ts --maxWorkers=1
  rtk /usr/bin/arch -arm64 node --experimental-strip-types --test web/src/lib/api.auth.test.mjs web/src/lib/api.direct-auth.test.mjs web/src/lib/api.refresh-auth.test.mjs
  ```

  Results: typecheck GREEN, **16 canvas tests passed**, **six auth tests passed**.
- `git diff --check`: passed on the integrated tree.
- State: **ACCEPTED for the mechanical build package on `0759e01`**. The real
  browser tool/second-turn/reload journey G6 has not been run.

## Remaining observed product failures

- G2 persisted/in-app credentials are not connected to the outbound provider
  request; current turn execution reads environment credentials.
- G3/G4 typed tool persistence is not called by the live server, and second-turn
  history rejects Tool messages. The independent durability fixture is being
  frozen before the isolated source candidate is validated.
- Native renderer raw input belongs in the Rust host wrapper. A real compiled
  native PTY target reached runtime and failed four raw-mode cases at candidate
  `3692743`. Independent test-owner review also repaired a false-ready empty-byte
  marker condition without changing raw/restoration assertions. The native
  implementation and corrected fixture remain unintegrated.
- Docker server is ready (`29.8.0`). Ubuntu packaging and its golden journey
  remain unrun; usable historical native installer/artifact commits have been
  identified for selective salvage.

These component acceptances are not G0–G8 release acceptance.
