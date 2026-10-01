# V2 convergence evidence — 2026-10-01

## DOCS_RECEIPT_DURABILITY (evidence-only candidate)

- Base SHA: `9f6428c1763381d57ab83cf82a42bda284c8fd0f`.
- Candidate branch: `docs/receipt-durability-20261001`.
- Purpose: preserve an independently recomputed audit of durable receipts for
  the production/native-TUI priority. This is documentation evidence, not a
  product implementation and not acceptance.
- Exact changed paths: `worklog/V2-INTEGRATION-20260930.md` (append only),
  `worklog/V2-CONVERGENCE-20261001.md` (new).
- External audit JSON:
  `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-convergence-y52o0gi/acceptance/integrated-acceptance-audit.json`.

## Audit method and durable result

Input synthesis:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-grouped-salvage-priority-9_dtmq3z/synthesis/integrated-acceptance.json`
SHA-256 `e1d1aeead2b06d45cc225c440d4b9df2d0726d17b82e671b4972d6141ea75517`.
The file declares canonical final SHA `9f6428c1763381d57ab83cf82a42bda284c8fd0f`
and status `SCOPED_ACCEPTED_NOT_GLOBAL_RELEASE_ACCEPTED`.

The following exact read-only Python audit command was executed. It recursively
compares every synthesis-declared receipt/build/log/freeze/spec pair, expands
both installed evidence roots, hashes every nested artifact, preserves both
literal command manifests and their nested command/log metadata, and extracts
default-TUI counts plus escape cleanup semantics. It performs no build, test, run,
PTY, network, product-gate, or filesystem mutation operation.

```text
python3 /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/audit_receipts_readonly.py
```

The auditor output is the external immutable-by-hash artifact:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-control-y52o0gi3/acceptance/integrated-acceptance-audit-20261001-r2.json`
with SHA-256
`f3a04a88cade8b742ee86fee3bdf62485fb956a4dba9e5286e36a71fe872ca53`.
It reports 13/13 declared path-hash pairs matched, 32 expanded evidence
artifacts hashed, and 2 literal command manifests audited. It records the
actual installed binary/library/archive hashes for both the `fe9830b` release
and current `9f6428c` release, all nested command strings and log hashes, and
the escape result flags (`termios_restored`, `child_reaped`, `daemon_gone`,
`forced_kill`, `raw_mode_seen`, `secret_echo`, `provider_thread_joined`).
The previous external output remains preserved at:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-convergence-y52o0gi/acceptance/integrated-acceptance-audit.json`.

The default-TUI receipt's controls are format 0, default-TUI **2 passed**, and
native-daemon-flow **4 passed**, all failed/ignored zero. Native final-9f
attestation has four probe controls, behavior **314 passed / 0 failed / 0
ignored**, and escape exit 0. These are scoped controls only.

## Scope, counts, and blockers

The accepted scope is exactly two priority packages: (A) native-input framing
at `fe9830b2e61c07f2aee91c9d5a29c2111b495579`, and (B) default-TUI contract at
`9f6428c1763381d57ab83cf82a42bda284c8fd0f`, plus the final-9f native
attestation. It is not an all-G5/all-G8 release claim. No claim is made that
241 tips were reviewed, merged, or accepted; the synthesis instead records
three preservation priority tips and explicitly says no all-pending-branches
merge claim.

Remaining observed blockers/boundaries: quality probe open (not Clippy green),
workspace quality open/RED, bridge and server diagnostics, resize not
integrated, web/Ubuntu full release open, and G4 same-session durability open.
Full G5 and G8 remain unclaimed. This document does not unlock dependencies or
change gate state.

## Candidate disposition

Status: **DOCS_RECEIPT_DURABILITY / PREVERIFIED evidence candidate only**.
It must be integrated by the sole canonical writer and is not `ACCEPTED` merely
because the documentation commit exists. No Cargo/build/run/PTY/network command
was run for this candidate.
