# DISC-003 progress

Status: **IN PROGRESS / NOT ACCEPTED**.

This slice converts every DISC-002 candidate family into an explicit reconciliation record. The current ledger contains 32 families: 16 have pinned partial evidence and 16 remain queued with typed unresolved work. No queued family receives an implementation status by inference from its path.

Implemented in this slice:

- `tools/reconcile_surfaces.py` validates one-to-one candidate coverage and prevents silent feature-scope shrinkage.
- Evidence references must resolve to the locked repository and carry exact Git blob SHAs.
- Reconciled records require source, caller, test, and spec evidence; partial records require pinned source plus explicit unresolved work.
- `sources/disc-003-reconciliation.manifest.json` hash-binds the reconciliation to the DISC-002 rules, evidence catalog, and canonical Ralph plan.
- Upstream status distinguishes V2 implementation, shared legacy, partial/planned behavior, reference-only 9router implementation, and unresolved candidates.

Current evidence-backed findings include Session V2 parity gaps, process-local BackgroundJob semantics versus durable subagents, tool/permission boundaries, scoped plugin lifecycle with deferred reload/external activation, ModelsDev CLI/cache behavior, Integration OAuth/key HTTP surfaces, scoped reference metadata/guidance, and 9router account persistence/routing/token-refresh/translation/dashboard/settings/network-proxy reference semantics. The prompt-context record explicitly separates scoped reference metadata and model-visible guidance from Git cache materialization/I/O, while keeping native mention/template expansion and configured/remote/nested instruction behavior unresolved.

Completion remains blocked on full caller/test/spec traversal, especially dynamic registration, generated APIs, platform gates, UI/client surfaces, legacy compatibility, remaining 9router dashboard/proxy details, and complete translation/provider/format/streaming coverage.
