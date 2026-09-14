# PROV-007: Provider authentication handler

## Goal
Implement authentication handler for provider credentials in crates/providers/src/auth.rs.

## Scope
- AuthMethod enum: ApiKey(String), BearerToken(String), OAuth2 { access_token, refresh_token, expires_at }
- ProviderAuth: provider_id String, method AuthMethod
- AuthHandler: auths HashMap, new(), add_auth(), get(), validate(), refresh()
- 5 unit tests: add_and_get_apikey, bearer_token_valid, oauth2_expired, refresh_updates_expiry, token_format_bearer

## Deliverable
- crates/providers/src/auth.rs (implemented)
- Verified via cargo test -p opencode-rk-providers

## Status
DONE
