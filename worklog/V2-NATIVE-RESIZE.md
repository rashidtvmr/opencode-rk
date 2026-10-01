# V2 native resize candidate

Package: primary native resize implementation fallback (single-level)
Base: `75ad7b5b681c9bcdb4d37f6f84f3f410e87dabda`

## Implementation

`Renderer::terminal_size` queries `tcgetwinsize` through the `TerminalInput`
owned descriptor. It clones the safe `OwnedFd` under the existing lifecycle
mutex, treats closed/headless/zero/error results as unavailable, and does not
introduce raw-fd borrowing, unsafe code, libc calls, or dependencies.

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

## Verification

Source-level checks performed: `git diff --check`; inspected the Rustix 1.1.4
`tcgetwinsize` API and existing bridge ownership path. Heavy build and PTY
verification are intentionally left to the canonical integration owner.

Status: CANDIDATE only; not PREVERIFIED or ACCEPTED.
