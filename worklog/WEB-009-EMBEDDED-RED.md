# WEB-009 embedded production bundle RED

## Claim and ownership

- Task: `WEB-009`
- Session: `ses_f31607881ffeA4gofXMXmKgxx0`
- Branch: `lane/WEB-009-embedded-red-20260923`
- Base: `182cb53472c01e7b8b908b3672abd0e8eda77524`
- Owned test: `crates/server/tests/web009_embedded_activity_red.rs`

## Source evidence

- `crates/server/src/lib.rs:267-274` exposes `router` and authenticated
  `router_with_auth`; `crates/server/src/lib.rs:324` installs
  `web_assets::serve` as the fallback, so the router is the embedded web-asset
  serving seam.
- `crates/server/src/daemon_auth.rs:150-157` leaves non-`/api/` paths,
  including assets, available through authenticated router middleware. The test
  therefore uses `router_with_auth` with a deterministic published token and
  disposable in-memory storage.
- `crates/server/src/web_assets.rs:38-71` serves `/`, rejects API paths,
  resolves `/assets/...` from embedded `WEB_DIST`, and applies SPA fallback only
  for extensionless paths.
- `crates/server/src/web_assets.rs:74-101` returns JavaScript content type,
  bounded body handling is enforced by the test, and `nosniff` is set by the
  serving seam.
- `crates/server/web_dist/index.html:12` currently declares the production
  module `/assets/index-XBArXcct.js`; no web source or generated asset is read by
  the test.
- Current embedded JS SHA-256:
  `ce34499eea886a5289012a7772d29fe318687c672e2245e0020a915bd8bbe833`.
  It contains `Reasoning summary` and `https:`, but lacks `Tool activity`,
  `References`, and `noreferrer`.

## RED contract

The test GETs `/` through the production authenticated router, parses the bounded
same-origin `type="module"` script `src`, proves every script URL is a bounded
`/assets/` path with no external/protocol-relative URL, then GETs that module
through the same router. It requires a JavaScript content type, non-empty body
within a 2 MiB cap, exact user-visible `Tool activity`, `Reasoning summary`, and
`References` labels, plus `https:` and `noreferrer` security semantics.

No network, browser, npm, filesystem source scan, or generated-artifact direct
read is used. Current stale `web_dist` compiles and fails on the missing exact
`Tool activity` label.

## Verification

Exact command:

```text
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 python3 - <<'PY'
import subprocess, os, sys
cmd = ['cargo','test','-p','opencode-rk-server','--test','web009_embedded_activity_red','--','--test-threads=1']
r=subprocess.run(cmd,timeout=180,text=True,env=os.environ.copy())
raise SystemExit(r.returncode)
PY
```

Result: compiling RED; `0 passed, 1 failed`; failure:
`embedded production bundle is missing exact user-visible label "Tool activity"`.

Test hash (SHA-256): `dd2029891ec289cbc3b75abb07f646eb4e2024ff9b5ed48944c4291640eb0a95`

## Decision

`WEB-009` remains `blocked`, awaiting production web bundle rebuild. This lane
adds no product or generated artifact and does not edit existing tests.
