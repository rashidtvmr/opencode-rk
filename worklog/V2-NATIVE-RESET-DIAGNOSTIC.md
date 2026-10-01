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
