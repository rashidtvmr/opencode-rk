# PHASE1-SIGNING-READINESS

## Claim and boundary

- Task: `PHASE1-SIGNING-READINESS`
- Session: `ses_f28c48529ffe43Q5RX1B5uKR8M`
- Branch: `plan/PHASE1-SIGNING-READINESS`
- Base: `7262682e31c1f473912c9483804c9430eb4ee4c5` (`origin/main` at lane start)
- Owned paths: this worklog and the task's claim row only.
- Lane type: text/static proposal. No Cargo, workflow, product, test, secret, credential, signing, notarization, or network operation.
- This document defines a boundary and a future receipt contract. It does not create an artifact, release, signature, notarization ticket, certificate, key, or acceptance decision.

## Source evidence

All paths below are read at the exact base commit above.

| Evidence | Classification | Consequence |
|---|---|---|
| `PLAN.md:241-252`, release completion certificate | current contract | A release claim needs exact revision, artifact hashes, provenance, platform evidence, and unresolved blockers. A receipt is evidence, not authority. |
| `docs/SECURITY.md:62-72` | current security policy | Actual signing identities cannot be fabricated; missing authority stops the lane and is recorded as a blocker. |
| `docs/AUTONOMOUS_EXECUTION.md:106-122` | current trust boundary | Signing identities and production accounts are human-only authority. Worker output is untrusted evidence. |
| `docs/ADAPTER_PROTOCOL.md:230-239`, row B8 | current external-authority contract | Signing/consent/device authority is an external gate; absence is `blocked`, never a fabricated pass. |
| `tasks/completion/delivery.json:12`, `SHIP-001` | planned release requirement | Platform signing is required where the distribution policy requires it, or the authority remains blocked. This text lane does not implement SHIP-001. |
| `worklog/PHASE1-VERTICAL-SYNTHESIS.md:44-52,114-117,159-176,383-407` | landed planning proposal, not executable product evidence | Phase 1 may be an honestly labeled unsigned candidate. Apple signing/notarization and Windows Authenticode are external-optional and do not block that candidate; mandatory platform journeys and exact-revision receipts still do. |
| `crates/cli/src/install_commands.rs:131-177`, `InstallPlatform` and `release_artifact_ok` | current code surface | The current pure artifact gate models supported platform plus checksum only. It has no signing or notarization semantics; its presence is not release proof. |
| `scripts/install-oc2.sh:65-80` | current installer code | The POSIX installer checks SHA-256 before writing. A checksum proves integrity against the supplied digest; it does not prove a signer identity. |
| `scripts/install-oc2.ps1:36-46` | current installer code | The PowerShell installer likewise checks SHA-256 before writing. It does not verify Authenticode. |
| `docs/REPOSITORY_PROTECTION.md:150-167` | current policy boundary | Source files cannot attest an external platform's current state. The same separation applies to external signing infrastructure: local text and local checks cannot attest an identity or service result. |

There is no `release/` directory or `.github/workflows/release.yml` at this base. The receipt below is therefore a proposed interface for a later authorized release lane, not a claim that those paths exist.

## Observable contract

### 1. Unsigned Phase 1 candidate

An `unsigned-phase1-candidate` is eligible for the explicitly unsigned Phase 1 lane only when all of these are true:

1. The artifact is bound to one exact 40-character source revision.
2. The artifact's SHA-256, target platform, architecture, format, and build inputs are recorded.
3. The required installed journey passes on every mandatory target in the Phase 1 matrix, with a disposable HOME/data directory and no development runtime substituted for the installed artifact.
4. The receipt says `mode: unsigned` and contains no implied Gatekeeper, SmartScreen, Authenticode, or notarization trust claim.
5. The distribution is labeled as unsigned/dev/test where users could mistake it for a trusted production release.

This status permits an honestly labeled local/Phase 1 candidate. It does not mean “safe,” “publisher-verified,” “notarized,” or “accepted by a release authority.” OS warnings, quarantine, SmartScreen prompts, or Gatekeeper rejection are expected consequences of this status and must not be hidden.

### 2. Optional external Apple signing and notarization

