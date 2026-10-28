# Credential candidate review receipt — 2026-10-27

This preserves the returned independent verdict after temporary artifact loss.
It is not a fresh source review or product acceptance.

- Evaluator session: `ses_e86fd5902ffe10QmJH1vdW0bRa`.
- Verdict: **REJECT**, applicable only to reviewed candidate
  `bb576551c89fcb5f236787c6fe6900c88e1e258339579cc91a2c9de1aae532e1`.
- Candidate path: `crates/providers/src/persisted_auth.rs` (private candidate).
- Base: `6f6872bf87e02f05afe9c1a5f63e55fe4c040499`.
- Reference: `/Users/mymac/Projects/tmp/opencode` at
  `450e7b92412c780a7b5a75226bb8d49aaca24e22`.
- Candidate and diff are reported lost; their bytes were not reverified.
- Re-persisted verdict SHA256:
  `41bd014fc19f3df00de472a29dfb3e4fb5f5d5591af5c5ffa48b7660b05cae3c`.
- Re-persisted scratchpad SHA256:
  `fc84c2aceaeb54b7c1c805202a07c51e0dd5c361b3366c91ba924576b45f51f4`.
- Main independently read and hashed both re-persisted files in
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/concurrency20/credential-review/`;
  both matched the returned full hashes.

## Returned blocking findings

1. Candidate generates and requires `stablecred_` IDs; reviewer cites upstream
   `packages/schema/src/credential.ts:10-14` creating `cred_` IDs via
   `identifier.ts` ascending identifiers.
2. Candidate adds and requires `credentialId` in legacy auth.json entries;
   reviewer cites `20260805200742_import_legacy_credentials.ts` generating SQL
   credential IDs during import instead. Exact compatibility consequences need
   actual reader inspection; an extra JSON field alone does not prove every
   upstream reader rejects a document.
3. `.ok().flatten()` discards auth read/parse/bounds errors and returns an empty
   inventory, making failure indistinguishable from absence.
4. Entries without the candidate-specific ID field are silently omitted,
   including legacy entries; a correct migration and truthful inventory are
   required.

Previously recorded installed auth-control 8/8 PASS belongs to unchanged base,
not this rejected candidate. Concurrent historical ENOSPC and later recovery do
not alone prove the precise cause of every historical SQLite startup failure.

No compilation, installed test, new delegation, or integration was performed
while receiving this handoff. Negative verdict stays blocking until a corrected
exact candidate is independently reviewed.
