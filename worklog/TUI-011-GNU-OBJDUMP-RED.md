# TUI-011 GNU objdump architecture RED lane

Status: RED frozen; task NOT completed (implementation absent, by design).

## Claim

- Task: `TUI-011` (RED-only child lane `TUI-011-GNU-OBJDUMP`).
- Session: `ses_f27295547ffedwPMO18IQD5PBo`.
- Base commit: `2d04c1c925595b566f617b073129ecee24593eb9`
  (`origin/lane/CROSS-PLATFORM-RUNNER-CLOSURE-VERIFY`).
- Worktree: `/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/lane-tui011-gnu-objdump-red`,
  branch `red/TUI-011-GNU-OBJDUMP`.
- Owned product file: `crates/opentui-bridge/tests/gnu_objdump_wrapper.sh` (new).
- Owned scratchpad: this file. Own claim row: `tasks/completion/claims.json` TUI-011.
- Transfer evidence: prior claim `ses_f3c4de578ffelQv59xDXmOs03B` was `blocked`;
  user explicitly authorized transfer; orchestrator delegated a RED-only lane.
  Two prior delegated agents failed before startup from provider errors and
  changed nothing. `cc.reclaim` + `cc.claim` executed with that evidence.

## Source evidence (exact)

- `crates/opentui-bridge/native/build_opentui.sh:404-408` (`verify_elf`):
  ```sh
  archline="$(objdump -f "$art" 2>/dev/null | awk -F': ' '/^architecture:/{print $2; exit}')"
  [ "$archline" = "$expect_objarch" ] || die "objdump architecture '$archline' is not '$expect_objarch'"
  ```
  `awk -F': '` prints everything after the first `: `, so the GNU line
  `architecture: i386:x86-64, flags 0x00000150:` yields
  `i386:x86-64, flags 0x00000150:` and fails the literal `x86_64` compare at
  `build_opentui.sh:649` (`x86_64-unknown-linux-gnu` -> `expect_objarch=x86_64`).
- LLVM/Apple objdump emits `architecture: x86_64` (no flags suffix), which is
  why the defect passed on the Darwin/arm64 host and hid on real GNU hosts.
- GNU binutils emits `architecture: i386:x86-64, flags 0x00000150:` for the
  same ELF. The x86_64 Linux artifact was built on a GNU host, so any GNU
  runner re-verifying it through this script fails closed incorrectly.

## Target boundary

- Fix belongs in `build_opentui.sh` (parser must accept the GNU arch token
  before the comma or normalize flags), owned by an implementation lane.
  This RED lane does not touch the script.

## Artifact under test (real, tracked)

- `crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu/libopentui.so`
- SHA-256 `9f074adf1e3c67bb027433d44da05b9e285a37ae58cf0b2ed76504304450c79e`
- size `26627832` bytes.
- `file -b`: `ELF 64-bit LSB shared object, x86-64, version 1 (SYSV), dynamically linked`.
- Apple objdump `-f`: `file format elf64-x86-64`, `architecture: x86_64`.
- GNU wrapper directive line: `architecture: i386:x86-64, flags 0x00000150:`.

## Test contract

`crates/opentui-bridge/tests/gnu_objdump_wrapper.sh`:

1. resolves the real objdump before shadowing it;
2. asserts the tracked x86_64 Linux artifact SHA-256/size (fail closed on drift);
3. installs a PATH-prepended `objdump` wrapper: `-f` emits the forced GNU line,
   every other invocation `exec`s the real objdump;
4. sanity-checks the wrapper emits/delegates correctly;
5. runs `bash build_opentui.sh --verify-only <artifact> --target x86_64-unknown-linux-gnu`
   with the wrapper first in PATH;
6. RED: expects nonzero exit whose stderr contains the exact current-parser
   string `objdump architecture 'i386:x86-64, flags 0x00000150:' is not 'x86_64'`;
7. GREEN: expects exit 0, stderr reports the GNU arch accepted, stdout carries
   the artifact SHA-256.

Deterministic, offline, tempdir under `$TMPDIR` removed on exit, no user DB,
no secrets, no repository writes. Bounded.

## RED evidence (macOS Darwin/arm64 host)

```
$ rtk bash crates/opentui-bridge/tests/gnu_objdump_wrapper.sh
Running builder with GNU objdump wrapper forced first in PATH
--- builder exit: 1
--- builder stderr
build_opentui: file type ok: ELF 64-bit LSB shared object, x86-64, version 1 (SYSV), dynamically linked, BuildID[sha1]=90c98cc7d08f714948df9528bc86fd8916f05f90, with debug_info, not stripped
build_opentui: error: objdump architecture 'i386:x86-64, flags 0x00000150:' is not 'x86_64'
FAIL: RED: builder rejects the GNU objdump architecture line (current parser bug)
TEST_EXIT=1
```

Independent RED on native Ubuntu x86_64 (Lenovo node, Nomad) recorded below.

## Decisions

- One new shell test in `crates/opentui-bridge/tests/` as instructed; not a Rust
  Cargo target, so no Cargo build is introduced (no Cargo expected).
- Test asserts the exact current failure string so the RED is specific, not any
  generic nonzero exit.
- Test also encodes the GREEN contract so the same frozen file flips to PASS
  after implementation; no test edit needed for GREEN.

## Remaining unknowns / blockers

- Implementation lane must normalize the GNU `i386:x86-64` arch token (strip at
  the comma / accept both `x86_64` and `i386:x86-64`) at `build_opentui.sh:404-408`.
- Parent TUI-011 still blocked on MSVC/signing/frozen manifest review.
- This lane does not mark `completed`; implementation is absent by contract.