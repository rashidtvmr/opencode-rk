---
name: Tokio Async
description: One Tokio runtime per daemon, spawn_blocking for SQLite, bounded channels, timeout/select, graceful shutdown
---

# Tokio Async

Applies to daemon, server (Axum 0.8), tools (`tokio::process`).

## Rules

- One `#[tokio::main]` / `Builder::new_multi_thread()` runtime per daemon. Bounded CPU pool only where profiled. Never per-agent runtime or per-subagent OS process (PLAN.md ADR-001).
- Never block executor: rusqlite/SQLite sync I/O only via `tokio::task::spawn_blocking`. No `.await` holding blocking lock/guard.
- Bounded `mpsc::channel(N)`: bound count + bytes. `try_send` handles `Full`/`Closed`; no unbounded `Vec`/`String` growth; enforce byte budget (PLAN.md §9).
- `tokio::select!` + `tokio::time::timeout` for cancellation (parent/child, slow client, provider stall). No retry of ambiguous side effects without classification.
- Graceful shutdown: `tokio::signal::ctrl_c()` in `select!`; drain, kill child (`child.kill().await`), close FDs.
- Assert reclaim: cancellation test proves tasks/processes/FDs reclaimed, JoinHandles joined/aborted.
