# PROV-023-LOADER-VERIFY

## Claim
- Task: PROV-023 (verification-only lane)
- Session: ses_f31530101ffe5jsZ85fgNu9QEf
- Branch: lane/PROV-023-loader-verify-20260923 @ a198704
- Scratchpad: worklog/PROV-023-LOADER-VERIFY.md (owned file; no product/catalog/test edits)
- Impl under review: abf206a (crates/providers/src/request_profile.rs)
- Frozen loader RED: crates/providers/tests/prov_023_loader_red.rs SHA-256 d6aa3a01ad4e6a4d09c148672d29dbc608c739e5bbf7cb574f8b9ad16becc2d8
- Static catalog: docs/provider-compatibility.json; PROV-019 seam: request_profile::profile_for

## Source evidence
- request_profile.rs:28-37 env name, 256 KiB cap, 32/16/32/32/8 caps, {model=models/*} allowlist, include_str embedded catalog.
- :349-382 documented_entry: exact provider id match, kind-or-exact-URL match, typed UnknownProvider/UndocumentedEndpoint, BadEndpoint only for URL-shaped unknown selectors.
- :384-418 load_catalog + read_bounded_catalog: unset/empty env to embedded; set to bounded open (metadata pre-check + take(MAX+1) post-check), no fallback; raw len + secret-substring pre-scan before serde_json::from_slice.
- :420-478 validate_catalog: version==1, YYYY-MM-DD shape + month/day range, providers 1..=32, endpoints 1..=16, aliases non-empty 1..=32, limitations <=32, auth headers 1..=8, lowercase id spellings, bounded text, duplicate provider/endpoint-kind/alias/auth-name reject, deny_unknown_fields on all structs.
- :480-509 map_auth_headers: ASCII alnum+dash names, dup names reject, allowlist Authorization->Bearer, x-api-key/x-goog-api-key->ApiKeyHeader, anthropic-version->None (non-credential, dropped), else HeaderNotAllowed, empty kinds reject.
- :511-519 contains_secret_like_bytes: sk-, sk-ant- (subsumed), Bearer raw substring scan.
- :564-606 is_safe_https_url: non-empty, <=2048 B, no whitespace/control, single allowed placeholder max (others {/} reject), reqwest::Url::parse, https + host + no userinfo + no fragment + credential-like query keys (key,api_key,apikey,token,secret,password,authorization) deny.
- :104-120,234-254,258-285 errors carry no payload; headers_for/with_diagnostics redacted-only.
- lib.rs:50 module wired; repo grep: no product caller of profile_for outside request_profile.rs + 3 test files.
- docs/provider-compatibility.json:1-90 v1, openai/anthropic/google, https-only, no sk-/Bearer/sk-ant- bytes (grep 0), 2929 B.
- docs/SECURITY.md:24-32 no direct secret file access / unrestricted inherited env; config.rs:84-150 env-driven precedent (from_env, get_api_key).

## Verification runs (serial, jobs/threads 1, bounded Python timeouts)
- cargo test -p opencode-rk-providers --test prov_023_loader_red -- --test-threads=1 => 21 passed 0 failed RC0.
- cargo test -p opencode-rk-providers --test prov_019_request_profile -- --test-threads=1 => 5 passed 0 failed RC0.
- cargo test -p opencode-rk-providers --test prov_023_runtime_catalog -- --test-threads=1 => 4 passed 0 failed RC0.
- cargo test -p opencode-rk-providers --test prov_023_provider_catalog -- --test-threads=1 => 5 passed 0 failed RC0.
- cargo test -p opencode-rk-providers --lib -- --test-threads=1 => 73 passed 0 failed RC0.
- shasum -a 256 crates/providers/tests/prov_023_loader_red.rs => d6aa3a01...becc2d8, matches frozen SHA. Zero test edits.
- git diff --check clean. Only tasks/completion/claims.json (ledger) + owned worklog dirty.

## Findings
- Bounded read before parse: PASS. Metadata size pre-check + take(MAX+1) post-check (:401-418); embedded path uses fixed include_str bytes with raw-len check (:384-394).
- Strict schema/caps/duplicates: PASS. deny_unknown_fields (:313-347), version/date/shape/caps/dups (:420-478), duplicate auth names (:480-492).
- Optional override with embedded fallback only when unset/empty: PASS. var_os filter empty (:385-391); set-but-missing/malformed never falls back (:388-389 + fail-closed tests T missing/malformed/version/shape).
- Fail-closed missing/malformed override: PASS. open/read/metadata/len/secret/parse/validate all map to BadEndpoint (:401-418,:392-398).
- Exact lookup/no guessed endpoint: PASS. find id==provider_id (:354-358), kind-or-exact-URL match (:365-368), typed Unknown/Undocumented (:369-377). No synthesis.
- Safe URL/userinfo/query/placeholder: PASS. https+host, no userinfo/fragment (:592-599), credential query deny (:600-605), one allowed placeholder max else {/} reject (:573-588). Google {model=models/*} resolves (loader T valid_override + runtime T01).
- Auth-header mapping: PASS. Closed allowlist, anthropic-version dropped not credentialed, dup/empty/unknown => HeaderNotAllowed (:480-509). PROV-019 openai/anthropic happy paths preserved (5/5).
- Secret-free errors/debug: PASS. RequestError carries no payload (:104-120); loader debug/unknown/unknown-endpoint tests GREEN; catalog raw has 0 sk-/sk-ant-/Bearer hits; catalog doc limitation sentence "Authorization credentials are secrets..." contains word "Authorization" not "Bearer " so no scan hit; limitation "task-"/"risk-" style words absent.
- No network: PASS. Only File open + Url::parse; no reqwest client/fetch; tests offline.
- Legacy profile compatibility: PASS. PROV-019 5/5 + lib 73/73 green; RequestProfile::validate re-resolves against live catalog (:234-254) so embedded-catalog profiles validate OK.
- Overreject check (sk-/Bearer substrings vs checked-in catalog): NO LEAK, NO OVERREJECT on current catalog. Raw byte counts sk-=0, sk-ant-=0, Bearer+space=0. Note residual overreject surface (classify only): any future doc sentence containing e.g. "mask-" or "Bearer token" prose would fail closed; current catalog avoids it. Secret scan is fail-closed by design per card Failure states.
- RequestProfile::validate determinism: DETERMINISTIC given same catalog input. Pure function of (self, catalog bytes): same endpoint/auth/options => same Ok/Err. Caveat: result depends on env override at call time (by design); two calls with different OPENCODE_RK_PROVIDER_CATALOG_PATH can differ. Not nondeterminism (no clock/rng/retained state), but caller-visible input dependence. Recorded, not a defect.
- Env path vs security policy (classify, no patch): OBSERVATION, not defect. Loader reads one explicit allowlisted path var OPENCODE_RK_PROVIDER_CATALOG_PATH as catalog *location* (docs/SECURITY.md bars direct *secret file* access + unrestricted inherited env, plus config.rs:84-150 from_env precedent for non-secret config). Loader never reads secret values, rejects secret-like catalog bytes pre-parse, maps only header kinds. Arbitrary-path read is bounded (256 KiB, JSON+schema only, no exec). Whether policy wants an additional path-allowlist (e.g. constrain to docs/ or disposable fixtures) is a controller/policy call, not a loader defect; no patch per lane bounds.

## Defect verdict
- No loader defect found. All frozen + regression targets green, frozen SHA unchanged, zero test edits.

## Status
- blocked on missing real transport caller (unchanged from LOADER-IMPL): no production caller of request_profile::profile_for outside request_profile.rs + tests. Loader seam is catalog-backed and fail-closed, but end-to-end user routing needs a real provider transport consumer. Independent verification of this report pending.
