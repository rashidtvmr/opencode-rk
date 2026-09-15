# OpenCode V2 upstream research: providers and authentication

Source repository: `/home/rashid/projects/tmpcodes/opencode`
Observed HEAD: `07619a09d6da7945ea3fdbb4f8ae5ce8dc2b6eeb` (2026-09-13).
Important qualification: PLAN.md pins a different upstream revision
(`95daf90670b7c039c436c85537da5fbfe2205b41`), so this research must not silently
claim that observed HEAD equals the pinned release.

## Provider registry and models

- `provider/provider.ts:1042-1371` initializes providers from models.dev, config,
  environment, stored API auth, plugins, custom providers, GitLab discovery,
  provider model hooks, and enabled/disabled/blacklist/whitelist filters.
- `provider/provider.ts:129-153` lists bundled providers. `:175-821` defines
  custom provider behavior. `:1375-1511` resolves SDKs, base URLs, API keys,
  variable interpolation, timeout/chunk timeout, SSE wrapping, OpenAI item IDs,
  bundled/npm/file SDK loading, and fetch caching.
- `provider/provider.ts:1517-1713` provides fuzzy model lookup, default and small
  model selection, model parsing, and ModelNotFound/Init errors.
- `models.ts:111-182` fetches `https://models.dev/api.json` with a 10-second
  timeout, five-minute cache, file lock, hourly refresh, and disable/path/url
  flags. It supports a snapshot fallback.
- `config/config.ts:797-1000` supports provider API/name/env/id/npm/whitelist/
  blacklist/options, model and small model selection, enabled/disabled providers,
  timeout and chunk timeout, agent options, and provider metadata.

## Request transformation and model execution

- `transform.ts:192-368` applies provider-specific caching, message cleanup,
  modality validation, temperature/topP/topK, effort/thinking variants, and
  budget limits.
- `transform.ts:749-1058` maps store/usage/thinking/cache options, gateway and
  Azure options, output token limits, and provider schemas. Default output is
  capped at 32,000 unless the flag overrides it.
- `session/llm.ts:80-426` merges system prompts and variants in base -> model ->
  agent -> variant order, adds provider options, request metadata headers,
  retries, tool-call repair, and provider hooks.
- Request metadata includes OpenCode project/session/request/client headers and
  model headers. The implementation must distinguish legitimate app identity
  headers from client impersonation.

## Authentication store and OAuth

- `auth/index.ts:7-91` stores API, OAuth, and well-known credentials in
  `auth.json` under the global data path with mode 0600. OAuth state contains
  refresh/access/expiry/account information; API state contains key and metadata.
- `auth.ts:96-234` defines provider auth methods, authorization, callbacks, pending
  OAuth state, validation errors, missing code, and callback failures.
- Provider server routes in `server/instance/provider.ts` expose provider list,
  auth methods, OAuth authorize, and OAuth callback operations.
- CLI provider behavior in `cli/cmd/providers.ts:1-510` includes list/login/logout,
  provider selection, well-known auth, API-key storage, and provider hints.

## Codex and other provider integrations

- `plugin/codex.ts:13-608` uses PKCE, CSRF state, browser/headless device flows,
  localhost callback on port 1455, access bearer authentication, and
  `ChatGPT-Account-Id`. It rewrites the Codex API endpoint and applies model
  allowlisting, refresh-on-expiry, and zero-cost metadata.
- `plugin/github-copilot/copilot.ts:12-379` and `models.ts:110-146` implement
  GitHub device flow, refresh, model discovery, provider switching, and required
  provider headers.
- `plugin/cloudflare.ts:1-67` collects account/gateway identifiers when absent.
- Internal plugins include Codex, Copilot, GitLab, Poe, Workers, and Gateway;
  some external plugin bodies were not fully verified and remain unresolved.
- GitLab (`provider/provider.ts:541-677`) supports instance URL, API-token/OAuth,
  AI gateway headers, workflow/agentic chat, and discovered zero-cost models.
- Bedrock (`:276-421`) resolves region/profile/credentials and endpoint prefixes.
- Vertex (`:442-506`) resolves project/location and Google ADC credentials.
- Workers/Gateway (`:678-721`) uses account/gateway metadata and token env vars.

## Retry, overflow, usage, and account behavior

- `error.ts:9-197` detects context overflow, quota, invalid prompt, stream, and
  API-call failures, including HTTP 413.
- `retry.ts:12-122` honors retry-after-ms, retry-after seconds/date, and bounded
  exponential delay. It classifies overload, rate limits, and free-usage limits.
- `overflow.ts:1-22` computes usable context from input and reserved output;
  compaction can be disabled explicitly.
- `processor.ts:361` accumulates input/output/reasoning/cache token counts and
  cost from step-finish data.
- `account/index.ts:134-456` implements device login, polling, refresh, user/org
  discovery, configuration retrieval, and account persistence. It uses bounded
  polling and concurrent first-org selection.

## Security and resource observations

Positive controls include 0600 auth files, PKCE/CSRF state, timeout caps, refresh
expiry checks, bounded retries, model cache TTL, and redacted status surfaces.
Risks requiring deliberate local handling include credentials in process
environment, shallow environment copies, fixed localhost OAuth ports, npm/file
plugin loading, well-known `auth.command` execution, gateway metadata parsing,
and JWT account ID parsing without signature verification. These are evidence to
review, not permission to copy unsafe behavior.

## Local Rust parity gaps to verify

Local `crates/providers/src/auth.rs` has API key, bearer, and in-memory OAuth2
state. `config.rs` has generic provider configuration. Existing retry, rate-limit,
budget, health, metrics, routing, registry, response, and streaming modules are
partial foundations. Likely missing or incomplete: durable protected auth store,
provider-specific OAuth connectors, model catalog refresh, request transforms,
Codex/Copilot account flows, well-known auth, provider-specific headers,
retry-after parsing, context overflow classification, cost accumulation, and
server/CLI auth routes.

## Explicit safety boundary

Upstream provider-specific headers must be classified as documented provider
requirements. This project must not add headers or traffic patterns intended to
impersonate Codex/Claude Code, evade detection, or bypass provider controls.
Local credentials require explicit consent, schema and permission validation, and
redacted storage. Live credentials are excluded from tests.

## Checklist

- [x] Provider registry precedence and filters
- [x] models.dev fetch/cache/fallback
- [x] SDK/base URL/key/timeout resolution
- [x] Model lookup/default/small model
- [x] Request transforms and provider options
- [x] Auth file shape and OAuth callback lifecycle
- [x] Codex OAuth/account/model/header behavior
- [x] Copilot device flow and model discovery
- [x] GitLab, Bedrock, Vertex, Workers/Gateway behavior
- [x] Retry, overflow, usage/cost, account refresh
- [x] Provider routes and CLI commands
- [x] Security risks and local parity gaps
- [ ] Full verification of pinned revision differences
- [ ] Full verification of external Poe/GitLab plugin bodies
