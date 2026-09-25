# PATH-OPEN-RED scratchpad

Claim: PATH-OPEN-RED, session ses_f3c4de578ffelQv59xDXmOs03B.

Base: `a340af69617f11ebe3ad4ceaf527daa8761d52f9` on
`red/APP012-AUTHORIZE-OPEN`; dependency `PATH-HARDLINK-RED` is present with
frozen hash `71bf5f632a44a3602fe7f188a16886ad04ea87b2da7d62da8506a79260413903`.

Source evidence:

- Current security API, `crates/security/src/lib.rs:206-289`, returns a path
  decision only. It exposes no opened descriptor/handle, identity token, or
  deterministic pre-open synchronization seam.
- Current tools caller, `crates/tools/src/file_ops.rs:173-200`, authorizes only
  write operations on this baseline; reads pass directly to `execute`.
- Current I/O, `crates/tools/src/file_ops.rs:203-247`, receives the raw path and
  calls `fs::read_to_string(path)`, so the object opened is not bound to any
  earlier authorization.
- Existing verified contract,
  `worklog/APP012-PROTECTED-PATH-CONTRACT.md:128-141`, requires denial or reading
  only an already-bound descriptor after a concurrent path swap.
- Prior RED audit,
  `worklog/APP012-PROTECTED-PATH-RED.md:31-44`, explicitly removed a timing-based
  rename/replace assertion because no deterministic descriptor seam exists.
- Synthesis assigns `PATH-OPEN-RED` to
  `crates/security/tests/app012_authorize_open_red.rs` and the later
  implementation only to `crates/security/src/lib.rs`.

Status: BLOCKED before test authoring. A security-crate integration test cannot
import the downstream tools crate without a dependency cycle, and the current
security public API cannot perform or bind an open. Inventing an
`authorize_open` import would be a compile failure, not RED. A timing-only race
against `fs::read_to_string` would be nondeterministic and could not prove the
same-object guarantee. The planned implementation path also cannot wire the
real `FileTool` caller by editing `security/src/lib.rs` alone.

Required authority action: prewire an implementation-independent compiling seam
that either (a) returns an owned, already-authorized descriptor/handle from a
security service consumed by `FileTool`, or (b) moves the frozen behavioral RED
to the tools/server boundary and grants the corresponding caller ownership.
Tests must then deterministically coordinate replacement between path selection
and open, and assert denial or bytes from the originally bound object.

No test hash was frozen. No product, test, manifest, library, schema, or verifier
file was changed. This blocker keeps `V1-FREEZE-RED` open.
