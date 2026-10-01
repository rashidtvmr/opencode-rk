# V2 native resize candidate

Package: primary native resize implementation fallback (single-level)
Base: `75ad7b5b681c9bcdb4d37f6f84f3f410e87dabda`

## Implementation

`Renderer::terminal_size` queries `tcgetwinsize` through the `TerminalInput`
owned descriptor. It clones the safe `OwnedFd` under the existing lifecycle
mutex, treats headless/zero/ioctl-error results as unavailable, and retains the
existing closed-handle error. It introduces no raw-fd borrowing, unsafe code,
libc calls, or dependencies. A non-Unix native implementation validates the
handle and returns unavailable geometry, preserving feature compilation and
the caller's configured fallback.

The native CLI checks that geometry at loop entry and on bounded idle wakes,
calls the existing validated `Renderer::resize` only when dimensions change,
and repaints only after a change or another existing state/input event. The
input decoder, authenticated turn worker, streaming transcript, onboarding
dialogs, and terminal cleanup remain unchanged.

## Evidence

The pinned OpenTUI/native bridge already owns the input descriptor and uses
`rustix::termios`; the linked native artifact is the c01292fd fork. Upstream
OpenTUI 0.4.5's renderer dimension event behavior is represented here by
observing kernel geometry directly, avoiding a new signal/FFI ABI surface.
Exact dependency source object: `0c8c4f7cff2927e3df63a9757a45eff9a343611c`.
`packages/core/src/renderer.ts::createCliRenderer` (666-694) prefers TTY
dimensions, `sigwinchHandler` (852-856) reads changed dimensions, and
`handleResize` (3683 onward) applies them. OpenCode
`95daf90670b7c039c436c85537da5fbfe2205b41` selects 0.4.5 in its package catalog.

Frozen controller contract: `tests/e2e/native_resize.py`, SHA-256
`7d27d3bfb5735fe7106604f9474abf8cff0dca3ce81e45de612f3ce13a842449`.
Exact integrated baseline `f8bde4923cb0f6fdf9c86cbb9341d82d50a89209`
failed with `PTY timeout awaiting a visible native frame after TIOCSWINSZ`;
two turns and all cleanup controls passed. Receipt and freeze are retained in
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-today-control-y52o0gi3/`.

## Verification

Source-level checks performed: `git diff --check`; inspected the Rustix 1.1.4
`tcgetwinsize` API and existing bridge ownership path. Heavy build and PTY
verification are intentionally left to the canonical integration owner.

Status: CANDIDATE only; not PREVERIFIED or ACCEPTED.

Independent source review found the first candidate's Unix-only getter was
called from OS-independent native CLI code. Candidate `f64aa5c` is preserved;
the controller's mechanical correction supplies the non-Unix fallback and
aligns headless handling with the getter's documented unavailable result.
Windows runtime and complete G5/G8 acceptance are not established by this slice.
