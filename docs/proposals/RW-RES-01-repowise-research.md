# RW-RES-01: Repowise-style codebase intelligence research

Status: research-only. No product code. Sources: repowise docs (docs.repowise.dev, github.com/repowise-dev/repowise docs/agent/MCP_TOOLS.md), PLAN.md ADR-001/ADR-004, docs/SECURITY.md s1-3, tasks/TOOL-009.md.

## 1. Capability inventory

| Capability | Repowise surface | Local compute? | Notes |
|---|---|---|---|
| Architecture overview | `get_overview`, `get_architecture` (opt-in, workspace) | Yes (graph+git) | Entry points, layers, module map; `_meta` freshness envelope |
| Wiki / index | auto wiki + deterministic (`--no-prose`) wiki, `repowise init/update/reindex` | Yes | Incremental update; stale warning on HEAD drift |
| Symbol search | `get_symbol`, `search_codebase` mode=symbol/path | Yes | tree-sitter AST; `path::Symbol` ids, range reads |
| Semantic search | `search_codebase` mode=concept/hybrid, `get_answer` (RRF full-text+vector+symbol) | Needs embedder | LanceDB vectors; BM25 fallback keyless |
| Why / decisions | `get_why` (search/path/dashboard), decisions layer | Partial | Keyword local; extraction/LLM synthesis needs model |
| Risk | `get_risk` (hotspot/dependents/co-change), `get_change_risk` (diff score), `get_health` | Yes | Deterministic Python; co-change from git |
| Dependency path | `get_dependency_path` (opt-in), `get_execution_flows`, `get_blast_radius`/`get_conformance` (workspace) | Yes | Import-graph walk; miss returns ancestors/bridges |
| Dead code | `get_dead_code` (tiers, safe_to_delete>=0.70) | Yes | Pure graph+SQL, <10s claim; framework-aware allowlist |
| Context card | `get_context` (docs/symbols/ownership/callers/callees/skeleton) | Yes | Task-shaped; TOOL-009 precedent for JIT listing |
| Ops surface | `lean` profile, omission store/`distill`, hooks, dashboard, PR bot, Teams/Jira | Mixed | Lean = 6 tools ~2.1k schema tokens |

Canonical default: 10 tools (`get_overview/answer/context/symbol`, `search_codebase`, `get_risk/change_risk/why/dead_code/health`) + `list_repos` in workspace mode. Opt-in: `get_dependency_path`, `get_execution_flows`, `generate_refactoring_code`, `get_architecture`, `get_blast_radius`, `get_conformance`, `set_finding_status`.

## 2. Fits local-first Rust harness (ADR-001/ADR-004)

- `get_overview`, `get_context`, `get_symbol`, symbol/path search: pure AST + SQLite index. ADR-001 single Tokio runtime, blocking parse on CPU pool; ADR-004 SQLite catalog + content-addressed blobs for wiki pages.
- `get_risk`/`get_change_risk`/`get_health`/`get_dead_code`: deterministic graph+git math, no LLM. Fits bounded quotas: cap findings (repowise clamps limit 25), token-budgeted responses with omission refs.
- Deterministic (`--no-prose`) wiki + incremental `update`: matches ADR-004 retention/quota per category; freshness envelope (`indexed_commit`, `index_age_days`) prevents confidently-wrong answers.
- `get_dependency_path` (opt-in only): cheap BFS over stored edges; keep off by default like upstream.
- `get_why` path/dashboard modes over locally recorded decision records + git archaeology fallback: keep; drop LLM synthesis path.
- `lean` profile + `distill`/omission store: adopt as resource control (PLAN s9 byte budgets).

## 3. Does NOT fit (network/JS/cloud)

- Hosted dashboard/Next.js, PR bot, Teams/portfolio, Jira/Confluence, breaking-change guard SaaS, SSO/SCIM, CVE/SBOM/compliance suite: cloud multi-user scope, excluded per PLAN s7 (no claim over upstream cloud infra).
- `generate_refactoring_code`, chat/`get_answer` LLM synthesis, model-written wiki prose, decision extraction via provider: require external LLM key + network; banned in native mode without explicit opt-in (ADR-005, SECURITY s3: no hidden JS runtime, no silent network).
- Workspace multi-repo federation (`repo="all"`, cross-repo blast radius), webhook receivers, Postgres/pgvector on-prem topology: remote multi-user opt-in profile only (ADR-002); default is single daemon per user/data-dir.
- Telemetry (`command_run`/`mcp_tool_call`): default-off here; upstream 90-day retention unacceptable as default.
- AGPL-3.0 engine: cannot embed; reimplement natively, keep behavior parity not file translation (ADR-003).

## 4. Privacy / resource notes (docs/SECURITY.md s1-3)

- Privacy: raw source transient only, never persisted; store graph+stats+generated docs in workspace-local SQLite/LanceDB-equivalent with same ACLs as repo. Embeddings non-reversible but derived: protect index dir like repo. BYOK/offline split: zero-LLM layers default; any model call needs explicit human grant with resource/identity/expiry bounds (ADR-006). No secret logging; secret scan stores fingerprint only. Denied access asserts absence of side effects.
- Isolation: index parser runs under OS backend (Landlock-class), closes inherited caps, tested on platform; prompt/regex is not a sandbox (SECURITY s3-4).
- Resources: one runtime, bounded CPU pool for parse/embed; byte-bounded queues/responses; per-category retention/GC; measure whole tree incl. sidecars (PLAN s9). Stale-index signal mandatory.

## 5. Recommended minimal native subset

1. `overview` (graph+git summary, freshness envelope).
2. `context` (file/module/symbol card + skeleton + callers/callees, batched).
3. `symbol` (bounded range read by id).
4. `search` (symbol/path full-text + co-change; semantic only with local embedder, else BM25).
5. `risk` (hotspot/dependents/co-change/test-gap + diff scorer merged, deterministic).
6. `why-local` (recorded decisions + git archaeology; no LLM synthesis).
7. `dead-code` (tiered, safe_to_delete>=0.70, framework allowlist).
8. Defer: dependency-path (opt-in flag), hosted/cloud, LLM prose/chat/codegen, telemetry (off).

Precedent: TOOL-009 ToolSearch JIT listing maps to `context`/`search` with schema kept resident (lean profile lesson).
