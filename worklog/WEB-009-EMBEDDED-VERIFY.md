# WEB-009 embedded artifact verification

## Claim and ownership

- Task: `WEB-009`
- Session: `ses_f314b32b3ffeY0eNBUcAxhOcc2`
- Branch: `lane/WEB-009-embedded-verify-20260923`
- Base HEAD: `d7d9ab823fa3cca7840f6dc530b17f6b4606a8c9`
- Owned file: this worklog plus this task's ledger row only.
- Verification-only. No product, generated asset, or test edits.

## Scope

Verify the frozen embedded-router target and the existing `web_assets` target,
then inspect the generated Vite artifact for bounded same-origin module loading,
required activity/security labels, stale-hash absence, index/artifact agreement,
size, source-map absence, and secret-looking material. Run full web Vitest when
dependencies are available. Rebuild only if byte identity can be proven.

## Frozen evidence

- Frozen test: `crates/server/tests/web009_embedded_activity_red.rs`.
- Expected frozen SHA-256: `dd2029891ec289cbc3b75abb07f646eb4e2024ff9b5ed48944c4291640eb0a95`.
- Prior embedded implementation evidence: `worklog/WEB-009-EMBEDDED-IMPL.md`.
- Browser verification evidence: `worklog/WEB-009-BROWSER-VERIFY.md`.

## Results

- Frozen SHA confirmed: `dd2029891ec289cbc3b75abb07f646eb4e2024ff9b5ed48944c4291640eb0a95`.
- Index: exactly one `type="module"` script, `/assets/index-ranRBVll.js`; no
  external, protocol-relative, whitespace, traversal, or non-asset script URL.
- Embedded JS: `509255` bytes, SHA-256
  `20260b4692e59a440ddfa2834dcee71c0f68608869f875e0f58a90b41a9fc316`; under
  the 2 MiB cap. Exact `Tool activity`, `Reasoning summary`, `References`,
  `https:`, and `noreferrer` strings present.
- Generated references: every index/CSS `/assets/` reference resolves to one of
  the seven committed assets. No stale `index-XBArXcct.js` or
  `index-Cf7wuPpG.css` file/content remains. No `*.map` file or
  `sourceMappingURL` marker. Conservative private-key/provider-token/JWT scans
  found no secret-looking material.
- Frozen target: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1` plus 180-second Python
  timeout, `cargo test -p opencode-rk-server --test web009_embedded_activity_red
  -- --test-threads=1`: `1 passed, 0 failed`.
- Existing asset target: same serial bounds,
  `cargo test -p opencode-rk-server --test web_assets -- --test-threads=1`:
  `1 passed, 0 failed`.
- Full web suite: serial `pnpm exec vitest run --reporter=basic --maxWorkers=1
  --minWorkers=1`, 180-second Python timeout: `17 files, 49 passed, 0 failed`.
- No Vite rebuild run. Prior implementation receipt records two byte-identical
  builds; this verification observed the committed artifact without changing it.
- No defect or drift found. Cargo emitted existing warnings only. `pnpm` emitted
  its existing `pnpm.onlyBuiltDependencies` warning.

## Decision

GREEN verification candidate. Ledger remains `blocked` only on branch-session
activity unification and unavailable external real-provider/browser evidence.
