# Provider Credential Filtering Semantics Research

Updated: 2026-10-26; role: contract researcher; session: ses_e88d69c49ffelbqhgSyI6SudfU
Base/candidate SHA: 7b94da36 (opencode-rk-recovery-20261005)
upstream SHA: 450e7b92412c780a7b5a75226bb8d49aaca24e22

## Goal and success criteria
Determine exact credential filtering semantics for custom provider discovery vs available model picker in upstream OpenCode V2. Identify real mismatch or refute prior audit claim that conflated raw discovery and available catalog.

## Evidence read
- `packages/core/src/provider.ts:448-510` — `snapshot()` function building `available` filter
- `packages/core/src/provider.ts:320-326` — `Interface.available()` returns `snapshot().providers`
- `packages/core/src/model.ts:92-234` — `read()` builds `data.models` from `input?.available`; `available()` filters `all` by `model.enabled` only
- `packages/core/src/model.ts:232` — `available: preferFree(all.filter((model) => model.enabled))`
- `packages/core/src/config/plugin/provider.ts:175-202` — `overlayProvider` sets `provider.activation = "enabled"` for configured providers
- `packages/schema/src/provider.ts:34,85,94` — `Activation = Schema.Literals(["auto", "enabled", "disabled"])`; default is `"auto"`
- `packages/core/src/plugin/host.ts:189-228` — plugin `provider.list` and `model.list` both call `.available()`
- `packages/core/src/model-resolver.ts:380-399` — runtime resolution checks `activation === "enabled"` + credential absence for auth route selection

## Findings (confirmed)

### Provider availability filter (`provider.ts:457-470`)
The `snapshot()` inner filter for `available` providers:
```
if (record.provider.activation === "disabled") return false
// ... sourceConnection mismatch check ...
if (record.provider.activation === "enabled") return true
if (integration?.connections.length) return true
return record.provider.integrationID === undefined && !integration
```

Three branches:
1. `activation === "disabled"` → excluded (always)
2. `activation === "enabled"` → **included unconditionally** — NO credential/connection check
3. `activation === "auto"` (default) → included if connection exists OR if no integrationID and no integration record

### Model availability (`model.ts:100, 232`)
- `data.models` initialized from `input?.available` (provider snapshot's available definitions only)
- `model.available()` filters `all` by `model.enabled` field ONLY
- No re-check of provider credential availability at model level

### Configured providers (`config/plugin/provider.ts:192`)
All user-configured providers get `activation = "enabled"`, meaning they ALWAYS appear in `available()` regardless of whether credentials are configured.

## Real mismatch vs prior audit

CONFIRMED: Prior audit was correct that there is a conflation, but incomplete on WHERE.

The mismatch: **`provider.available()` (provider picker) and `model.available()` (model picker) both include providers/models based on `activation` flag alone.** Providers with `activation === "enabled"` (which includes ALL user-configured custom providers) appear in both pick lists WITHOUT any credential validation.

The credential check (`integrations.list()` at `provider.ts:453`) only gates the `"auto"` activation branch. Explicitly-enabled providers bypass it entirely.

This is NOT a bug per se — it is intentional design: configured providers with explicit `activation: "enabled"` are surfaced to the user so they can configure credentials. The actual credential enforcement happens later in `model-resolver.ts:393-399` when a model is selected for use (route = `Auth.none` when no credential, but model still resolves).

## Decisions and rationale
- Discovery (provider list) and catalog (model list) both use the same `activation`-based filter
- Credential filtering is NOT applied at discovery/catalog time for enabled providers
- Credential filtering IS applied at model-resolution time (runtime usage), not at picker time
- No `hidingProviders` mechanism exists in upstream; all enabled providers show regardless of credentials

## Alternatives rejected
- Credential gating at picker level: Would hide enabled-but-unconfigured providers, contradicting user intent to see them and add credentials

## Verification executed
Source inspection only (read-only, no build/test/network/DB/Git per constraints)

## Artifacts and hashes
Scratchpad: /Users/mymac/Projects/opencode-rk-recovery-20261005/T/opencode/research_provider_picker/scratchpad.md

## Remaining uncertainties
- Whether this is considered a "bug" or "feature" by product definition — appears intentional by design
- The model-resolver auth route selection (`Auth.none`) for enabled providers without credentials means models can still be "selected" but will fail at execution time for API-key-based providers
