# PHASE1-LINUX-RUNNER-CLOSURE: Linux x64 and arm64 external-runner closure

Claim: session `ses_f28972554ffe2Af4z7y1ZT4aFO`, task
`PHASE1-LINUX-RUNNER-CLOSURE`, branch `plan/PHASE1-LINUX-RUNNER-CLOSURE`, HEAD
`7262682e31c1f473912c9483804c9430eb4ee4c5`, scratchpad
`worklog/PHASE1-LINUX-RUNNER-CLOSURE.md`.
Scope: text/static closure specification only. No Cargo, workflow, product or
test file edit. No runner evidence is claimed to exist; every execution receipt
below is UNRUN and must be produced by a real Linux host.
Disposition: this artifact makes the Linux external-runner lane executable and
bounded. It does not accept APP-012, does not close `G-LINUX-RUNNER`, and does
not assert Landlock isolation from anything in this repository today.

## 0. Authority and source revisions

| Ref | Revision | Use here |
|---|---|---|
| Linux vertical map (audited, landed) | `b924d9a` (SHA-256 `58b12ddc32b76a13b1a1e31bb09c4ef36910d7ab22b69082bfd55b3390e1df27`) | runner requirement, Landlock/installer/APP-012 gaps |
| Vertical synthesis (landed) | `7262682` (this HEAD; SHA-256 `c257818712b0a6b92a0dded065cf7d33f10c0801c3c7604d283276ea8caa4571`) | `G-LINUX-RUNNER`, LINUX-* / V2-LINUX-* stages |
| Native candidate (pushed, not main) | `origin/lane/CROSS-PLATFORM-RUNNER-CLOSURE-VERIFY` `2d04c1c925595b566f617b073129ecee24593eb9` | artifacts.json/sbom.json/build_opentui.sh and bounded installer 427 lines |
| Bounded installer candidate | `plan/packaging-matrix` `4771e3b8f53f6a26a754268896e4fe6e16a496e2` (== installer at `2f87242`) | bounded POSIX consumer |

`b924d9a` is NOT an ancestor of `7262682` (it sits on
`plan/PHASE1-VERTICAL-LINUX`); it is cited as an audited source plan, not as
integrated source truth. Raw `git` ancestry:

```
$ git merge-base --is-ancestor b924d9a HEAD   -> exit 1 (not ancestor)
$ git rev-parse HEAD                          -> 7262682e31c1f473912c9483804c9430eb4ee4c5
$ git rev-parse origin/main                   -> 8a91a7b49a5a1c948218ad8f176d44e015530dcb
```

## 1. Verified current-state evidence (file:line)

