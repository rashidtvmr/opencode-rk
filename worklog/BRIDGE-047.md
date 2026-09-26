# BRIDGE-047 local_settings

Claim: per-directory `{model, theme}` store, std only, IO via `crate::persistence`.
Source: `packages/tui/src/context/local.tsx` model store :137-180 (state/model.json `{recent,favorite,variant}`; live `model` map NOT persisted), fallback :236-245; theme discovery `context/theme.tsx:37-61`, selection in KV; KV flat get/set `context/kv.tsx:51-56`; IO `util/persistence.ts` via `persistence.rs` tmp+rename @ a0d9b6c.
Target: `crates/opentui-bridge/src/local_settings.rs` only. lib.rs NOT touched (owner pre-wires).
Tests: RED not run (no cargo per scope); logically green, 9 tests min 5.
Decisions: line format `dir \t model \t theme` with `\`-escapes; from_text fail-closed (dup key/over-cap/bad line -> None); load malformed -> `Persist(JsonShape)`; bounds 128/64/1024/128 per contract.
Unknowns: where owner mounts module in lib.rs; exact file path/name for durable file on disk.
