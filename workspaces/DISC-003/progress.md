# DISC-003 progress

Status: **IN PROGRESS / NOT ACCEPTED**.

This slice converts every DISC-002 candidate family into an explicit reconciliation record. The current ledger contains 32 families: 24 have pinned partial evidence and 8 remain queued with typed unresolved work. No queued family receives an implementation status by inference from its path.

Implemented in this slice:

- `tools/reconcile_surfaces.py` validates one-to-one candidate coverage and prevents silent feature-scope shrinkage.
- Evidence references must resolve to the locked repository and carry exact Git blob SHAs.
- Reconciled records require source, caller, test, and spec evidence; partial records require pinned source plus explicit unresolved work.
- `sources/disc-003-reconciliation.manifest.json` hash-binds the reconciliation to the DISC-002 rules, evidence catalog, and canonical Ralph plan.
- Upstream status distinguishes V2 implementation, shared legacy, partial/planned behavior, reference-only 9router implementation, and unresolved candidates.

Current evidence-backed findings include Session V2 parity gaps, process-local BackgroundJob semantics versus durable subagents, tool/permission boundaries, scoped plugin lifecycle with deferred reload/external activation, ModelsDev CLI/cache behavior, Integration OAuth/key HTTP surfaces, scoped reference metadata/guidance, deterministic HTTP/WebSocket recorder fixtures, typed Effect layer/SQLite runtime adapters, repository parsing/cache materialization, canonical browser-safe event manifests/wire contracts, generated client/server contract identity, current-to-legacy event/config compatibility, typed control-plane route boundaries, location-scoped configuration discovery, replayable scoped State transforms, 60-minute idle location-service caching, global/bootstrap and npm/install metadata behavior, and 9router account persistence/routing/token-refresh/translation/dashboard/settings/network-proxy reference semantics. The newer OpenCode records deliberately separate deterministic replay from live/network/filesystem/environment recorder behavior, accepted BASE/DB runtime semantics from unresolved OPS ownership, pure repository parsing from clone/fetch/filesystem cache effects, canonical Schema identity from runtime consumers, generated contract identity from provider/network integrations, legacy wire translation from GlobalBus/runtime effects, HttpApi error/DTO translation from Git/session/project control-plane mutations, and accepted BASE configuration/lifecycle behavior from unresolved OPS-001/004/005/008 runtime/cache/install/control-plane ownership.

Completion remains blocked on full caller/test/spec traversal, especially dynamic registration, remaining generated/SDK paths, platform gates, UI/desktop client surfaces, broader compatibility/config mappings, exact configuration-runtime task decomposition, container/observability/install ownership, sharing/enterprise remote surfaces, remaining 9router dashboard/proxy details, and complete translation/provider/format/streaming coverage.
