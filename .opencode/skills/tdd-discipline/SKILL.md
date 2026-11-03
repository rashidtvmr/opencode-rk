---
name: TDD Discipline
description: Enforce Rust RED-GREEN-refactor for features and bugfixes, compiling RED frozen before minimal implementation
---

# TDD Discipline

Feature owners work in a registered persistent-disk Git worktree with an explicit expert role
and test grant (`docs/AGENT_WORKER_PRACTICES.md`). Return a mergeable revision and
verification evidence, not copied-file fragments. Author/test-runner failures
return to the feature owner for repair; main is not the default code assembler.
No temp-folder or RAM-disk implementation fallback. Disposable test fixtures may
be temporary, but export required evidence to granted persistent disk storage.

Use `docs/TDD.md` and the current implementation-first policy. Establish a
meaningful compiling RED before the repair. If a candidate already exists,
preserve its bytes and demonstrate RED on its base in an isolated fixture;
never delete or reset user/peer work to manufacture test ordering.

## RED

Write one minimal test asserting observable behavior. Must compile, run, fail for missing behavior (not typo). Assert denied paths cause no side effects (docs/TDD.md:32-56). Freeze hash before GREEN.

## GREEN

Minimal real code only. No `todo!()`, `unimplemented!()`, stubs, mocks for success. Never edit frozen tests to pass.

## Verify

Scope order: focused behavior, then target, affected crate and required workspace
checks. Use the controller's granted argv, timeout, resources and shared locks.
Configured maxima are not actual headroom: account for linker/test memory, CPU,
I/O and other jobs. Do not bypass the existing exclusive heavy-validation lock
until a tested shared admission implementation replaces it. Never share a mutable
Cargo target between concurrent builds.

Expert implementation owners may author new tests in granted paths and continue
independent work while verification waits. Protected semantic tests stay frozen.
Use deterministic disposable fixtures (including controlled local provider stubs
and injected clocks); never user databases, external side effects or ignored tests
to manufacture GREEN. See `docs/AGENT_STRATEGY_V2.md` sections 5–7.
