# DISC-003 decisions

1. DISC-003 preserves every DISC-002 candidate family's exact feature-ID set. Reconciliation may add child work but cannot shrink scope to make coverage easier.
2. A path match is never implementation evidence. `queued` means implementation status remains `unresolved`.
3. `partial` is used when pinned source evidence exists but caller/test/spec coverage or parity is incomplete. Missing evidence is recorded explicitly.
4. `reference-implemented` is reserved for observed 9router behavior. It means the reference implements the behavior, not that OpenCode RK has implemented it or must copy its architecture.
5. Core `BackgroundJob` process-local status is not treated as durable subagent state; the legacy task layer is evidence for UX/behavior only.
6. Review evidence stores paths and immutable blob hashes rather than source copies, keeping audit artifacts bounded.
