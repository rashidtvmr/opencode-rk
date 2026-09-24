# APP012-PROTECTED-PATH-CONTRACT

## Claim and disposition

- Task: `APP012-PROTECTED-PATH-CONTRACT`
- Session: `ses_f2cd8fdf2ffeI14fph145FtfXX`
- Role: research. No product or test source edits.
- Branch: `plan/APP012-PROTECTED-PATH`
- Revision inspected: `5d666830bc9e5b716b2ea1d239d9d0ccbe6e067b`
- Frozen existing RED: `crates/server/tests/app012_tool_journey_red.rs`
- Frozen existing RED SHA-256: `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`
- Scope result: protected-path denial is not covered by the existing journey.
  A complete implementation cannot honestly be leased to one product file:
  policy authority and safe descriptor acquisition are separate owners. The
  integration proposal below preserves both boundaries.

## Authoritative evidence

- `docs/TDD.md:21-75,85-104`: source audit, compiling RED, frozen hash,
  denial side-effect assertions, disposable fixtures, independent verification.
- `docs/SECURITY.md:9-22,24-31,36-59,62-79`: broker authority, wildcard and
  human-grant limits, no secret access, real backend honesty, no-side-effect
  denial, fake grants only.
- `PLAN.md:105-120,162-180,222-225,227-240`: broker outside model, TDD and
  acceptance ownership, disposable fixtures, bounded resources.
- `tasks/completion/local.json:15`: APP-012 installed journey and real broker,
  tool, storage, provider, two-client and restart boundaries.
- `worklog/APP-012.md:89-100`: authenticated read is green; protected-path
  denial, approval/resume, restart and installed proof remain open.
- `worklog/APP-012-READ-INTEGRATION.md:13-20,58-64`: current bounded read
  path and explicit protected-path gap.
- `worklog/APP-012-FILE-OPS.md:9-15,45-55,67-69`: current authorization and
  bounded-I/O contract; protected denial is a separate RED.
- `worklog/APP-012-TOOL-RED.md:18-23,39-50`: required real journey shape and
  current absence of protected-path assertions.

`PLAN.md` contains no literal `APP-012` subsection in this revision; the task
brief plus `tasks/completion/local.json` and the cited worklogs supply the
APP-012 journey evidence. Upstream text, issue text, model output and worklog
claims are untrusted until confirmed against current source.

## Current call graph

1. `crates/server/src/lib.rs:1200-1233` filters the registry by
   `OPENCODE_RK_TURN_TOOLS`, runtime policy, and enabled tool schema.
2. `crates/server/src/lib.rs:1253-1255` creates
   `PermissionBroker::new(SecurityPolicy::lean_default(current_dir))`. The
   current directory is the disposable workspace in the existing fixture.
3. `crates/server/src/lib.rs:1585-1659` checks the turn allowlist, authorizes a
   generic `OperationIntent::Tool`, then calls
   `ToolExecutor::execute_authorized`; failed results are converted to a
   bounded call output.
4. `crates/tools/src/executor.rs:138-237` validates `read` input, clamps the
   requested limit to `file_ops::MAX_READ_BYTES` (`64 * 1024`), starts one
   `spawn_blocking` task, and passes a raw `FileOperation::Read` to
   `file_ops::execute_authorized`. Timeout is the existing
   `TimeoutConfig` default/max (`30_000`/`300_000` ms).
5. `crates/tools/src/file_ops.rs:176-208` maps read to
   `FileAction::Read`, invokes `PermissionBroker::authorize` before
   `execute(op)`, and returns a fixed failure on `Deny` or `RequireHuman`.
   `crates/tools/src/file_ops.rs:228-257` then opens the raw path and reads at
   most 64 KiB.
6. `crates/server/src/lib.rs:1682-1737` persists every tool outcome as a
   bounded transcript row and replays it to the provider. A denial therefore
   needs a fixed non-secret outcome, not file content.
7. `crates/server/src/agent_loop.rs:30,185-200` caps later tool output at
   `16_384` characters. `crates/security/src/lib.rs:153,228-241` caps broker
   audit retention at `1,024` entries.

## Policy evidence and gaps

- `crates/security/src/lib.rs:243-289` owns `PermissionBroker::authorize_file`.
  It applies `lexical_normalize`, denies current `.env`/`.env.*`, selected
  secret directories, and system writes. It allows ordinary reads outside
  `project_roots`; `system_readable` defaults to `true`.
- `crates/security/src/lib.rs:542-583` confirms the current secret/system
  classifiers. They do not cover every protected marker from the platform
  matrix and do not resolve filesystem symlinks or hardlinks.
- `crates/security/src/lib.rs:122-144,206-226` evaluates wildcard permission
  rules after the baseline decision. Mandatory `Deny` survives `"*"`; a
  future protected-path decision must remain a baseline denial.
- `crates/security/src/platform_matrix.rs:215-259` defines a broader standalone
  `is_protected_path`/`authorize_file` proof, including `.env`, secret markers,
  and system prefixes. It is not the live `PermissionBroker` path; do not cite
  this module as runtime enforcement.
