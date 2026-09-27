# Wave 5 - Finish required local extras and harden the integrated app

Status: DRAFT. Entry: `P1-G4` independently passed.

## Demonstration that closes this wave
All approved local requirements, including required research/voice, pass integration, adversarial security, recovery, resource and source-differential gates. Scope is frozen for release proof.

## Work packages
- **P1-W5-01 - Finish real search and deep-research workflows with citations**. Search/research retrieves real sources and produces reviewable cited output; local grep or fabricated references never count.
- **P1-W5-02 - Finish opt-in voice and dictation with a real adapter**. Microphone capture is opt-in and produces a real editable transcript; a recording icon or mocked transcription is not completion.
- **P1-W5-03 - Connect graph/canvas and operational views to real agent state**. Required operational visualization is useful and consistent with the actual engine; a new general workflow-builder product is not invented in this phase.
- **P1-W5-04 - Prove actual operating-system isolation on every release target**. A hostile generated tool cannot access secrets or protected/cross-project state beyond an explicit grant even when its text looks harmless.
- **P1-W5-05 - Harden the browser, local API and extension trust boundaries**. Untrusted websites, transcripts, artifacts, plugins or local listeners cannot act as the user or exfiltrate secrets.
- **P1-W5-06 - Finish crash recovery, cancellation and lifecycle cleanup**. Failure at any boundary leaves safe, truthful, recoverable state and no orphan resources.
- **P1-W5-07 - Meet measured resource and long-session performance budgets**. Measurements cover the whole app and disabled features, not just a Rust parent process or one bounded collection.
- **P1-W5-08 - Validate the model-agnostic execution and review workflow**. Cheaper or heterogeneous models can execute the plan without editing one another's files, self-certifying or overwhelming the machine.
- **P1-W5-09 - Run source differential and adversarial feature acceptance**. Every required local feature has real execution evidence or remains open; source inventory completeness is not mistaken for behavior parity.
- **P1-W5-10 - Close integrated defects and freeze the release-candidate scope**. Reserve capacity for real integration fixes; Wave 6 is release proof, not the first time all modules meet.

## Gate criteria
- No unapproved feature deferral, semantic mapping gap or blocking integrated defect.
- Actual OS isolation and hostile-content tests pass on each declared target.
- Resource thresholds were frozen before measurement and are not raised to obtain GREEN.

## Capacity and sequencing
Freeze shared contracts first; then dispatch only non-conflicting one-file candidates. Reserve integration and independent review capacity. Use one heavy validation token across the wave, not one build per worker. A wave may contain multiple bounded dispatch rounds; wave count is not a concurrency or time estimate. Keep repair capacity rather than filling every slot with new breadth.