| Claim | Verdict | Exact evidence at `7262682` |
|---|---|---|
| Landlock is detection-only, fail-closed | CONFIRMED | `crates/security/src/os_backend.rs:120` `let enforcement_linked = false;`; `:151 pub fn require_supported` returns `Err(Blocked)` while `available == false`; `:171 pub fn engage` always returns `Err(Blocked)`; `crates/security/src/platform_matrix.rs:155 pub fn enforce` always returns `Err(UnsupportedSandbox)`. Detection only: `os_backend.rs:338 detect_landlock` = `/proc/version` >= 5.13 AND `:363 filesystems_has_landlock`. |
| No unsafe / no libc in security crate | CONFIRMED | `crates/security/src/lib.rs:2 #![forbid(unsafe_code)]`; `crates/security/Cargo.toml` deps = contracts, serde, serde_json, thiserror only. A syscall backend must be a separate crate. |
| Inherited-capability hygiene exists but is NOT a sandbox | CONFIRMED | `os_backend.rs:283 restricted_spawn` uses `env_clear()` + null stdio; its doc comment at `:280-282` says "process hygiene, NOT an OS sandbox". `os_backend.rs:308 count_open_fds` reads `/proc/self/fd`. |
| x64 Linux native present on main but stale vs candidate manifest | CONFIRMED | `crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so` = `e84ced36b9f0d77069067833f1dd0edcc6324abdd5e4be662e1dccd2994b5d9a`, 26583552 bytes (local `shasum -a 256`, `stat -f%z`). Candidate artifacts.json records x64 = `9f074adf...` (26627832 bytes). The main blob is NOT the manifest-verified artifact. |
| arm64 Linux native absent on main | CONFIRMED | `git ls-files crates/opentui-bridge/native/` at HEAD = the single x64 `.so`. No `aarch64-unknown-linux-gnu`, no `build_opentui.sh`, no `artifacts.json`, no `sbom.json` (all present only on `2d04c1c`). |
| POSIX installer on main has no native/bounds gate | CONFIRMED | `scripts/install-oc2.sh` = 144 lines; `grep "MAX_ARCHIVE\|EXPECTED_NATIVE\|LIB_SUFFIX\|134217728"` = empty. It installs only `oc2`, never `libopentui.so`. Bounded variant (427 lines, `MAX_ARCHIVE_MEMBERS=4`, `MAX_ARCHIVE_PAYLOAD=134217728`, symlink refusal, rollback) exists only on `2d04c1c` / `4771e3b8`. |
| No Linux release/runner job | CONFIRMED | `.github/workflows/ci.yml:33` `matrix.os: [ubuntu-latest, windows-latest]`; no `--target`, no artifact upload, no SBOM step; only `ci.yml`, `completion.yml`; no `release.yml`, no `release/`. |
| APP-012 open, its path missing | CONFIRMED | `tasks/completion/local.json:15` APP-012 `paths:["tests/e2e/local_application.rs"]`; that directory does not exist (no `tests/e2e/`). |
| `installed_default_entrypoint.rs` is a stub suite, not macOS-gated | CORRECTION to `b924d9a` | `b924d9a` map lines 43 and 211 cite line 1 as `#![cfg(target_os = "macos")]`. At `b924d9a` and at HEAD line 1 is `//! Installed default-entrypoint journey...` and `grep -c cfg` = 0. The real defect is five `todo!(...)` bodies at `crates/cli/tests/installed_default_entrypoint.rs:69,79,88,98,106`. It is a non-executing stub suite on every platform, not a macOS-only file. This block is controller/test-author owned; no lane may edit it. |
| Bounds targets exist, whole-tree measurement does not | CONFIRMED | `config/resource-targets.json` `idleDaemonRssMiB:64`, `idleDaemonAndNativeTuiRssMiB:96`, `maxOpenWorkspaceStores:4`, requirement `Measure process tree not only main binary`. No Linux whole-tree measurement lane landed. |

`python3 tools/validate_repository.py` = FAIL (`backlog exhaustion`, 51
inherited errors). `python3 tools/convergence_gate.py` = BLOCKED, `total=59`
(58 inherited baseline + the untracked prior candidate). Both are controller
owned; this lane records them, does not fix them.

## 2. Runner requirements (the external gate)

`G-LINUX-RUNNER` (synthesis `7262682`, externalGates) stays BLOCKED until a real
Linux kernel produces the receipts in section 6.

| Lane | Runner | Why this exact runner |
|---|---|---|
| `V2-LINUX-INTEGRATED` x64 | `ubuntu-24.04` (x86_64, kernel >= 6.8) | hosted kernel has Landlock ABI >= 3; unprivileged `landlock_restrict_self` permitted with `no_new_privs` |
| `V2-LINUX-INTEGRATED` arm64 | `ubuntu-24.04-arm` native arm64 | Landlock must be proven on the target kernel; `qemu-user` is NOT acceptable (syscall emulation is not kernel enforcement proof) |
| local dev | any Linux >= 5.13 with `landlock` in `/proc/filesystems` | RED authoring only; not acceptance |

Kernel precondition command, must exit 0 before any Landlock claim:

```sh
grep -iq landlock /proc/filesystems
uname -r
```

If Landlock is absent or disabled at boot, the closure returns `BLOCKED`
(exit 75, section 5). It must never record `ok` from detection alone.

## 3. Real Landlock attach and inherited FD/env denial

### 3.1 What the kernel actually mediates (binding correctness rule)

Per `landlock_restrict_self(2)` and the kernel `userspace-api/landlock.html`
documentation: Landlock checks rights at `open(2)` time, not at `read(2)` /
`write(2)`. A file descriptor opened before enforcement keeps its access; "files
or directories opened before the sandboxing are not subject to these
restrictions". It is recommended to close inherited TTY and other descriptors.

Consequence for this closure, stated plainly so no lane over-claims:
- Landlock proves denial of new `open()` of out-of-grant paths
  (write marker absence, secret read denial, `/proc/self/fd` reopen denial).
- Landlock ALONE does NOT revoke a pre-opened inherited descriptor. The test
  therefore asserts the implementation closed inherited descriptors at spawn
  (`env_clear` + stdio null + explicit fd close before `exec`); that assertion
  is process-hygiene evidence, and it is recorded as such, not as kernel proof.

