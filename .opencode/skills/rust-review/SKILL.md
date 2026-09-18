---
name: Rust Review
description: Rust PR review for clippy, error handling, unsafe boundaries, and panic-DoS
---

# Rust Review

Use for Rust PRs, clippy failures, error-handling audits, `unsafe` changes in this workspace.

## Gates (run all, must be green)

```sh
cargo clippy --all-targets -- -D warnings
cargo fmt --check
cargo test --workspace
```

Repo: Rust 1.85 workspace, 12 crates (`contracts`, `foundation`, `storage`, `security`, `sessions`, `catalog`, `server` axum, `cli` clap+reqwest, `providers`, `tools`, `agents`, `opentui-sys`).

## Checklist

- [ ] `clippy -D warnings` clean, `fmt --check` clean.
- [ ] Errors via `thiserror`; no `anyhow` in library code; no discarded `Result` (`#[must_use]`, no `let _ =` on I/O).
- [ ] No `todo!()`, `unimplemented!()`, `panic!()` on untrusted input; no `unwrap`/`expect` outside tests.
- [ ] No stub GREEN: real assertions on real impl, no mocked success, no weakened asserts.
- [ ] Safe/unsafe boundary: `unsafe` minimal, `// SAFETY:` invariant documented, no ` transmute`, no `as` pointer casts, no `set_len`/`assume_init` without init proof.
- [ ] Panic-DoS: check OOB indexing, `str` slicing at non-char boundary, arithmetic overflow, `unreachable!`/`assert!` reachable from input, `RefCell` double-borrow.
- [ ] Concurrency/async (if touched): no `MutexGuard` double-lock, no ABBA ordering, no blocking call in async, no cancellation-unsafe `.await`.
- [ ] FFI/packed (if touched): `CString::as_ptr` lifetime, ABI/`repr(C)` padding, no unaligned `repr(packed)` refs.
- [ ] Capability broker honored (`docs/SECURITY.md`); no secret logging, no shell-string concat, no unbounded queue/output.

## Output

Report as: `path:line` + bug class + fix. Fail review if any gate red or any `todo!/unimplemented!/unwrap` on untrusted path.
