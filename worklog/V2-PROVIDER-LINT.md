# PROVIDER-LIB-CLIPPY candidate

- **Package/gate:** PROVIDER-LIB-CLIPPY; provider-library `-D warnings` repair.
- **Base SHA:** `f6e2d3b05c21f97c7224a333f177c60dfe210c0d`
- **Candidate status:** source-only candidate; independent verifier must run the
  provider-library Clippy gate and provider all-target tests. This worklog makes
  no runtime, full-workspace, or G0-G8 claim.
- **Observed failure:** the supplied workspace Clippy run exited 101. The six
  provider diagnostics were recorded at `clippy.log` lines 94-101, 667-755:
  test-only `Duration` import, `AccountSetupStore::authorize` argument count,
  nested metadata-expiry `if`, manual `AuthMethodKind::Default`, non-canonical
  `MethodInput` clone, and explicit auto-deref in route composition.
- **Repairs:** moved `Duration` behind `#[cfg(test)]` while retaining library
  `Instant`; added the explicitly authorized method-local `#[expect]` for the
  existing audited eight-argument authorization call; collapsed only the
  expiry-gated metadata validation condition; derived `Default` with `Unknown`
  as the default variant; implemented the opaque `Copy` clone as `*self`; and
  removed the redundant route auto-deref.
- **Frozen test/source equivalence:** no test body was modified. SHA-256 hashes
  of each `#[cfg(test)]` suffix before/after are unchanged:
  `auth.rs` `fdcbb84f9e8671aa1d0e3a7fdcd0081e9170a7bce08e5b71b4e387702e59f6a3`,
  `account_setup.rs` `aa8daf9150d0420059b0c30a38cc204482559446d2343916f18ae753f0524d2c`.
  The remaining four files have no test suffix in this source snapshot.
- **Checks performed:** owned-source rustfmt (edition 2021, `skip_children=true`)
  and `git diff --check` passed. No Cargo/build/test command was run by this
  source-only worker. The independent verifier owns all runtime validation.
- **Exact changed paths:** the six provider source files named by the package,
  plus this worklog; no manifests, tests, locks, configs, policies, or other
  product paths were changed.
