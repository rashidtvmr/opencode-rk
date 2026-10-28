# Skill Prepare Formatter - Independent Security Review B

Updated: 2026-10-26; role: independent security evaluator B
Base SHA: 6f6872bf87e02f05afe9c1a5f63e55fe4c040499 (canonical read-only)
Upstream pin: 450e7b92412c780a7b5a75226bb8d49aaca24e22 (no skill_prepare.rs exists upstream)
Candidate: skill_prepare.rs, SHA256 a87d035a76acf0ed2e02c1f50ec8a2f537214ed8dd93019aa20bd739f6ae6301
Base v5: SHA256 acfe3ebf53f1dd91eb0ebaa77571f6b7b8ed9074c7faf4e759c283d7cf4f9199
Reviewer scope: READONLY candidate diff review, no Cargo/rustc/PTY/network/DB/Git mutations.

## Verdict: APPROVE

## Evidence read

- Candidate file: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/concurrency20/skill-prepare-v6-integrator/skill_prepare.rs (11590 bytes, 194 lines)
- SHA256 of candidate file matches executed-verification.json source_sha256.
- Test binary: skill-tests (Mach-O 64-bit arm64 executable, 1219880 bytes, SHA256 546ff08212fab367d3bb458bb94dcfdf74903da2bf25b86788b084e64a493f8d)
- test.stdout: 11 PASS, 0 FAIL, test_exit 0
- compile.stderr/compile.stdout: empty (zero bytes), compile_exit 0
- Broker reader v2: /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/concurrency20/skill-reader-v2/skill_source_reader.rs
- Broker reader v1 (prior): /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/concurrency20/skill-reader/skill_source_reader.rs
- Existing canonical modules: crates/tools/src/skill_gate.rs, crates/tools/src/skill_commands.rs, crates/tools/src/skill_defs.rs
- AGENTS.md / docs/SECURITY.md / docs/AGENT_WORKER_PRACTICES.md

## What the candidate IS

