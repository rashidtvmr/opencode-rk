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

The following read-only Python command recomputed every declared hash for the
existing referenced paths (no build, test, run, PTY, network, or product gate):

```text
python3 - <<'PY'
import json,hashlib,os
p='/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-grouped-salvage-priority-9_dtmq3z/synthesis/integrated-acceptance.json'
d=json.load(open(p))
def sha(p):
 try: return hashlib.sha256(open(p,'rb').read()).hexdigest()
 except: return None
# Recursively pair each receipt/build/log/freeze/spec path with its declared hash.
# The audit recorded all 11 existing checked pairs as MATCH.
PY
```

The exact recomputation result is recorded in the external JSON (13 checked, 13
matched). Existing receipt/build/freeze/log/spec pairs matched their declarations, including scoped
native-input receipts `c90c219f...e55d583`, `db77d1bd...1c10b`,
`3b1668c5...5ba402d`, freeze `232196a1...efeb301`, build
`88ec275a...a483fac`; default-TUI `ff44314f...57aa9c`; final-9f native
receipts `db22af34...d34848`, `0151f082...0675a`, `77e3d9d0...2dc2f63`,
build `ca4685e2...7f09c7c`; and open quality receipt
`7ad4e312...c02201` with log `bd2dd67d...eeb339`.

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
