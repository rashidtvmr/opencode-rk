# Native installed PTY fixture portability

Mechanical test-harness change only; no product or test assertion semantics
changed.  The fixture now stages the platform's native library suffix, accepts
`OC2_NATIVE_LIBRARY` with the macOS `MAC_OPENTUI_FIXTURE` fallback preserved,
and uses `termios.TIOCSCTTY` when available (retaining the existing Darwin
request fallback). Unsupported platforms fail explicitly rather than skip.

Base: `6c52b419d4e4029f1baf15e4b25f289872cd0bb8`.

Validation was limited to Python AST syntax parsing, assertion-tree comparison
against base, and `git diff --check`. No PTY/native runtime, Cargo, build, or
Docker validation was performed in this work package; the parent runs both
installed platform profiles.

The semantic test method's assertion AST remained unchanged: 8 assertion nodes,
SHA-256 `f471ce0b3e3a9a6f411010bde3cbd9b5be2e1692b9a5492f99d0f97ea6508b13`.
