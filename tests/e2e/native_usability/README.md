# Native PTY regressions

Stdlib Ruby 2.6+. Run from the workspace root against the built native binary. The scripts accept the binary path as `ARGV[0]`; all disposable configs and transcripts go under `target/phase1-convergence-20261021/native-usability/`, never this test directory.

Selection (`selection_pty_journey.rb`) and child-control
(`subagent_pty_journey.rb`) regressions are undergoing integrated-binary
verification. Their presence and syntax checks do not establish product
acceptance. Run them with the tested binary as the first argument and an explicit
`DYLD_LIBRARY_PATH` pointing to that artifact's native library. Set
`EXPECTED_BINARY_SHA256` to the independently recorded build digest.

The selection journey captures OSC 52 without forwarding it to the host
clipboard. It distinguishes enabled release-copy from disabled left-release
retention and right-press copy. `EXPECT_COPY_RED=1` is a pre-fix reproduction
mode; successful expected-RED reproduction cannot count as product acceptance.
Final verification uses the default acceptance mode.

The child journey must preserve the complete custom follow-up prompt, correlate
real child provider requests, and verify typed state and generation. It uses the
permission API for task approval. Mouse-only permission coverage belongs to the
six-decision journey below.

```sh
BIN="$PWD/target/debug/oc2"
DYLD_LIBRARY_PATH="$PWD/target/ui-native/lib" ruby tests/e2e/native_usability/import_pty_journey.rb "$BIN"
DYLD_LIBRARY_PATH="$PWD/target/ui-native/lib" ruby tests/e2e/native_usability/model_import_pty_probe.rb "$BIN"
```

Set `DYLD_LIBRARY_PATH` to the native-library directory for the tested artifact. The model probe passes an explicit value to the child process, defaulting to `target/ui-native/lib`; the import journey uses it when valid, otherwise derives it beside the binary. For installed verification, pass the installed binary and its installed library directory. Use the exact same built binary for both runs.

On this macOS host, `/usr/bin/ruby` strips `DYLD_LIBRARY_PATH` before the
script runs. Use `/opt/homebrew/bin/ruby` when binding an explicit native
library directory, and verify the library path and digest in the resulting
summary. A fallback to `target/ui-native/lib` does not verify an installed
library pair.

Record and verify the binary SHA-256 against the build evidence before running:

```sh
shasum -a 256 "$BIN"
```

Both summaries print `binary_sha256`; the model probe also enforces an independently supplied expected hash when `EXPECTED_BINARY_SHA256` is set, failing with `binary SHA mismatch expected=... actual=...` on mismatch. Example with a known expected hash:

```sh
EXPECTED_BINARY_SHA256='<trusted-build-sha256>' DYLD_LIBRARY_PATH="$PWD/target/ui-native/lib" ruby tests/e2e/native_usability/model_import_pty_probe.rb "$BIN"
```

The permission journey is the mouse-only six-decision matrix plus fresh-daemon
restart checks for Always allow and Always deny. It accepts the original
`<binary> <output_dir> <label>` arguments and writes only disposable evidence
under the output directory. The normal 100x32 run should be given up to 300
seconds because it starts six isolated native daemons:

```sh
DYLD_LIBRARY_PATH="$PWD/target/ui-native/lib" ruby tests/e2e/native_usability/permission_pty_journey.rb "$BIN" "$PWD/target/phase1-convergence-20261021/native-usability/permission-pty-verification" normal
```

Compact geometry can be exercised explicitly; the PTY size, child environment,
window ioctl, and ANSI cell parser all use the same values. A compact terminal
must still render the complete decision dialog and footer; if it clips required
controls the journey fails closed rather than clicking an unpainted location:

```sh
PTY_COLS=64 PTY_ROWS=20 PHASE1_ONLY=1 \
  DYLD_LIBRARY_PATH="$PWD/target/ui-native/lib" \
  ruby tests/e2e/native_usability/permission_pty_journey.rb "$BIN" "$PWD/target/phase1-convergence-20261021/native-usability/permission-pty-compact" compact
```

The permission summary records `terminal_cols` and `terminal_rows`, the binary
SHA-256, native-library SHA-256, all six ordinary scope results, and separate
`durable_restart_all_passed` evidence. Restart verification is not inferred
from rule-file presence.

`import_pty_journey.rb` preserves the startup offer, persisted decline, restart silence, settings reopen, inert pointer events, preview/cancel/apply, conflict handling, source immutability, command activation and restart checks. Its 18 acceptance assertions are the 16 named `required_checks`, unchanged source hash, and no captured error. `model_import_pty_probe.rb` exercises provider catalogue GET, selected imported model display, actual `/responses` request with the exact fixture model ID, streamed reply, and unchanged imported config. It requires no manual provider/model choice or injected recent selection, and asserts `model.json` absent before and after.

Each run creates a timestamped disposable fixture and evidence directory under the corresponding `target/.../import-pty-verification/` or `target/.../model-import-pty-verification/` folder. The fixtures include isolated HOME/XDG/config/data/project directories; the model probe additionally starts a loopback fake provider. Review each printed summary and `summary.json`; exit status is nonzero if a required assertion fails. No generated fixture or previous PASS summary is committed.