### 3.2 Attach contract (the missing backend)

The real backend must live in a crate that may use `unsafe` (a new
`crates/security-landlock/src/lib.rs`), because `crates/security/src/lib.rs:2`
forbids unsafe. Contract it must satisfy:

1. `landlock_create_ruleset` returns fd >= 0; probe ABI via
   `landlock_create_ruleset(NULL, 0, LANDLOCK_CREATE_RULESET_VERSION)`.
2. `landlock_add_rule(fd, LANDLOCK_RULE_PATH_BENEATH, ...)` per `FsGrant`
   (`os_backend.rs:188 FsGrant`, `writable` selects write/truncate bits). Only
   declared roots are added; no implicit `/`.
3. `prctl(PR_SET_NO_NEW_PRIVS, 1, 0, 0, 0)` then `landlock_restrict_self(fd, 0)`
   returns 0. Only then may `engage()` (`os_backend.rs:171`) return `Ok`.
4. One ruleset handle per engage, closed on drop; no retained fd.
5. Empty grant list is refused, never treated as "allow all".
6. ABI/feature probe failure returns typed `Blocked`, never `Ok`. No panic on
   kernel < Landlock ABI v1.

RED owner: `crates/security/tests/phase1_landlock_enforcement.rs` (new file,
must compile and fail for the absent backend before implementation; frozen
after). The test must assert absence of side effects, not an error string:

- allowed fixture write under a granted root succeeds;
- denied write outside the grant leaves no file on disk (`assert!(!marker.exists())`);
- read of an out-of-grant secret path is denied by the kernel;
- a pre-opened out-of-grant fd is closed by the spawn helper and cannot be read;
- child env is empty (`env_clear`) so no `*_KEY`/`*_TOKEN` leaks;
- `/proc/self/fd` count is equal before and after 50 setup/teardown cycles;
- kernel without Landlock returns explicit `BLOCKED`, never silent allow.

## 4. Native artifacts, hashes and arm64 closure

Honest state: the checked-in `main` x64 blob (`e84ced36...`) is not the
manifest-verified artifact, and arm64 Linux bytes do not exist on `main`. The
runner must build/verify both from a pinned builder, then bind the produced
bytes to `artifacts.json` on the integrated revision. Candidate manifest values
(read from `2d04c1c`, NOT executed here) are the target to reproduce:

| Target | Path | sha256 (candidate) | size (candidate) |
|---|---|---|---|
| `x86_64-unknown-linux-gnu` | `lib/x86_64-unknown-linux-gnu/libopentui.so` | `9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e` | 26627832 |
| `aarch64-unknown-linux-gnu` | `lib/aarch64-unknown-linux-gnu/libopentui.so` | `e85a45710e9e181b3eb7cca877a1d9022f2210bfa1e06b7c159e734506da3b89` | 26598960 |

Build/verify commands (builder script exists only on the candidate; the
integration lane must land it first). All bounded, one Cargo-heavy process at a
time per AGENTS.md:

```sh
bash crates/opentui-bridge/native/build_opentui.sh --verify-only \
  crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so --target x86_64-unknown-linux-gnu
bash crates/opentui-bridge/native/build_opentui.sh --verify-only \
  crates/opentui-bridge/native/lib/aarch64-unknown-linux-gnu/libopentui.so --target aarch64-unknown-linux-gnu
shasum -a 256 crates/opentui-bridge/native/lib/*/libopentui.so
```

Acceptance rule: on the integrated revision, `shasum -a 256` of each shipped
Linux blob equals its `artifacts.json` entry, and x64 and arm64 entries cannot
silently point at one another. A manifest hash that does not match the shipped
blob is FAIL. A stale `e84ced36...` blob without a manifest entry is FAIL until
deliberately re-bound.

## 5. Failure states

Every command below exits with one of these codes; the receipt records it.

| Code | Name | Trigger |
|---|---|---|
| 0 | ok | receipt written, all assertions passed |
| 65 | EX_DATAERR | archive/hash/manifest divergence; expanded payload over bound |
| 69 | EX_UNAVAILABLE | no Linux kernel runner; `/proc` absent; required tool missing |
| 70 | EX_SOFTWARE | harness/internal error; handle or fd leak detected |
| 71 | EX_OSERR | syscall failure (`landlock_*`, `prctl`) with errno |
| 73 | EX_CANTCREAT | unsafe archive member/path traversal; install dir not writable |
| 74 | EX_ULIMIT | installed `oc2 --version`/`--help` identity fails; binary removed |
| 75 | EX_BLOCKED | Landlock unsupported or disabled at boot; explicit BLOCKED, never allow |
| 76 | EX_FDLEAK | inherited descriptor not closed before exec |
| 77 | EX_ENVLEAK | non-empty child environment or secret-shaped variable present |
| 78 | EX_RESOURCE | whole-tree RSS or open-store count over `config/resource-targets.json` |

