# BASE-001 - Server control plane event bus (opencode.server-control-plane)

Status: COMPLETE. Kind: product. Runtime optional: False.
Mandatory for full declared release: yes.
Requirements: opencode.server-control-plane.
Dependencies: none.
Ownership locks: crates/server/src/event_bus.rs, this card.
Suggested module: `crates/server/src/event_bus.rs` (bounded only).

## User-observable outcome

Server daemon components exchange control events over a bounded
fan-out bus. No unbounded queue, no blocking publish, dead
subscriptions pruned.

## Source evidence

- `crates/server/src/event_bus.rs` (EventBus, ServerEvent, EventKind,
  Subscription, BusError): this lane.
- `crates/server/src/lib.rs` line 3 (`pub mod event_bus;`): pre-wired.
- `crates/server/Cargo.toml`: tokio mpsc only, no new deps.

## Observable contract

- `EventBus::new(capacity)` bounded per-subscriber mpsc depth.
- `publish(event)` fan-out clone to matching subs via `try_send`,
  `Err(BusError::Full)` if any target full, never blocks.
- `subscribe(Option<EventKind>)` returns `Subscription` with own channel.
- `Subscription::recv()` async, `try_recv()` non-blocking, `Drop`
  removes entry from bus (id-keyed).
- `subscriber_count()` live sub count.
- Deviation: spec says `PermissionRequested(OperationIntent)`; server
  crate has no security dep, so payload is `String`. Upgrade path:
  add `opencode-rk-security` dep or shared intent type in contracts.

## Failure states

- Publish to full subscriber buffer: `BusError::Full`.
- Dropped receiver pruned lazily on next publish plus eagerly on Drop.
- Filtered sub skips non-matching kinds at publish (no queue waste).

## Acceptance criteria

- BASE-001-T01 publish_and_receive green.
- BASE-001-T02 bounded_rejects_overflow green.
- BASE-001-T03 multiple_subscribers green.
- BASE-001-T04 filtered_subscription green.
- BASE-001-T05 drop_subscription_cleanup green.
- `cargo check --workspace` clean.

## Test-first execution

1. RED: stub file replaced directly (no prior suite in module).
2. GREEN: mpsc fan-out implementation; 5 tests pass.

## Evidence

- `cargo test -p opencode-rk-server -- --list`: 5 tests listed.
- Test binary direct run:
  `running 5 tests / test event_bus::tests::multiple_subscribers ... ok /
  test event_bus::tests::filtered_subscription ... ok /
  test event_bus::tests::drop_subscription_cleanup ... ok /
  test event_bus::tests::bounded_rejects_overflow ... ok /
  test event_bus::tests::publish_and_receive ... ok /
  test result: ok. 5 passed; 0 failed; 0 ignored.`
- `cargo check --workspace`: Finished dev profile, EXIT 0.
- Note: `cargo test -p opencode-rk-server` harness stdout swallowed
  by runner (prints only Finished line); binary run above is the
  verbatim receipt.
