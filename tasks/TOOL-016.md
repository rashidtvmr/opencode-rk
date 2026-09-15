# TOOL-016

Status: NOT STARTED. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: REQ-042.
Dependencies: none (milestone gating via tools/plan_model.py).
Test obligations: TOOL-016-T01, TOOL-016-T02, TOOL-016-T03, TOOL-016-T04, TOOL-016-T05.
Ownership locks: crates/tools/src/mcp_catalog_search.rs and catalog/mcp-index.json only; worker never edits shared lib.rs, Cargo.toml, schemas, migrations.
Suggested module: crates/tools/src/mcp_catalog_search.rs plus catalog/mcp-index.json (new search module in crate opencode-rk-tools and new versioned index file; lib.rs wiring left to integrator).

## User-observable outcome

MCP catalog search at the top of the MCP list using a versioned, source-attributed popular-library index: searches name, description, tags, and provider with bounded results. Lists candidates only; never executes arbitrary untrusted installs.

## Source evidence

- crates/tools/src/mcp.rs for existing MCP/extension policy surfaces (McpConfig, McpPolicy, McpTool, McpClient).
- crates/tools/src/ext_perms.rs for existing MCP/extension policy surfaces (grant_perm, MAX_EXT_PERMS).
- crates/tools/src/ext_secure.rs for existing MCP/extension policy surfaces (grant_all, MAX_SECURE_PERMS).
- crates/tools/src/tool_allow.rs:8 for bounded tool allowlist precedent (MAX_TOOL_ALLOW).
- crates/server/src/event_bus.rs for bounded event delivery (ServerEvent, BusError Full/Closed).
- docs/SECURITY.md:14-32 brokered permissions, no direct secret access/unrestricted env.

## Observable contract

- `catalog/mcp-index.json`: `{ version: "1", updated: YYYY-MM-DD, entries[]: { name, description, tags[], provider, source } }`; consumers reject missing version; every entry carries non-empty source attribution.
- `CatalogQuery { text, tag: Option<String>, provider: Option<String>, max_results }`: empty text with no filters rejected.
- `search(index, query) -> SearchResult { hits: Vec<CatalogHit>, truncated: bool }`: case-insensitive substring match over name/description/tags/provider; deterministic order (score desc, name asc); capped hits.
- `CatalogHit { name, provider, source, snippet }`: display metadata only; no install command, no executable payload, no env values.
- Bounds: index entries max 512, query text max 256 chars, max_results 1..=50 default 20; file max 256 KiB.
- Deterministic: same index plus query yields byte-identical hits; no I/O inside search (caller loads file), no network, no clock.
- Suggested module boundary: crates/tools/src/mcp_catalog_search.rs owning CatalogEntry, CatalogQuery, CatalogHit, SearchResult, CatalogError, load_index, search; catalog/mcp-index.json owning data; shared lib.rs wiring left to integrator.

## Failure states

- Missing version or bad JSON: `Err(CatalogError::BadIndex)`; no hits returned.
- Empty query (no text, tag, or provider): `Err(CatalogError::EmptyQuery)`; no scan performed.
- Over-cap index or file: `Err(CatalogError::OverCap)`; index rejected whole, never partially applied.
- No match: Ok with empty hits and truncated false.
- Over max_results: truncated true with head results in deterministic order.
- Secret safety: entries hold display metadata only, never credentials or env values; no install execution path exists in this slice. Tests use disposable fixture indexes only.

## Resource bounds

- Pure search: no Command, no thread, no network; index load bounded by 256 KiB file cap; search O(entries) time, O(max_results) memory.
- No unbounded queue, no unbounded retained output; owner and cancel path defined per crates/tools/src/tool_allow.rs:8 bounded-allowlist precedent.

## Acceptance criteria

- TOOL-016-T01: name search happy path: query "postgres" over fixture index returns the postgres entry first with provider and source present and snippet within bounds.
- TOOL-016-T02: tag and provider filters: tag "db" returns only db-tagged entries; provider filter returns only that provider; combined filters AND together in deterministic order.
- TOOL-016-T03: empty and miss: empty query yields EmptyQuery; nonsense token yields empty hits with truncated false.
- TOOL-016-T04: truncation and caps: max_results 2 over 10 matches yields 2 hits plus truncated true; over-512-entry index yields OverCap.
- TOOL-016-T05: isolation and safety: repeated search byte-identical; index entries contain no executable commands or secret patterns; no network or install subprocess touched.

## Test-first execution

- RED: author tests TOOL-016-T01..T05 against crates/tools/src/mcp_catalog_search.rs plus fixture catalog/mcp-index.json; establish compiling RED (fail: no mcp_catalog_search module).
- GREEN: implement minimum native Rust versioned-index search with bounded results; GREEN, refactor, rerun; negative tests (bad-index, empty-query, over-cap, miss).
- Evidence: `cargo test -p opencode-rk-tools mcp_catalog_search`; `cargo check --workspace`; frozen test hash plus command manifest; verifier decides acceptance.