`EX_ULIMIT` (74) is chosen to keep the existing main installer identity gate
(`scripts/install-oc2.sh:105,124` already `exit 74`) and the new runner codes
disjoint.

## 6. Exact bounded commands and versioned receipt

Resource policy for every command: `CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1`,
`timeout` per command, one Cargo-heavy process at a time, disposable HOME and
data dir only, never the user's real database.

```sh
# 1. Landlock RED/GREEN on the real kernel (x64 and arm64 runners)
grep -iq landlock /proc/filesystems || exit 75
CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 \
  cargo test -p opencode-rk-security --test phase1_landlock_enforcement -- --test-threads=1

# 2. Native artifact integrity + arm64 bytes
bash crates/opentui-bridge/native/build_opentui.sh --verify-only \
  crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so --target x86_64-unknown-linux-gnu
shasum -a 256 crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so  # expect 9f074adf...
shasum -a 256 crates/opentui-bridge/native/lib/aarch64-unknown-linux-gnu/libopentui.so  # expect e85a4571...

# 3. Bounded POSIX installer into a disposable HOME (427-line variant)
tmp_home="$(mktemp -d)"; trap 'rm -rf "$tmp_home"' EXIT INT TERM
HOME="$tmp_home" sh scripts/install-oc2.sh \
  --archive release-linux-x64.tar.gz --checksum "$ARCHIVE_SHA256" \
  --install-dir "$tmp_home/.local/bin" || exit 65
test -f "$tmp_home/.local/bin/oc2" || exit 65
test -f "$tmp_home/.local/bin/native/lib/linux-x64/libopentui.so" || exit 73
readelf -d "$tmp_home/.local/bin/oc2" | grep NEEDED

# 4. Installed APP-012 headless PTY journey from the clean HOME
HOME="$tmp_home" CARGO_BUILD_JOBS=1 RUST_TEST_THREADS=1 timeout 300 \
  cargo test -p opencode-rk-cli --features native --test local_application -- --test-threads=1

# 5. Whole-tree resource measurement (process tree, not the daemon alone)
for p in $(pgrep -f 'oc2|opentui'); do \
  awk '/VmRSS/{print FILENAME, $2}' /proc/$p/status; done
```

Receipt JSON (versioned, bounded; written per lane, unsigned):

```json
{
  "schema": "phase1.linux-runner.receipt.v1",
  "lane": "PHASE1-LINUX-RUNNER-CLOSURE",
  "status": "unrun",
  "revision": "7262682e31c1f473912c9483804c9430eb4ee4c5",
  "target": "linux-x64",
  "runner": {
    "os": "ubuntu-24.04",
    "arch": "x86_64",
    "kernel": "",
    "landlock_in_proc_filesystems": false,
    "landlock_abi": 0
  },
  "landlock": {
    "ruleset_attached": false,
    "restrict_self_ret": -1,
    "denied_write_marker_absent": false,
    "secret_read_denied": false,
    "inherited_fd_closed": false,
    "child_env_empty": false,
    "fd_before": 0,
    "fd_after": 0
  },
  "artifacts": [
    {"path": "crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so",
     "sha256": "e84ced36b9f0d77069067833f1dd0edcc6324abdd5e4be662e1dccd2994b5d9a",
     "size_bytes": 26583552, "manifest_match": false}
  ],
  "installer": {
    "archive_sha256": "",
    "member_count": 0,
    "expanded_payload_bytes": 0,
    "identity_exit": 0,
    "native_lib_installed": false
  },
  "app012": {"installed_binary": false, "bare_launch_ok": false, "restart_resume_ok": false},
  "resources": {
    "peak_tree_rss_mib": 0,
    "idle_target_mib": 64,
    "idle_native_tui_target_mib": 96,
    "open_workspace_stores": 0,
    "max_open_workspace_stores": 4
  },
  "unsigned": true,
  "signing": "not-run",
  "exit_code": 69,
  "failure_code": "EX_UNAVAILABLE",
  "note": "No Linux kernel runner was available; no runner evidence exists."
}
```

