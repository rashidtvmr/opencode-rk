# Convergence V2: integrated product gates

The integrated running product is the completion authority. See `docs/AGENT_STRATEGY_V2.md` and `AGENTS.md`.

Current orchestration: `AGENT_STRATEGY_V2.md`; predefined expert
roles: `AGENT_WORKER_PRACTICES.md`. Historical inventories are evidence, not a release gate.

## Hard golden journey

The core application is not accepted until a release-built `oc2`/`opencode2` from a fresh disposable HOME can, on the exact integrated revision:

1. launch with no subcommand into the native interactive path;
2. discover/start exactly one authenticated daemon and attach without manual `serve`;
3. configure a provider in-app when missing and use that persisted configuration in the actual request;
4. render native OpenTUI and accept raw interactive input;
5. submit a prompt through the same daemon-owned engine used by web/headless;
6. stream provider output, receive a tool call, authorize through the real broker, execute it, persist typed call/result metadata and continue the provider turn;
7. submit a second user turn after the tool call;
8. exit/restart and resume identical typed history;
9. attach a second client without duplicate work;
10. exercise denial, interruption, daemon restart and terminal restoration;
11. rerun the gate on the exact integrated SHA.

## Gates

- G0 Preservation: preserve unique/dirty work; salvage only for an active product
  need. Exhaustive legacy disposition is not a prerequisite to Phase-1 delivery.
- G1 Startup: fresh launch and authenticated daemon ownership/reuse.
- G2 Provider: persisted/in-app provider config drives a real request and first response.
- G3 Tool: real permission broker + tool + typed result + provider continuation.
- G4 Durability: second turn, restart/resume and second-client attachment.
- G5 Native TUI: real PTY/raw input/render/resize/suspend/resume/restoration.
- G6 Web: same daemon/session engine from a real browser, including tool + reload/resume.
- G7 Ubuntu: release archive installs and same golden path runs on Ubuntu.
- G8 Release: Mac + Web + Ubuntu pass on one exact candidate SHA with security/adversarial smoke checks.

Gates may advance in parallel when path ownership does not overlap. A gate is GREEN only on the exact integrated revision.

## Scheduling

Use the complete-feature workspace contract in `AGENT_STRATEGY_V2.md` section 2:
expert owner, registered disk-backed feature worktree outside `.git`/temp/RAM
storage, complete wiring/tests, mergeable revision, owner-fixed
conflicts, prompt merge/push and proven safe retirement. The main orchestrator is
not a manual assembler of file or JSON fragments. Runtime gaps remain explicit.

Work from observed failures or approved upstream parity gaps. Expert owners span
the files needed for end-to-end behavior, with non-overlapping checkpoint grants.
Deliver small wired checkpoints continuously; source inspection is embedded.
Use the current configured ceilings, not the historical four-candidate cap.
Four integration-preparation lanes are authorized; final canonical publication is
one guarded atomic transaction. Actual concurrency depends on implemented runtime
support and measured resource admission, not configuration numbers alone.

Historical `ralph.json`, claims and worklogs are evidence/requirements inventory, not the V2 execution queue.

## Acceptance

Only `ACCEPTED` on the exact integrated revision unlocks dependencies. Claim completion, a candidate branch, source presence and self-reported GREEN do not.

## Tests

Implementers cannot weaken semantic contracts. Independent test owners may repair non-semantic harness/compiler/fixture problems. A wrong historical contract may be superseded by an independent evaluator when a higher authority unambiguously contradicts it. HITL is reserved for actual product ambiguity or unavailable external authority.

## Breadth after core convergence

After G1-G8 are GREEN, resume approved extensions such as remote gateway/control, infinite delegation canvas, iOS, Android, notifications and broader client parity using the same failure-driven package model.
