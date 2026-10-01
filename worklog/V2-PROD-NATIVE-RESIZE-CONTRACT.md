# V2 production native resize contract (NATIVE-RESIZE)

Controller pre-freeze source review corrected readiness to use the selected
fixture model `gpt-5.6`: the imported `has_model_status` helper is deliberately
fixed to the onboarding fixture's `gpt-5.6-mini`. Completed-turn waits now require
the visible `assistant: <reply>` row, preventing a streaming delta or pending
old-geometry frame from being used as idle-resize readiness. Candidate `65074c9`
is preserved; no runtime RED or acceptance has been established yet.

The fixture also explicitly acquires the controlling PTY before executing the
CLI and checks that its foreground process group equals the owned child PID.
Pinned OpenTUI 0.4.5 (`0c8c4f7cff2927e3df63a9757a45eff9a343611c`, selected by
OpenCode `95daf906`'s package catalog) registers a SIGWINCH handler in
`packages/core/src/renderer.ts`. A slave descriptor with `isatty` alone does not
establish that the kernel's resize signal reaches the CLI. A fresh Python child
performs `TIOCSCTTY` then `execve` with the existing isolated environment and
fixed argv; this preserves the PID/group and avoids threaded `preexec_fn`.

## Candidate scope

This is a source-only test-owner repair of candidate `79fc3e6` (which remains
preserved as a normal commit). It owns only the live native PTY resize
observation; it does not implement product resize handling.

The test reuses the immutable two-turn fixture contract from
`tests/e2e/native_escape_input.py`: exact `FIRST`/`SECOND` user turns, two
ordinary provider requests, and durable four-message history. It starts an
attested release artifact in isolated HOME/XDG/data/runtime/project locations,
sets the initial kernel PTY geometry to 80x24, waits for both visible assistant
responses, then performs one idle `TIOCSWINSZ` transition to 60x8. It sends no
input for the resize and requires no third provider request.

The post-ioctl bytes are fed to a fresh 60x8 `TerminalScreen`. The oracle
requires a visible new native frame containing the OpenCode header and composer
prefix, explicit CUP/HVP rows no greater than eight, and no stale provider
activity. It does not require `ESC[2J`; differential repaint implementations
are valid. The full 100x24 grow path is intentionally not claimed by this
slice and remains a future product/test extension.

## Authority and source evidence

- Pinned OpenCode authority is commit
  `95daf90670b7c039c436c85537da5fbfe2205b41` in
  `sources/upstream.lock.json` (lines 7-14). The upstream checkout is not
  vendored here; this file does not claim uninspected upstream internals.
- Current `crates/cli/src/tui_entry.rs::native_size` (lines 530-537) reads
  `COLUMNS`/`LINES` with an 80x24 fallback, and `native_loop` (lines 560-711)
  paints from the renderer's current dimensions. There is no current observed
  CLI terminal-size event query in this source path.
- Current `crates/opentui-bridge/src/safe_renderer.rs::NativeRenderer::resize`
  (lines 603-616) makes native resizing available safely, but availability is
  not proof that the CLI receives kernel geometry changes.
- Current host/app abstractions model `HostEvent::Resize` and frame invalidation
  (`crates/cli/src/native_host.rs:229-240,333-340` and
  `crates/cli/src/native_app.rs:579-585`). These are source contracts, not
  installed real-PTY evidence.
- The pinned upstream TUI behavior to inspect before product implementation is
  the OpenTUI `TerminalDimensions`/`createCliRenderer` sizing and reactivity
  path. A historical local product attempt (`b53a492198c0d453650ed1192cb4fdb2838462ba`)
  contains native terminal-size/geometry polling and renderer resize callers;
  it is explicitly historical evidence, not current authority.
- Existing `native_escape_input.py` supplies the unchanged fixture `State`,
  `Handler`, `Fixture`, bounded `send_fragments`, authenticated descriptor and
  exact two-turn `history` checks. Its protected helper hash is not modified by
  this candidate.

## Cleanup and resource contract

The test bounds requests at 128 KiB, responses at 256 KiB and PTY capture at
256 KiB; provider reads use the existing bounded five-second fixture socket
behavior. PTY writes use the existing deadline/nonblocking `send_fragments`
helper. Artifact paths are absolute and require `native: true` and
`profile: release`; descriptor validation requires schema 1, loopback origin,
validated owned PID/group and a 64-hex token. The socket path is capped at 100
bytes. The test records raw-mode observation and checks terminal attributes
before closing the PTY, performs normal Ctrl-C CLI reaping, stops only the
validated daemon, checks its PID is gone, joins the non-daemon provider thread,
and rejects forced cleanup, secret echo, output overflow, or hidden cleanup
failures. The fixture root is intentionally preserved for parent forensic audit.

Failure evidence records the actual error, fixture root, provider count,
geometry/oracle observations and every cleanup flag. `phase: success` is
written only after all semantic and cleanup checks pass.

## Status and limitation

AST parsing and `git diff --check` are the only validation appropriate in this
source-only lane. No Cargo build, HTTP, provider, PTY, runtime, or heavy command
has been run. Status is **candidate / not frozen RED / not PREVERIFIED / not
ACCEPTED**. The parent must execute the bounded release-artifact gate and record
the exact integrated SHA. Future product implementation, if required after
acceptance, should be serialized after native input acceptance and investigate
the existing safe `rustix` terminal-size/descriptor boundary rather than
substituting environment variables.
