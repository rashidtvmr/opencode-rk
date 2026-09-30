# G2 persisted-provider request authentication implementation

Package: G2 persisted-provider request authentication (implementation owner)
Base: `HEAD59f54d0`

Implemented schema-filtered persisted OpenAI API credential resolution with
`OPENCODE_AUTH_CONTENT` and XDG/HOME auth-file lookup, bounded blocking reads,
ambient fallback, and fail-closed missing-credential behavior. Native nonstream
and stream turns now use this resolver. Streaming retains the resolved cloned
client across tool-continuation rounds rather than rebuilding from the ambient
environment. Rooted file authorization resolves relative paths against the
captured project root before broker evaluation.

Upstream evidence: `packages/opencode/src/auth/index.ts` (`file`, `Api`,
`all`, `get`) and `packages/llm/src/provider.ts` persisted auth override path.

Source-ready only; no heavy build/test was run per the package validation
budget. `git diff --check` passed. Parent must run the frozen
`prov_025_request_auth` gate on the exact committed candidate.
