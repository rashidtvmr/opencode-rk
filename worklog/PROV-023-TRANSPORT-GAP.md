# PROV-023 live transport contract gap

## Candidate and claim

- Product spine inspected at `7e23cc4` after independent loader verification.
- Task: `PROV-023`; integrator session `ses_f3c4de578ffelQv59xDXmOs03B`.
- Scope: discovery proposal only. No product, catalog, or frozen test edit.

## Exact current behavior

- `crates/server/src/lib.rs:920-937` and `:1146-1163` admit only `openai/<model>` for native turns and construct `OpenAiResponsesClient::from_env`.
- `crates/providers/src/responses.rs:516-534` loads `ProviderConfig::from_env("openai")`, validates that mutable environment-derived base URL, and retains it directly.
- `crates/providers/src/responses.rs:537-552` and `:582-599` send production requests to `POST {base_url}/responses` with bearer authentication.
- `crates/providers/src/config.rs:31-39,76-114` defaults the base to `https://api.openai.com/v1` but permits `OPENAI_BASE_URL` to replace it without consulting `request_profile`.
- `docs/provider-compatibility.json:62-82` documents only OpenAI endpoint kind `chat-completions` at `/v1/chat/completions`. It explicitly says Responses is recommended for new projects, but provides no `responses` endpoint entry.
- Repository search finds no production caller of `request_profile::profile_for`; only its implementation and tests call it.

## Why this cannot be wired honestly now

The catalog loader performs exact provider + endpoint-kind/URL lookup and must never guess. Using the `chat-completions` profile to authorize `/responses`, truncating its URL to a base, or retaining unrestricted `OPENAI_BASE_URL` after a nominal unrelated lookup would violate the exact-entry contract and preserve the drift PROV-023 is meant to close.

The task card gives `docs/provider-compatibility.json` an ownership lock and requires reviewed documentation-backed entries. The implementation lane cannot invent or silently add a Responses entry, alter frozen catalog expectations, or claim that a different endpoint covers the production transport.

## Proposed reviewed child contract

Controller/integration authority should create a serialized child (suggested `PROV-023-RESPONSES-CALLER`) with ownership of the catalog entry, request-profile selector, Responses client, and independently authored tests:

1. Add a source-dated OpenAI `responses` entry with exact `https://api.openai.com/v1/responses` URL and documented auth-header names.
2. Add a public selector (typed or exact name) without changing existing chat-completions semantics.
3. Freeze a compiling RED proving `OpenAiResponsesClient::from_env` derives its production endpoint from that exact profile, rejects a missing/malformed/unsupported catalog before reading credentials or sending network traffic, and never appends `/responses` to an arbitrary base.
4. Define whether a custom/local OpenAI-compatible base is a separate explicit capability/profile class. It must not bypass the documentation catalog by environment string alone.
5. Use a local disposable HTTP fixture to prove the real server turn caller reaches only the catalog-authorized endpoint, with no live credentials or external network.
6. Preserve bounded timeout/output limits, redirect denial, redacted errors, and existing stream cancellation.

Until that reviewed entry and RED exist, PROV-023 remains blocked despite the loader seam and all focused tests being GREEN. No acceptance is claimed.