The receipt above is the honest UNRUN template. It may be committed as a
template but MUST NOT be committed as `status:"ok"` until a real Linux runner
produced it. Fabricating any field is a failed lane.

## 7. POSIX installed APP-012 (Linux)

Parent journey (`tasks/completion/local.json:15`) requires: install into a clean
HOME, bare `oc2` with no subcommand, in-app setup, a coding turn with a brokered
tool plus approval, a second client on the same session, daemon restart and
resume from persisted state. Linux prerequisites still open:

- `tests/e2e/local_application.rs` does not exist and must be authored as a
  compiling RED by a test-author lane, then frozen.
- Approval/resume is not wired: `crates/server/src/lib.rs:1196` converts
  `Decision::RequireHuman` into a terminal path; the parent stays open.
- The install must place `libopentui.so` beside the binary so the loader
  resolves it without `LD_LIBRARY_PATH` (bounded installer stage at candidate
  `2d04c1c`, `EXPECTED_NATIVE=native/lib/$PLATFORM/libopentui.so`).
- Protected-path read (`.env`) denial must assert zero side effect.
- Restart mid-turn must restore truthful state, never a fabricated completion.

Negative/resource assertions carried into the journey: bare launch on piped
stdin takes the documented headless path and never hangs in raw mode; exactly
one store owner exists (a second launch attaches, never forks a database);
whole-tree idle RSS stays within `config/resource-targets.json`.

## 8. Darwin non-proof (explicit)

The authoring host is `Darwin Mac 27.0.0 ... RELEASE_ARM64_T8132 arm64`,
`rustup target list --installed` = `aarch64-apple-darwin` only, `docker` daemon
DOWN, and `/proc/filesystems` absent. Therefore, from this lane and this host:

- kernel Landlock attach is NOT proven; `engage()` still returns `Err(Blocked)`;
- the inherited-fd and child-env denials are NOT proven on a real kernel;
- arm64 Linux native bytes were NOT built or executed;
- the installed Linux APP-012 journey was NOT run;
- no CI/release workflow was triggered; `.github` is CODEOWNER `@rashidtvmr`
  and `gh` is unavailable, so no hosted-runner result may be claimed.

A unit suite that never attaches a ruleset is not isolation evidence
(`docs/SECURITY.md:49-56`). No `status:"ok"` receipt exists for Linux.

## 9. Verification performed in this lane

```
git rev-parse HEAD                         -> 7262682e31c1f473912c9483804c9430eb4ee4c5
git branch --show-current                  -> plan/PHASE1-LINUX-RUNNER-CLOSURE
git merge-base --is-ancestor b924d9a HEAD  -> exit 1 (source plan, not ancestor)
shasum -a 256 lib/x86_64-unknown-linux-gnu/libopentui.so -> e84ced36... (stale vs manifest)
stat -f%z   lib/x86_64-unknown-linux-gnu/libopentui.so -> 26583552
git ls-files crates/opentui-bridge/native/ -> only the x64 .so (no arm64, no manifest)
wc -l scripts/install-oc2.sh               -> 144 (no bounds); candidate 427
grep -c cfg crates/cli/tests/installed_default_entrypoint.rs -> 0 (corrects b924 line 43/211)
grep todo! installed_default_entrypoint.rs -> 5 stub bodies
uname -a                                   -> Darwin arm64; no Linux kernel; docker daemon DOWN
python3 tools/validate_repository.py       -> FAIL backlog exhaustion 51 (inherited)
python3 tools/convergence_gate.py          -> BLOCKED total=59 (inherited)
git diff --check                           -> PASS
```

No Cargo, product test, runner, browser, database, network, credential or
user-data operation was used. No frozen test was touched.

## 10. Remaining unknowns and non-claims

- Real Linux kernel runner availability (x64 and native arm64) is unknown here;
  it is the binding `G-LINUX-RUNNER` gate.
- Whether the `main` x64 blob is a deliberate stale artifact or an unmerged
  refresh is unknown; the integrated lane must re-bind it, not assume.
- The `installed_default_entrypoint.rs` `todo!()` block needs a controller /
  test-author decision; implementers may not edit or silently replace it.
- The bounded installer and the Landlock backend crate are candidates only
  (`2d04c1c`, `4771e3b8`), not `origin/main`.
- No APP-012 acceptance, no Phase 1 completion, no signing, no notarization and
  no hosted-runner success is claimed. `b924d9a` and `7262682` are planning
  sources; this closure makes their Linux lane executable, nothing more.