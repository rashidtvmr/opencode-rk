---
name: TDD Discipline
description: Enforce Rust RED-GREEN-refactor for features and bugfixes, compiling RED frozen before minimal implementation
---

# TDD Discipline

Iron law: no prod code without failing test first. Wrote code first? Delete, restart.

## RED

Write one minimal test asserting observable behavior. Must compile, run, fail for missing behavior (not typo). Assert denied paths cause no side effects (docs/TDD.md:32-56). Freeze hash before GREEN.

## GREEN

Minimal real code only. No `todo!()`, `unimplemented!()`, stubs, mocks for success. Never edit frozen tests to pass.

## Verify

Scope order: one test, then target, crate, workspace. Bounded:

```sh
CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 timeout 120 rtk cargo test -p <crate> --test <target>
```

Keep 8GB budget: one heavy command at time, narrow on timeout. Deterministic fixtures only: no net, clock, `#[ignore]`.