`apple-developer-id` means an authorized operator used an Apple Developer ID Application identity to sign the exact artifact. That is a cryptographic signature claim, not notarization.

`apple-notarized` additionally means Apple accepted the signed artifact through the notary service, the acceptance result is bound to the exact artifact digest, and the resulting ticket is stapled or otherwise verifiable under the documented distribution procedure. A Developer ID signature alone must remain `apple-signed`, never `apple-notarized`.

Apple identity, trust, and notarization are outside the repository worker. A local `codesign` invocation cannot create Apple-issued identity evidence.

### 3. Optional Windows Authenticode

`windows-authenticode` means the exact PE executable or installer was signed with an organization-approved code-signing certificate and the verifier observed `Valid` under the target Windows trust policy. `windows-authenticode-timestamped` additionally records a valid trusted timestamp. Authenticode validity does not guarantee SmartScreen reputation or installer policy acceptance; those are separate observations.

The Phase 1 synthesis currently names the Windows target `x86_64-pc-windows-gnu`; this document does not expand support to `windows-msvc` or claim that an unsigned GNU artifact is Authenticode-signed.

### 4. Receipt and authority separation

A receipt records observations. It never grants authority, changes a task status, or substitutes for independent acceptance. The release owner/human authorizes signing; infrastructure performs the operation; a verifier checks the resulting receipt. A worker can propose the schema and explain the boundary, but cannot self-authorize the human step.

## Inputs required from humans and infrastructure

### Common release inputs

- Exact source revision, artifact set, target matrix, artifact format, and expected output names.
- Reproducible build metadata: builder identity/reference, toolchain versions, target triples, build flags, source/archive hashes, SBOM, license inventory, and provenance reference.
- Immutable artifact handoff. The signer receives the same bytes whose digest is recorded; signing must not silently rebuild or mutate the candidate.
- A release owner approval binding the artifact digest, target, signing purpose, and permitted identity reference. The approval is an authority event, not a secret.
- Isolated signing runner with a controlled clock, trusted platform roots, bounded logs, and an append-only receipt sink.
- A verifier with the platform tools and trust roots needed for the claimed check. Verification must be offline once the signed artifact and required public trust material are available.
- Retention and access policy for public certificate metadata and receipts. Private keys, passwords, API keys, notary credentials, and raw environment data never enter the repository or receipt.

### Apple-specific inputs

