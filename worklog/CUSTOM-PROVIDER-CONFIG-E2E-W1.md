# CUSTOM-PROVIDER-CONFIG-E2E-W1

- Claim: `CUSTOM-PROVIDER-CONFIG-E2E-W1`; session `ses_f1c5c790fffem57G3U9DJR05Q6`; assigned route `9router/luna-free` verified against user-approved route. Owned paths: `crates/cli/tests/custom_provider_config_e2e.rs` (frozen test), this pad, own claim row.
- Base commit: `af9766a` (draft test authored). Branch `red/CUSTOM-PROVIDER-CONFIG-E2E-W1`.
- Host: aarch64-apple-darwin; vendored native opentui at `crates/opentui-bridge/native/lib/x86_64-unknown-linux-gnu` only. Release binary already built at `/Users/mymac/Projects/opencode-rk-phase1-restored/target/release/opencode-rk` by prior approved setup; worktree-local target directory lacks that binary. No binary build attempted by this execution-only task.
- Source evidence (af9766a):
  - `crates/cli/src/main.rs::serve` 688-760: loads only models.dev cache or `Catalog::default()`; never reads `opencode.json` project config. No provider-config plumbing.
  - `crates/server/src/lib.rs::search_models` 459-477: serves `state.catalog` (models.dev only). No project provider entry merge.
  - `crates/server/src/lib.rs::create_turn` 929: `OpenAiResponsesClient::from_env_for(provider_id)` - env-only resolution.
  - `crates/providers/src/config.rs::ProviderConfig::from_env` 84-115 + `get_api_key` 148-150: single env var per provider; no project `opencode.json` options (baseURL/headers/body).
  - `crates/providers/src/responses.rs`: no project custom headers/body forwarding path.
- Observable contract (frozen test): disposable data dir + project CWD/config; `env_clear`, only `ACME_FALLBACK_KEY` set; real installed `serve`, descriptor bearer, authenticated `/api/models` exposes `acme/model-key`; POST `/api/sessions`; POST turn `acme/model-key`; GET `/api/sessions/<id>/messages` persists user+assistant; provider fixture receives exactly ONE loopback POST `/v1/responses` with fallback bearer, custom header `x-acme-config`, custom body key `acme_extension`, wire model ID; no extra provider/discovery calls; no credential leak in API responses/transcript.
- Task constraints (per user route): Build opencode-rk binary no-default-features. Locate serde_json rlib produced in this exact target dir. Compile test standalone with `rustc --test -L dependency` and exact `--extern serde_json`. Test imports only std + serde_json. Run with OPENCODE_RK_BIN. Require genuine RED at acme/model-key absent/config ignored. Bound process/time/bytes cleanup. Freeze SHA. Blocked note, commit/push. No Cargo test/native opentui.

## Execution

### Steps
1. Prior claim owned by stopped setup session `ses_f1c99ba85ffeCzCU13TGuyEy4R` was reclaimed with delegation evidence, then claimed by this session through `tools/completion_claims.py`.
2. Reused prebuilt release binary: `/Users/mymac/Projects/opencode-rk-phase1-restored/target/release/opencode-rk`; no-default-features build was completed in the setup session.
3. Reused exact serde_json rlib `target/debug/deps/libserde_json-24416be3666bbe41.rlib` available in this worktree.
4. Compiled standalone Rust test with no Cargo/native build. Changed fixture temp root from a long descriptive name to `rk-<pid>-<id>` after reproduction showed macOS Unix socket path exceeded `SUN_LEN` before reaching daemon API. Test now reaches the expected contract assertion.

### Verification
- `rtk rustc --test --edition=2021 -L target/debug/deps --extern serde_json=target/debug/deps/libserde_json-24416be3666bbe41.rlib crates/cli/tests/custom_provider_config_e2e.rs -o /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/custom_provider_config_e2e-w1` -> exit 0.
- `rtk python3 -c 'import os,subprocess; test="/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/custom_provider_config_e2e-w1"; binary="/Users/mymac/Projects/opencode-rk-phase1-restored/target/release/opencode-rk"; env=os.environ.copy(); env["OPENCODE_RK_BIN"]=binary; p=subprocess.run([test,"--test-threads=1","--nocapture"],env=env,stdout=subprocess.PIPE,stderr=subprocess.STDOUT,text=True,timeout=60); print(repr(p.stdout)); print("exit",p.returncode); raise SystemExit(p.returncode)'` -> exit 101; test compiled and launched binary successfully, reached `crates/cli/tests/custom_provider_config_e2e.rs:365:5`, `project provider/model key must appear in live /api/models catalog`. Expected RED: missing `acme/model-key`; no API/provider turn submitted, loopback provider listener times out and drops. Not setup, native, or compile failure.
- Fixture budget: one daemon child, one provider fixture thread, per-I/O timeout 2s, journey deadlines 8s, response cap 256 KiB, test runner hard timeout 60s. Disposable temp files removed on drop.
- Frozen test SHA-256: `6803ed3d094a6ce5127c1e3e0e41fe3c6f8c34d280897c8a471dae05b1b74964`.

### Status
blocked: RED confirmed; implementation not part of this lane. No feature implementation or acceptance claimed.
