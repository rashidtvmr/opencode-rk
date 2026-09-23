# PROV-023 catalog integration review

## Claim

- Task: `PROV-023`
- Session: `ses_f3c4de578ffelQv59xDXmOs03B`
- Reviewed revision: `163f303`
- Scope: audit only; no product, catalog, or frozen-test edits.

## Observable contract

`tasks/PROV-023.md:3-13` marks the mandatory task `Runtime optional: False`
and promises that users and PROV-019 profiles resolve only documented catalog
entries. Lines 28-41 require consumers to reject malformed catalogs and unknown
entries rather than guessing endpoints. A valid static JSON file is therefore
necessary but not sufficient for the declared outcome.

## Evidence

- `docs/provider-compatibility.json` exists from commit `1be93d3`; SHA-256
  `9e66d038063450eff682837e7dead9abb3d8b3e73911b5badd049592a1d3b7a3`.
- Existing Rust catalog test SHA-256:
  `257ef2ead7e9a4399963489c610dbc56d54bcba31b2c9efcc081e0a7cdde538a`.
- Added Python catalog test SHA-256:
  `29d0c5d17782d39fbb346b2eff4ff4a828a652a2f50ea2a63ffe5daea8fdedd4`.
- Independent lightweight rerun:
  `python3 -m unittest -v tests.bootstrap.test_prov023_provider_catalog`
  produced 6 passed, 0 failed.
- The prior reviewer ran
  `cargo test -p opencode-rk-providers --test prov_023_provider_catalog`
  and recorded 5 passed, 0 failed.
- Repository search excluding tests, task cards, and worklogs finds no reader of
  `docs/provider-compatibility.json`.
- `crates/providers/src/request_profile.rs:302-350` duplicates OpenAI and
  Anthropic endpoints as hardcoded strings in `documented_entry`; it does not
  load or validate the catalog.
- Repository search finds no product caller of `request_profile::profile_for` or
  `documented_entry`; usages are internal to that module or tests.

## Falsification result

The JSON schema and offline lookup helpers are GREEN, but those helpers are
test-local. Changing a documented endpoint in the catalog cannot affect the
hardcoded Rust profile, and removing/corrupting the catalog cannot make a live
provider path fail closed because no live path reads it. Thus catalog/runtime
drift is possible while all current tests pass.

## Decision

`PROV-023` remains **blocked**. It cannot be accepted as docs-only while its card
is runtime-mandatory and promises consumer enforcement. The earlier ledger note
"pending catalog implementation" was inaccurate: the catalog already exists;
the missing behavior is a real consumer and caller.

## Required RED and integration sequence

1. Integration authority assigns a product owner and path; the current task
   locks only `docs/provider-compatibility.json`, so this reviewer cannot edit a
   provider source file or shared wiring.
2. Author a public-boundary RED proving the provider request path resolves an
   endpoint from the versioned catalog, rejects a missing/bad-version catalog,
   and returns `UnknownProvider`/`UndocumentedEndpoint` without synthesis.
3. Freeze that test before implementation. A test-local JSON lookup is not an
   admissible replacement.
4. Implement one bounded, immutable catalog loader (maximum 256 KiB and declared
   entry caps), then make the real provider/profile caller use it.
5. Independently verify catalog tests plus the real caller on the integrated
   revision. Preserve fail-closed behavior and never read credentials from the
   catalog.

No parent or release acceptance is claimed.
