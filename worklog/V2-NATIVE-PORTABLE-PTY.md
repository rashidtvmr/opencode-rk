# Native installed PTY fixture portability

Mechanical test-harness change only; no product or test assertion semantics
changed.  The fixture now stages the platform's native library suffix, accepts
`OC2_NATIVE_LIBRARY` with the macOS `MAC_OPENTUI_FIXTURE` fallback preserved,
and preserves the existing PTY/session setup without adding a controlling-TTY
ioctl. Unsupported platforms fail explicitly rather than skip.

Base: `6c52b419d4e4029f1baf15e4b25f289872cd0bb8`.

Validation was limited to Python AST syntax parsing, assertion-tree comparison
against base, and `git diff --check`. No PTY/native runtime, Cargo, build, or
Docker validation was performed in this work package; the parent runs both
installed platform profiles.

The initial candidate's pre-spawn `TIOCSCTTY` ioctl was rejected during parent
review: it ran before `Popen(start_new_session=True)`, could return `EPERM`, and
could alter terminal authority when run as a session leader.  That block was
removed in this corrective candidate.  The semantic test method now exactly
matches the base method; its AST dump is compared in validation.  The parent
review finding is a fixture defect, not a product failure or acceptance result.