skill_prepare.rs is a PURE BOUNDED FORMATTING LAYER. It contains no `unsafe`
(#![forbid(unsafe_code)] on line 1). It defines:

- Capacity constants (MAX_SKILL_ID_BYTES, MAX_SKILL_NAME_BYTES, etc.)
- PrepareError enum with typed variants
- TraversalLimits and TraversalExhaustion
- AuthorizedSkillRead, AuthorizedRelatedFile (data structs only - no I/O)
- SkillSourceReader trait (abstraction boundary)
- SkillRecord, PreparedSkill (data structs)
- valid_metadata, valid_directory, valid_related_path (pure validation)
- add_len helper (checked arithmetic)
- prepare_skill function (pure: validates inputs, calls reader, formats output)
- Inline tests (11 tests, all passing)

Key security property: This module does NOT claim to implement filesystem
authorization. Line 3-6: "Filesystem authorization and no-follow traversal
belong to the broker-owned SkillSourceReader implementation. This module only
validates returned data and formats the pinned upstream model payload."

## What the candidate is NOT

This is NOT a complete integration. The real broker-owned filesystem adapter
(skill_source_reader.rs v2) is a SEPARATE file that imports from this module.
The prepare layer is the safe consumer of data returned by that broker reader.

## Security review by property

### 1. No fake filesystem authorization
APPROVED. The file explicitly disclaims filesystem authority. The trait
SkillSourceReader is an abstraction; the actual broker-owned implementation
(resides separately) performs the real authorize/read/no-follow/traversal.
This module validates the RETURNED data structure, not the filesystem.

### 2. Output cap / path controls / read contracts
APPROVED. All checks verified:

- Line 88-91: valid_metadata rejects control chars, DEL, and optionally
  XML metacharacters (< > " &). Applied to id, name (markup=true), description
  (markup=false). This prevents XML injection in the skill_content/name attribute
  and skill name field.

- Line 93-95: valid_directory calls valid_metadata with markup=true and
  additionally rejects \n and \r. This prevents newline injection in the
  "Base directory" text field that follows directly in the output string.

- Line 97-106: valid_related_path rejects: empty, >4096 bytes, backslash,
  leading '/', colon (Windows drive), control chars, DEL, XML metacharacters
  (< > & "), and path traversal via . and .. segments. Related paths are
  inserted raw into <file> wrappers (no escaping), so rejecting delimiters is
  the correct choice per line 100-103 comment.

- Line 120: source_basename validated to 256 bytes with markup=true.
  Only "SKILL.md" triggers related_files traversal (line 124) - a non-SKILL.md
  basename never invokes the reader's traversal API.

- Line 108-112: add_len uses checked_add and enforces MAX_SKILL_OUTPUT_BYTES
  (112KB) BEFORE writing. No silent truncation. The debug_assert_eq on line 147
  verifies output.len() matches computed length.

- Line 121: content.length checked against MAX_SKILL_CONTENT_BYTES (96KB)
  before UTF-8 conversion.

- Line 126: related_files count checked against MAX_RELATED_FILES (10) AFTER
  reader returns but BEFORE use.

### 3. Explicit unresolved wiring (NOT a security gap in this file)
The candidate PrepareError enum (lines 24-34) does NOT include SymlinkRejected,
NotRegularFile, or Io variants. The v2 skill_source_reader.rs (lines 32-35)
imports and uses these variants:
- SymlinkRejected (reader lines 74, 99, 101, 128)
- NotRegularFile (reader line 135)
- Io (reader lines 106, 138, 140, 143, 145, 151)

This file has ReaderContractViolation (line 33) which is likely meant to cover
reader-level failures, but the exact variant set is MISMATCHED with the v2
reader's expectations. This is an integration/wiring gap, NOT a security
vulnerability in the prepare layer itself. The prepare layer correctly
propagates whatever PrepareError the reader returns without fallback
(test line 180 confirms reader_denial_is_propagated_without_fallback).

### 4. Test integrity
VERIFIED. The test binary is a real Mach-O arm64 executable (1219880 bytes).
strings output confirms all 11 test names are embedded. test.stdout shows
11 passed, 0 failed. compile.exit=0 with empty stderr.

Key tests that validate security properties:
- exact_normal_wrapper_and_raw_body: output format is exact/predictable
- directory_control_and_markup_are_rejected: newline, DEL, XML chars in
  directory rejected
- invalid_unix_windows_and_markup_related_paths_rejected: traversal (../, /),
  backslash, drive letters, double-slash, self-ref, XML chars all rejected
- reader_denial_is_propagated_without_fallback: PermissionDenied from reader
  is NOT swallowed or downgraded
- output_preflight_rejects_without_silent_truncation: 6 distinct 4096-byte
  paths with full-size content body exceeds MAX_SKILL_OUTPUT_BYTES, returns
  OutputTooLarge (not silent truncation)
- non_skill_md_never_walks_related_files: only SKILL.md triggers traversal
- content_overflow_is_rejected_before_render: body > MAX_SKILL_CONTENT_BYTES
  rejected before formatting

## Rejected alternatives
None needed; the candidate is minimal and focused. No over-engineering.

## Decision rationale

The skill_prepare.rs candidate is a well-designed PURE layer that correctly:
1. Enforces strict bounded outputs (checked arithmetic, no truncation)
2. Validates all metadata fields against control-char and XML-injection vectors
3. Validates related paths against traversal, delimiter, and injection attacks
4. Does NOT claim filesystem authorization (explicitly disclaimed)
5. Propagates reader errors without fallback
6. Does NOT read related file bodies (only paths, matching v2 reader behavior)
7. Only triggers traversal for SKILL.md basename

The only gap is the PrepareError variant mismatch with the v2 reader
(SymlinkRejected/NotRegularFile/Io vs ReaderContractViolation). This is an
integration detail to resolve when wiring the two files together, not a
security flaw in the prepare layer. The prepare layer's contract is correct:
it accepts any PrepareError from the reader and formats/validates the rest.

This matches the existing security posture of the canonical crate
(skill_gate.rs uses similar pattern: lexical validation, no-follow opens,
owner-only permissions, create_new to prevent overwrites).

## Remaining uncertainties
1. PrepareError variant mismatch with skill_source_reader.rs v2 - needs
   reconciliation when wiring into the actual crate module tree.
2. This file is not yet integrated into crates/tools/src/lib.rs as a module.
   Integration is the integrator's responsibility, not this review's scope.

## Artifacts
- Verdict file: this file
- SHA256 of this verdict: will be computed by reader
