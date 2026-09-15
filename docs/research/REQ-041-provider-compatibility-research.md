# REQ-041 Research: Official Provider Compatibility and CLI Authentication

Research date: 2026-09-15.

## Scope

This requirement covers legitimate compatibility with provider APIs and user-consented
authentication used by OpenAI Codex and Anthropic Claude Code. It does **not** cover
impersonating another client, spoofing client headers, bypassing provider controls, or
replicating undocumented endpoints.

## Current repository evidence

- `crates/providers/src/auth.rs:6-18` has API-key, bearer-token, and in-memory OAuth2
  representations.
- `crates/providers/src/auth.rs:40-101` provides an in-memory `AuthHandler` and basic
  refresh operation, but no durable protected storage.
- `crates/providers/src/oauth_flow.rs:1-65` validates HTTPS OAuth step metadata only;
  it does not execute an OAuth flow.
- `crates/providers/src/integration.rs:1-5` explicitly stops before credential storage
  and external OAuth execution.
- `crates/providers/src/config.rs:7-22` provides generic provider configuration.
- `docs/SECURITY.md:24-32` forbids direct secret access, logging secrets, and unrestricted
  inherited environment values.
- `docs/SECURITY.md:62-72` requires real human authority for OAuth and says credentials
  cannot be fabricated by implementation workers.

## Official facts to preserve

### OpenAI Codex

OpenAI's Codex authentication documentation describes ChatGPT sign-in and API-key
authentication. Codex CLI caches credentials in `~/.codex/auth.json` by default, or in
an OS credential store depending on its configuration. The file contains access tokens
and must be treated like a password. The implementation should import it only after
explicit user confirmation, validate ownership and file mode, parse a versioned subset,
and copy tokens into protected storage without logging or returning token material.

### Claude Code

Anthropic's Claude Code authentication documentation describes OAuth and API-key
authentication. On Linux, credentials are stored in `~/.claude/.credentials.json` with
mode `0600`; `CLAUDE_CONFIG_DIR` can relocate the configuration directory. macOS may
use the Keychain. This project must not silently read the Keychain or another tool's
credential store. Import is explicit and file-based unless a future native keyring
connector is separately authorized.

## Provider request policy

Use provider-native, documented API authentication and required headers. Preserve the
actual client identity as this application. Do not add headers or request patterns whose
purpose is to impersonate Codex, Claude Code, or another agent, evade bot detection,
hide the calling application, or access undocumented subscription endpoints.

Wire diagnostics are structured and redacted: provider ID, model, method, URL origin,
status, latency, request/response sizes, and approved non-secret headers only. Tokens,
authorization values, prompts containing secrets, and raw credential payloads are never
stored in logs or transcripts.

## Acceptance direction

The implementation must support official OAuth lifecycle hooks, refresh and logout,
explicit local-file import, provider/model selection, auth status, usage/limits status,
and offline differential fixtures. Live OAuth consent and production credentials belong
to the human verification phase, not automated implementation tests.
