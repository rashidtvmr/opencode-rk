# G2 persisted-provider request authentication implementation

Package: G2 persisted-provider request authentication (implementation owner)
Base: `HEAD59f54d0`

Implemented schema-filtered persisted OpenAI API credential resolution with
`OPENCODE_AUTH_CONTENT` and XDG/HOME auth-file lookup. File reads are blocking
and bounded to at most `MAX_AUTH_BYTES + 1` bytes using `Read::take`; regular
files are required before reading,
ambient fallback, and fail-closed missing-credential behavior. Native nonstream
and stream turns now use this resolver. Streaming retains the resolved cloned
client across tool-continuation rounds rather than rebuilding from the ambient
environment. Rooted file authorization resolves relative paths against the
captured project root before broker evaluation.

Upstream evidence: pinned `packages/opencode/src/auth/index.ts` (`Api` lines
23–27; `all` lines 58–67) defines the API record schema and inline JSON
override/fallback behavior. Persisted API credentials override environment
credentials in `packages/opencode/src/provider/provider.ts` lines 1582–1610.

This correction preserves upstream source selection: non-empty valid inline JSON
replaces the file source; empty or invalid inline content falls through to the
file; an empty valid inline object does not mask ambient credentials. Optional
API metadata is accepted only as a string-to-string object.

Source-ready only; no heavy build/test was run per the package validation
budget. `git diff --check` passed. Parent must run the frozen
`prov_025_request_auth` and `prov_030_auth_sources` gates on the exact committed
candidate.