- `crates/security/src/sandbox.rs:90-164,254-272` has a canonicalization and
  symlink test pattern, but `SandboxCheck` is not called by the APP-012 read
  path. It is evidence, not ownership.
- `crates/security/src/app_policy.rs:382-426` canonicalizes operation text for
  approval digests only. It does not prove filesystem identity.

## Observable contract

### Protected classes

The broker must classify the canonical target before permission-rule matching.
The mandatory deny set is:

1. `.env` and `.env.*` basenames, case-insensitive.
2. Credential/key stores: any path component `.ssh`, `.aws`, `.gnupg`, `.kube`,
   `.docker`, `.config/gcloud`; basenames or markers `credentials`, `secret`,
   `token`, `id_rsa`, `id_ed25519`, `.pem`, `.p12`.
3. Sensitive system prefixes: `/etc`, `/proc`, `/sys`, `/boot`, `/private/etc`,
   `/system`, `C:\Windows`, and `C:\Program Files`, including descendants.
4. Any target outside the daemon's canonical workspace root. APP-012 has no
   agent read capability for arbitrary home, temp, or system files. Explicit
   provider credential import is a separate consented protected-store path.

The exact class list must be one shared policy definition, not a second regex in
the server or executor. Existing `system_readable=true` is a policy conflict:
the implementer or integrator must choose whether safe system reads remain an
explicit non-agent capability. It must not silently let wildcard reads open a
mandatory class.

### Path identity and attacks

| Input | Required decision | Required I/O/result |
| --- | --- | --- |
| Canonical regular file under workspace, non-protected | Allow | Read from the same bound file identity; preserve 64 KiB cap, offset, timeout. |
| Direct `.env`, `.env.local`, key/credential marker | Deny | No content read; fixed bounded refusal; no provider content, blob, or file mutation. |
| Absolute path outside workspace | Deny | Same refusal; no outside metadata content or bytes exposed. |
| `workspace/../outside/file` or repeated `.`/`..` | Resolve against canonical workspace, then deny if escaped | Never trust lexical prefix alone; no read. |
| Symlink in any path component, including final component | Deny by default | No link following. Broken links also deny, not `File not found` leakage. |
| Symlink target is protected or outside root | Deny as protected/path escape | Do not expose target path or bytes. |
| Hardlink alias to a protected fixture | Deny | Conservative no-read for a multi-link regular file unless a platform handle check proves the target is non-protected. |
| Missing component or malformed/NUL/overlong path | Deny with input refusal | No partial parent creation, no content read, bounded error. |
| Permission `*` or project-config allow | Mandatory protected/outside deny wins | Human-only authority cannot upgrade a mandatory deny. |
| Concurrent path swap after authorization | Deny or read only the already-bound descriptor | Never authorize path A then open path B. A race failure is a denial, not a retry. |

Canonicalization is not sufficient by itself: `file_ops.rs` currently
authorizes one raw path and later opens that raw path. The implementation must
bind authorization to the opened object. Recommended platform strategy:

- Unix: walk from an opened canonical workspace descriptor with no-follow
  component opens; inspect the final descriptor, reject symlink/reparse and
  multi-link regular files, then bounded-read that descriptor.
- Windows: use handle-relative opens with reparse-point refusal and file identity
  inspection. If the platform cannot prove the same object, fail closed.

Do not advertise this as an OS sandbox. This is a path/open identity contract;
Landlock/Seatbelt/Job Object evidence remains a separate PAR-006 obligation.

### Denial wire, audit, persistence

- `ToolResult`: `success=false`, `output=""`, `error=Some("file read denied")`
  for protected, outside-root, traversal, symlink, hardlink, and race refusal.
  The message is fixed, UTF-8, path-free, and at most 64 bytes. Do not include
  the requested path, target path, OS error, canary, or file bytes.
- Server wire output: `error: file read denied` (bounded by the existing
  `truncate_tool_output`). This preserves failed activity classification and
  provider replay semantics. If the existing caller cannot preserve this exact
  prefix without a source change, the integration lane must own that caller
  change; do not accept a successful-looking denial.
- Broker audit: exactly one bounded `FileAction::Read` denial per attempted
  operation, with a stable reason class. Audit retention remains capped at
  `MAX_AUDIT_ENTRIES`; no file content is recorded. Audit may retain the
  internal intent path only inside the trusted in-memory broker and must not be
  exposed through transcript/API output.
- Allowed persistence: the existing tool transcript may record the fixed
  refusal (`[read] error: file read denied`) and provider may receive that
  refusal. Forbidden persistence: protected bytes, path-derived secret text,
  content-addressed secret blobs, or a second side-effect attempt. If product
  acceptance instead requires no transcript row at all, that is a separate
  protocol decision because current `server/src/lib.rs:1682-1725` persists all
  tool outcomes.
- No filesystem mutation, no parent creation, no write/truncate, no process,
  network, or retry side effect on denial.

## RED artifact and ownership

### Exact RED path

