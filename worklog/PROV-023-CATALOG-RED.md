# PROV-023-CATALOG-RED

## Claim

- Task: PROV-023.
- Session: `ses_f32dca9d8ffeV3z4jQQdITtaso`.
- Branch: `lane/PROV-023-catalog`.
- Owned files: `tests/bootstrap/test_prov023_provider_catalog.py`, this scratchpad,
  and the PROV-023 ledger row only. No catalog edits.
- Ledger: `in-progress`, claimed with `tools/completion_claims.py` before edits.

## Source evidence

- `tasks/PROV-023.md:26-55`: catalog shape, exact lookup, caps, secret scan,
  canonical JSON, T01-T05.
- `tasks/PROV-023.md:57-61`: RED must fail on missing catalog; negative fixtures
  cover bad-version, bad-endpoint, secret, over-cap.
- `docs/provider-compatibility.json:1-90`: current documentation
  catalog at candidate commit `dd7670d`; OpenAI, Anthropic, Google endpoint
  sources and dates are recorded there. This lane does not add endpoint claims.
- `docs/research/REQ-041-provider-compatibility-research.md:7-10,48-58`:
  official documented APIs only; no impersonation, spoof headers, undocumented
  endpoints, or secret logging.
- `crates/providers/src/config.rs:31-64`: repository provider defaults include
  OpenAI, Anthropic, Google URLs and API-key environment names. Values are not
  imported into the catalog.
- `crates/providers/src/registry.rs:59-82,108-139`: provider IDs are exact
  registry keys; default providers include OpenAI, Anthropic, Google.
- `crates/providers/src/model_route.rs:91-137`: named aliases are caller-owned,
  explicit, and unknown names return `UnknownModel`; no guessing precedent.
- `crates/providers/src/oauth_flow.rs:39-64`: HTTPS-only, bounded, no-network
  metadata planning precedent.
- `docs/SECURITY.md:24-32`: no direct secret access or unrestricted environment.

## Contract implemented by RED

`tests/bootstrap/test_prov023_provider_catalog.py` contains six executable tests:

- T01 strict top-level/provider/endpoint/refresh schema, version `"1"`, ISO
  dates, OpenAI + Anthropic + third provider.
- T02 HTTPS source/templates, RFC-style named auth headers, per-provider aliases,
  exact endpoint and alias lookup. No fallback or URL synthesis.
- T03 unknown provider/kind returns `None`; no guessed URL.
- T04 provider/endpoint/alias/file caps, no `undocumented`, spoof header names,
  or secret-like values.
- T05 sorted canonical object/array order plus pretty JSON byte identity.
- Negative disposable fixture helper: bad version, HTTP endpoint, provider
  over-cap, secret-like value. Synthetic fixture URLs are not product claims.

All validation is offline, one bounded read, no socket, clock, env, subprocess,
credential, or product-code import.

## RED evidence

Commands:

```text
rtk python3 -m unittest -v tests.bootstrap.test_prov023_provider_catalog
```

Result at candidate tree: `Ran 6 tests ... OK`. Existing catalog is present, so
the full suite is green against current documentation. The isolated missing-file
probe replaced only the module's `CATALOG_PATH` with the absent
`docs/provider-compatibility.missing.json` path:

```text
RED missing-catalog=5; negative fixtures=pass
```

This proves T01-T05 fail solely with `CatalogError.code == "missing-catalog"`
when `docs/provider-compatibility.json` is absent; the negative helper cases
pass independently. `py_compile` and `git diff --check` pass.

Frozen test hash:

```text
sha256 tests/bootstrap/test_prov023_provider_catalog.py = 29d0c5d17782d39fbb346b2eff4ff4a828a652a2f50ea2a63ffe5daea8fdedd4
```

## Status / blocker

Blocked pending the docs/provider-compatibility.json implementation/review by the
catalog lane. This lane authored RED only. No acceptance claim. The current
catalog's official-support uncertainty remains its cited research/docs surface;
tests intentionally do not infer undocumented endpoints, auth values, aliases,
or provider support.

## Remaining unknowns

- Independent verifier must freeze the final test hash and validate the catalog
  against the same revision.
- Integrator must decide whether catalog ordering/claims need a separate docs
  review; this lane does not modify that file.
