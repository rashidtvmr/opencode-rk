# Native reset diagnostic candidate

Process-only diagnostic branch from failed prepared source
`681ba084f1d1deb9ad7ec8fb6aa84598a7a85c40`; it is not a product candidate.

When `OC2_NATIVE_RESET_TRACE` names a path under the approved controller
temporary root, the bridge records only fixed cleanup-stage events and
low-level reset-operation errors. The trace is capped at 16 KiB, uses a
regular-file check, does not include user text, tokens, or terminal output,
and logging failures are ignored. No frame/input path is traced.

The intended verifier run sets the trace path and runs the unchanged frozen
fixture. It should reveal whether setup, explicit restore, release, native
destroy, ttyname/openat/fstat/tcgetattr, polling, or writing is reached. The
instrumentation is diagnostic-only and must not be integrated as product code.

Static checks performed by this leaf: `rustfmt --edition 2021 --check` on the
instrumented bridge source and `git diff --check`. No build, runtime, PTY,
network, or Cargo command was run.

## Mechanical compiler repair

The original diagnostic candidate failed Rust E0308 because a `let ... else`
branch could fall through after creating the trace file. The diagnostic owner
replaced that branch with explicit `symlink_metadata` matching, a private 0600
creation path, and a second metadata read before append. Existing regular files
are appended; symlinks, non-regular files, path collisions, and I/O errors are
ignored without panic. The approved controller-root parent is enforced exactly,
and the existing 16 KiB/event bounds and fixed-stage-only event callers remain
unchanged. This is process-only diagnostic maintenance and does not alter native
TTY reset behavior.
