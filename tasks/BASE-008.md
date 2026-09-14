# BASE-008 - Effect runtime adapter

Status: GREEN. Kind: product. Requirements: effect-runtime surface.

## Outcome

Async effect execution with retry, timeout, and result capture. `EffectRunner::new(default_timeout, max_retries)` drives all ops. `run` executes Io via tokio::fs, Db/Net return `Failure("not implemented")`. Failure retried up to max_retries, Timeout terminal. `run_batch` joins parallel with JoinSet, preserves input order. `dry_run` validates: non-empty DB statements, non-empty Io paths, http(s) Net URLs.

## Module

- Owned: `crates/foundation/src/effect.rs` (`pub mod effect;` pre-existing in lib.rs). tokio only, std fs via tokio::fs.
- Types: `Effect(Db/Io/Net)`, `DbEffect(Query/Execute)`, `IoEffect(ReadFile/WriteFile)`, `NetEffect(HttpGet/HttpPost)`, `EffectResult{effect,outcome,duration}`, `Outcome(Success/Failure/Timeout)`, `EffectRunner`.
- Extra: `run_op` (generic retry/timeout driver for tests and future executors), `default_timeout`/`max_retries` getters.

## Tests (in-module, 5)

- io_read_write_roundtrip, timeout_returns_timeout_outcome, dry_run_validates, batch_parallel, retry_on_transient.

## Commands

- `cargo test -p opencode-rk-foundation`: 27 passed.
- `cargo check --workspace`: 0 errors.
