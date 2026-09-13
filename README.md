# Lean Harness - source-pinned native-agent project kit

**Working name, not an existing product.** Rust + Tokio core; embedded SQLite;
optional compatibility hosts. Prepared 2026-09-13.

This repository is a detailed implementation plan and development-controller
bootstrap, **not a completed OpenCode replacement**. It contains 193 planned
slices, 965 named test obligations, and traceability for 35 explicit requirements.
The 965 obligations are specifications, not 965 implemented or passing tests.

## Start here

1. Read `PLAN.md`, then `AGENTS.md` and `docs/SECURITY.md`.
2. Give `prompts/START_HERE.md` to the coding harness that will do the work.
3. Run the local bootstrap checks below. They use Python 3.11+ and its standard
   library; the final application does not require Python.
4. Fetch the exact reference commits, inventory them, and complete DISC-001 through
   DISC-010. Do not call the feature list exhaustive until that audit is accepted.
5. Configure trusted worker, verifier, and sandbox adapters once before using the
   prototype loop. `docs/ADAPTER_PROTOCOL.md` describes the executable contract.

```sh
python3 tools/validate_plan.py
python3 -m unittest discover -s tests/bootstrap -v
python3 tools/ralph_loop.py --list-ready

# Explicit network operation; does not install or run upstream dependencies.
python3 tools/freeze_sources.py --fetch
python3 tools/inventory.py

# Expected to fail until the full inventory and independent reviews exist.
python3 tools/coverage_gate.py
```

## What to hand to an agent

`ralph.json` is the canonical extended plan. It retains familiar Ralph story
fields and adds dependencies, boundaries, test obligations, source evidence,
resource requirements, and acceptance gates. `prd.json` is a conventional Ralph
export. A generic runner that only reads `passes` does **not** enforce this kit's
security, dependency, or verification rules. Do not use that export as an
independent acceptance authority.

`tasks/ID.md` is each worker's task card. `FEATURES.md` is the navigable index.
`feature-ledger.json` links source families to slices. `requirements/` maps every
explicit requested capability to its task IDs. `sources/` records pins and the
limits of the current inspection.

## Reference pins

| Reference | Exact commit |
|---|---|
| anomalyco/opencode, observed `dev` | `95daf90670b7c039c436c85537da5fbfe2205b41` |
| decolua/9router, observed `master` | `17c4cc76877bd1755030a8414f8d0083f48dcccf` |

Selected V2 code, package trees, specifications and 9router routing code were
inspected. The complete repositories were **not** cloned in the authoring
environment. A full-file review, runnable upstream baseline, exact license audit,
models.dev fixture capture and source-exhaustiveness certification remain work
items. V2's own sources explicitly distinguish implemented, partial and planned
behavior. This kit preserves that distinction.

## Autonomous execution boundary

After one-time credentials, sandbox, cost limits and trusted adapters are
configured, the controller can select eligible tasks, request tests, demand RED,
request implementation, demand independent verification and integrate accepted
work. The included bootstrap is **serialized**, deliberately suitable for a
small machine. Production leased parallel worktrees, automatic scope expansion
and hardened controller deployment have dedicated AUTO tasks.

There is no built-in paid model connection, operating-system sandbox, or promise
that a finite program can force all future implementation problems to finish.
The loop stops with `blocked` rather than bypassing safety, inventing credentials,
weakening tests, or declaring incomplete work complete. Application-runtime HITL
is distinct from the development loop; development uses simulated grants.

## Important files

| File | Purpose |
|---|---|
| `PLAN.md` | Architecture, work sequence, definition of completion |
| `AGENTS.md` | Non-negotiable worker contract |
| `ralph.json`, `prd.json` | Extended plan and conventional export |
| `tasks/`, `FEATURES.md` | 193 detailed work items |
| `docs/SECURITY.md` | OS isolation, permission broker, human-only controls |
| `docs/STORAGE.md` | Embedded storage, bounded events, blobs, safe import |
| `docs/TDD.md` | RED/GREEN/refactor and differential evidence |
| `docs/ADAPTER_PROTOCOL.md` | Required trusted integration adapters |
| `docs/AUTONOMOUS_EXECUTION.md` | Loop, recovery, budgets and trust boundaries |
| `docs/SOURCE_AUDIT.md` | Exhaustiveness gates and evidence limitations |
| `config/` | Lean defaults, resource targets and example controller settings |
| `validation/` | Actual bootstrap test logs and structural validation |

The Rust crates are intentionally only an unimplemented scaffold. Rust tooling
was unavailable during preparation; no Rust compilation or product benchmark is
claimed. Optional JS/TUI compatibility is not falsely presented as zero-overhead
native functionality. The current user's 90 GB database was neither accessed nor
modified.