- Active Apple Developer Program organization, Team ID, and an authorized release operator.
- Developer ID Application certificate and its private key held in an approved keychain, HSM, or other non-exportable secret store. A subagent must not receive or create the private key.
- Exact bundle identifier, signing identity reference, hardened-runtime/entitlement decision, and artifact layout.
- Approved notarization credential/profile (for example, an App Store Connect API key or app-specific password held in the operator's secret store), plus network access to Apple's notary service from the authorized release runner.
- Apple acceptance record, artifact digest/submission reference, ticket or stapling result, and the final post-stapling artifact digest.

### Windows-specific inputs

- Organization-approved code-signing certificate with Code Signing EKU, intended subject/identity reference, private key in an HSM/Key Vault/approved secure signing service, and certificate-chain policy.
- Windows runner with the correct target architecture, `signtool`, trusted root/CRL/OCSP policy, and optional trusted timestamp service.
- Exact executable/installer digest, signer certificate thumbprint/fingerprint, chain result, timestamp result, and final artifact digest.
- A human decision if the certificate, timestamp authority, target, or distribution channel differs from the approved release policy.

## Offline verification procedure

These are commands for a later authorized operator/verifier. They are deliberately verification-only. This lane must not run signing, submission, keychain, credential, or network commands.

### Common, platform-independent checks

Run from a trusted checkout with the artifact already present. Do not fetch dependencies or contact a service.

```sh
# Bind the receipt to the exact source revision.
git rev-parse HEAD
git show -s --format='%H %cI' HEAD

# Hash one artifact without changing it.
shasum -a 256 "$ARTIFACT"                 # macOS/BSD
# or: sha256sum "$ARTIFACT"              # Linux
```

PowerShell uses:

```powershell
(Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant()
```

Record command ID, exact argument shape, exit code, tool version where available, selected normalized output, and output hash. Do not store raw unbounded output.

### macOS: signature, Gatekeeper assessment, and stapled ticket

For an `.app` bundle:

```sh
codesign --verify --deep --strict --verbose=2 "$APP"
codesign -dv --verbose=4 "$APP" 2> "$SIGNED_METADATA"
spctl --assess --type execute --verbose=4 "$APP"
xcrun stapler validate "$APP"
```

For a signed/notarized `.pkg`, use the package assessment form:

```sh
codesign --verify --strict --verbose=2 "$PKG"
spctl --assess --type install --verbose=4 "$PKG"
xcrun stapler validate "$PKG"
```

Interpretation:

- `codesign --verify` proves the signature is internally valid for the inspected artifact; it is not proof of Apple identity or notarization.
- `codesign -dv` exposes certificate/authority metadata, not a private key. Normalize it; do not copy arbitrary stderr into a receipt.
- `spctl` is the policy/Gatekeeper observation. A rejection is a failed external claim, not something to bypass.
- `stapler validate` confirms a stapled ticket. Successful stapling without an Apple acceptance record is insufficient for `apple-notarized`.
- Any nonzero result is recorded with the artifact digest. Never “repair” a failed result by disabling Gatekeeper, removing quarantine metadata, or changing the artifact.

### Windows: Authenticode and timestamp verification

On a Windows host with the artifact and public trust roots available:

```powershell
$Artifact = (Resolve-Path -LiteralPath $ArtifactPath).Path
$Hash = (Get-FileHash -LiteralPath $Artifact -Algorithm SHA256).Hash.ToLowerInvariant()
$Signature = Get-AuthenticodeSignature -FilePath $Artifact
$Signature | Select-Object Status, StatusMessage, SignerCertificate, TimeStamperCertificate
& signtool verify /pa /all /v $Artifact
$LASTEXITCODE
```

Interpretation:

- `Status=Valid` plus exit code 0 is the minimum `windows-authenticode` observation under that host's trust policy.
- `TimeStamperCertificate` and the `signtool` result are recorded separately; a valid signature without a required timestamp must not be upgraded to `windows-authenticode-timestamped`.
- `NotSigned` is an explicit unsigned classification. `UnknownError`, `NotTrusted`, `HashMismatch`, and tool failure are failures/blockers, never aliases for `NotSigned`.
- `certutil -dump` or certificate-chain output may be collected by release infrastructure, but only normalized public fields belong in the receipt.
- Authenticode does not establish SmartScreen reputation. Record that limitation rather than inferring user-facing trust.

## Receipt schema proposal

Use a versioned JSON object. The following is a schema description, not a checked-in release receipt.

```json
{
  "schema": "phase1.signing-receipt.v1",
  "receipt_id": "opaque-release-reference",
  "status": "unsigned-verified | signed | notarized | valid | blocked | rejected",
  "source": {
    "revision": "40-lowercase-hex",
    "revision_verified": true,
    "dirty_tree": false
  },
  "artifact": {
    "name": "relative-artifact-name",
    "platform": "linux-x64 | linux-arm64 | macos-x64 | macos-arm64 | windows-x86_64-pc-windows-gnu",
    "architecture": "target-architecture",
    "format": "tar.gz | zip | app | pkg | exe | other",
    "sha256": "64-lowercase-hex",
    "size_bytes": 0
  },
  "build": {
    "source_archive_sha256": "64-lowercase-hex-or-null",
    "toolchain": ["redacted-tool-name/version"],
    "builder_reference": "opaque-reference",
    "sbom_reference": "opaque-reference",
    "provenance_reference": "opaque-reference"
  },
  "verification": {
    "overall": "pass | fail | blocked | not-applicable",
    "offline": true,
    "commands": [
      {
        "id": "stable-command-id",
        "exit_code": 0,
        "output_sha256": "64-lowercase-hex",
        "selected_fields": {}
      }
    ]
  },
  "signing": {
    "mode": "unsigned | apple-developer-id | apple-notarized | windows-authenticode | windows-authenticode-timestamped",
    "apple": null,
    "windows": null
  },
  "authority": {
    "state": "not-required | human-approved | blocked",
    "operator_reference": "opaque-reference-or-null",
    "approval_reference": "opaque-reference-or-null"
  },
  "redaction": {
    "version": 1,
    "secret_scan": "pass",
    "raw_tool_output_retained": false
  },
  "failure": null
}
```

For an Apple receipt, `signing.apple` should contain only normalized public/operational fields such as `team_id`, `certificate_sha256`, `authority_label`, `timestamp_observed`, `notarization_submission_reference`, `notarization_status`, `ticket_sha256`, and `stapled`. For a Windows receipt, `signing.windows` should contain `certificate_thumbprint`, `chain_status`, `signature_status`, `timestamp_status`, `timestamp_reference`, and `signtool_version`.

The `failure` object, when present, contains a bounded code, a short redacted message, the failing command IDs, the artifact digest, and a remediation owner. It never contains a private key, credential, raw environment, or unbounded log.

### Schema invariants

- `source.revision` and every artifact digest are lowercase hexadecimal strings of the exact required length.
- A receipt is immutable after publication. A changed source revision, artifact byte, target, signer, or verification result requires a new receipt; never edit the old one.
- One receipt binds one `(revision, artifact_sha256, platform, format, signing mode)` tuple. A signed retry cannot reuse an unsigned receipt's final digest without a new record.
- `mode=unsigned` permits `authority.state=not-required`; it must not contain a claim of Apple or Windows trust.
- `apple-notarized` requires a successful signature check, Apple acceptance evidence, and ticket/staple evidence. `windows-authenticode-timestamped` requires a valid signature and trusted timestamp evidence.
- `overall=fail` or `blocked` cannot be presented as `pass`. An unsigned candidate remains valid only when its own unsigned gates pass; it cannot inherit a failed signed attempt's status.
- Missing fields are `null` with a reason in `failure` or `limitations`, never guessed values.
- `dirty_tree=true`, a digest mismatch, an unverified tool exit, or a receipt from another revision is not release evidence.

## Redaction and secret handling

Allowlist, do not denylist:

- Source revision, artifact relative name, platform/architecture, artifact digest/size, public certificate fingerprint/thumbprint, public authority label, Apple Team ID, tool versions, exit codes, stable command IDs, bounded timestamps, notarization/timestamp references, and opaque release/approval IDs.
- Normalize absolute paths to artifact-relative names. Do not record `$HOME`, usernames, machine names, CI account names, or temporary directory paths.
- Never record private keys, keychain exports, certificate private-key URLs, passwords, API keys, notary credentials, access tokens, environment dumps, command lines containing secrets, provider credentials, or raw process environments.
- Do not use shell tracing (`set -x`) around signing or credential setup. Capture only selected normalized fields from tool output.
- Store raw tool output only in an access-controlled, time-bounded external evidence store when policy requires it. The repository receipt stores a hash and allowlisted fields, not the raw stream.
- Run a secret-pattern scan over the receipt before publication. A failed scan is `blocked`; do not publish a partially redacted receipt.
- Public certificate metadata is not a secret, but personal names, email addresses, and account identifiers should be minimized unless the release policy explicitly requires them.

## Ownership, lifetime, persistence, and bounds

- The source revision and artifact bytes are immutable inputs. The release owner owns the decision to submit them to an external identity operation.
- The signing service/HSM owns private identity material. This repository never owns, mirrors, or requests it.
- A receipt is append-only evidence owned by release infrastructure. It is retained according to release policy; it is not a controller acceptance flag.
- State transitions are one-way for a given artifact digest: `draft -> unsigned-verified` for an unsigned candidate, or `draft -> signed -> notarized/valid` for an authorized external path. A failure transitions to `blocked` or `rejected`; it never silently falls back to `unsigned` after signing has begun.
- Any source, artifact, target, certificate, timestamp, or trust-policy change invalidates the prior receipt for release use and requires a fresh one.
- Receipt JSON is bounded (recommended maximum 64 KiB), with at most 32 verification records and at most 8 KiB retained per command result. Oversized output is summarized by digest or rejected; no unbounded log is retained.
- At most one signing operation may be active for a given artifact digest. No automatic replay of an ambiguous external side effect. Auth/signing failures stop immediately; retries occur only after human remediation and a new approval/reference.
- Receipt generation is pure text/JSON assembly with no product runtime effect. It cannot start a daemon, install an artifact, access a keychain, or mutate user data.

## Failure handling

| Failure | Receipt result | Unsigned Phase 1 effect | Signed/notarized claim |
|---|---|---|---|
| Identity, certificate, key, credential, or human approval absent | `blocked` with `missing_authority` | No effect if unsigned gates pass | No claim; do not fabricate or substitute a test/self-signed identity. |
| Signer/network/notary service unavailable | `blocked` with `external_service_unavailable` | No effect if unsigned gates pass | Preserve unsigned artifact separately; do not call a partially processed artifact signed. |
| Digest changes between build, sign, and verify | `rejected` with `artifact_digest_mismatch` | Candidate remains only if rebuilt and reverified | Quarantine the artifact; never relabel it or publish it. |
| `codesign`, `spctl`, or `stapler` fails | `rejected` or `blocked` per cause | No effect on a separate, honestly unsigned artifact | Do not bypass Gatekeeper, remove quarantine, or suppress the result. |
| Apple notary rejects the upload | `rejected` with `notarization_rejected` | No effect | Signed-but-not-notarized is a distinct state, not a pass. |
| Windows status is `UnknownError`, `NotTrusted`, or `HashMismatch` | `rejected`/`blocked` | No effect | Never map these to `NotSigned`; obtain a new authorized operation or fix the artifact. |
| Certificate expired/revoked, timestamp absent, or trust roots unavailable | `blocked` or `rejected` | No effect | Do not infer current trust from an old receipt. |
| Wrong architecture or unsupported package type | `rejected` with `unsupported_target` | Record the target as unsupported; do not broaden support | No cross-platform claim. |
| Receipt absent, partial, stale, or secret scan failed | `unverified`/`blocked` | Candidate is not release evidence until regenerated | No claim. |
| Human declines optional signing | `skipped` only in the external-gate record | Explicitly unsigned candidate remains eligible if all other gates pass | No signed distribution claim. |

A failed signed attempt must not delete, overwrite, or downgrade the separately hashed unsigned artifact. If signing may have modified bytes, quarantine that output and rebuild from the source revision. Never use a failed result as evidence that an unsigned artifact was “mostly signed.”

## Why a subagent cannot manufacture identity evidence

Identity evidence is a real external authority event, not a JSON field:

- An Apple Developer ID certificate/private key is issued and controlled by Apple and an authorized organization. Notarization additionally requires an Apple account, accepted service submission, and a service-generated ticket.
- An Authenticode certificate/private key is issued by a trusted certificate authority or organization policy and must be held by an approved signing service/HSM. `signtool` can consume an authorized key; it cannot create trust for a made-up certificate.
- A model can write a plausible certificate subject, JSON receipt, command transcript, or self-signed test certificate. None is evidence of Apple/CA issuance, key custody, human consent, timestamp authority, or notarization acceptance.
- Repository policy explicitly classifies signing identities as human-only authority and requires a blocker when unavailable. The correct subagent output is an exact boundary, schema, redaction rule, and reproducible verification procedure—not invented identity evidence.

## Validation and handoff limits

Expected local, read-only checks for this text lane:

```sh
python3 -m json.tool tasks/completion/claims.json >/dev/null
python3 tools/completion_claims.py .
git diff --check
python3 tools/validate_repository.py
python3 tools/convergence_gate.py
```

`tools/lane_gate.py` and all Cargo/test targets are intentionally not run: the lane contract forbids Cargo, workflow, product, and test operations. A repository-validator or convergence failure must be reported as inherited evidence, not repaired by editing protected backlog, controller, verifier, or test files.

This document does not prove that a Phase 1 artifact exists, that any platform journey passes, that Apple/Windows trust is valid, or that a parent release task is complete. `SHIP-001`, signed distribution, notarization, and independent acceptance remain separate external work. The only claim made here is a static, reviewable readiness boundary for an honestly labeled unsigned candidate and its optional external signing receipts.