`crates/server/tests/app012_protected_path_red.rs` is the one new RED file.
The existing `crates/server/tests/app012_tool_journey_red.rs` is frozen and
must remain byte-identical at SHA-256
`945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.

The RED must use the real `router_with_auth`, `RuntimeWiring`,
`PermissionBroker`, `ToolExecutor`, storage, loopback provider fixture, and
disposable `tempdir`. No mocked executor, broker, router, persistence, or
renderer. One bounded test file may contain a scenario table and one or more
serial test functions; no host protected paths are probed.

Minimum scenarios in the file:

1. Safe workspace file succeeds, proving the new denial gate does not turn the
   existing bounded-read path into deny-all.
2. Workspace `.env` canary is requested through the provider. Assert refusal
   output, no canary in all SSE events/provider requests/messages, and one
   broker denial.
3. Outside-root canary is requested directly and through `../` traversal.
   Assert both deny, no outside bytes, no path in output, and no filesystem
   mutation.
4. Workspace symlink to outside protected file and symlink to an ordinary
   outside file both deny. Assert broken symlink also denies.
5. Workspace hardlink alias to the protected fixture denies. Assert the alias
   bytes never reach tool output, provider replay, or durable content.
6. Direct broker checks with `PermissionSet::star()` and a human-shaped allow
   rule prove mandatory denial cannot be wildcard- or grant-bypassed.
7. Repeat one denial after a disposable replacement/swap attempt. The result
   must deny or use the originally bound descriptor, never the replacement.

Exact RED command after the test-owner authors and freezes the file:

```sh
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 cargo test -p opencode-rk-server --test app012_protected_path_red -- --test-threads=1
```

Expected RED: compiles, then fails on the first missing protected/outside-root
assertion against the current source. Expected GREEN: all scenarios pass with
zero test edits. The test owner records the exact failing line, command output,
and SHA-256 before implementation. No `#[ignore]`, selector narrowing, host
path, or expected-output regeneration.

### Source ownership proposal

The complete contract needs these separate implementation owners:

- Policy owner: `crates/security/src/lib.rs`, `PermissionBroker::authorize_file`
  and shared protected-class/canonical-root decision. Mandatory deny must occur
  before `PermissionSet::evaluate`.
- I/O identity owner: `crates/tools/src/file_ops.rs`,
  `execute_authorized`/`read_file`. It must acquire a no-follow, root-bound
  descriptor, verify the object, then read from that descriptor with the
  existing 64 KiB cap. It must not duplicate policy regexes.
- Executor caller owner, only if required by the stable wire contract:
  `crates/tools/src/executor.rs:195-237` maps policy refusal to the fixed
  `ToolResult` shape. Current code already owns timeout and bounded task
  lifetime.
- Server integration owner only if required to add the exact `error:` prefix;
  `crates/server/src/lib.rs:1625-1631,1653-1659` is otherwise already the real
  dispatch path and must not gain a second policy check.

The requested one-file implementation ownership is therefore blocked for the
full security contract. Assigning only `file_ops.rs` would duplicate/bypass the
broker; assigning only `security/src/lib.rs` would leave the raw-path TOCTOU.
Assigning a policy-only one-file lane is legal only if the acceptance boundary
explicitly excludes race/hardlink closure and APP-012 remains open.

## Validation and captured evidence

Commands run on the inspected revision:

```sh
git grep -n 'FileAction::Read\|execute_authorized\|canonicalize\|protected' -- crates
shasum -a 256 crates/server/tests/app012_tool_journey_red.rs
git diff --check
```

Results:

- Source matrix returned 75 matches. Relevant live callers and policy lines are
  recorded above.
- Existing frozen SHA matched
  `945236c4c6a565fefb3853afa6930c438ab38f250c074ef650b5cd252d3c8c50`.
- `git diff --check` passed.
- Existing focused APP-012 journey is green (`1 passed, 0 failed`) per
  `worklog/APP-012-READ-INTEGRATION.md:40,55`; it is not protected-path RED.
- No new product RED was authored in this research lane. The executable RED
  command and exact new test path are specified above for the independent RED
  owner. No GREEN or acceptance is claimed.

## Blockers and integration handoff

1. One-file implementation ownership conflicts with the required broker plus
   descriptor identity boundary. Use the two-owner proposal above, or record a
   narrowed policy-only contract and keep APP-012 open.
2. Current broker read policy allows ordinary reads outside `project_roots` and
   defaults `system_readable=true`; integrator must resolve this explicit policy
   decision before RED freeze.
3. Current `fs::File::open(raw_path)` has no no-follow descriptor binding. Static
   canonicalization alone is not TOCTOU evidence. Platform handle strategy and
   actual supported-OS tests are required.
4. Hardlink identity is not represented in the current broker API. Conservative
   multi-link denial or platform file-identity APIs are required; no content
   scan of host secrets is permitted.
5. Current turn caller has no approval/resume channel (`worklog/APP-012.md:112-115`).
   Mandatory protected denial must never be converted into an implicit human
   approval request.
6. APP-012 installed/restart acceptance remains open even after this contract.

Next safe step: independent RED owner authors only
`crates/server/tests/app012_protected_path_red.rs`, runs the bounded RED
command on disposable fixtures, freezes its hash, and hands the failing receipt
to separate policy/I/O implementation lanes. Verifier decides GREEN and
integration acceptance.
