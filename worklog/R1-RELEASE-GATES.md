# R1 release-gates — exact-SHA gate contract (candidate, not acceptance)

Package: R1 (release-readiness orchestrator). Base: `ddff2e6d617f2c70f60ed046f7af7636087f1d61`.
Status: PREVERIFIED_STATIC only. NO release-readiness claim. R1 integrated at `f5f440d8f4ea7085409a06f10343494945fef6c0` failed acceptance because the tracked template hard-coded `ddff`; R1.1 corrects this with fail-closed `<SOURCE_SHA>` materialization. Heavy platform gates NOT run here.

## Changed paths (grant: release/**, scripts/**, CI workflows, tests/release-gates/**, this worklog)

- `release/receipts.py` (NEW) — schema + stale-reuse validators: manifest (source_sha/binary/library/archive/runner/frozen_tests), artifact-hash binding, frozen-test rehash, runner pin, receipt validation (exact-SHA, exit 0, limit_failure null, log re-hash), fresh-evidence-dir guard, disposable HOME/XDG/TMP env + credential scrub.
- `release/run_release_gates.py` (NEW) — bounded serial gate driver. Modes `--check-contract` (static, safe anywhere) / `--run` (executes manifest gates). Real subprocess exit codes via direct Popen + select byte-budget reader + process-group kill (retained verify-native-utf8.py pattern; no shell, no pipe/tail masking). Rejects stale manifest/artifact/receipt cross-SHA reuse; `summary.json` carries `release_readiness_claim: false`.
- R1.1 — `--manifest-template` is restricted to static checks and materializes both source fields into fresh evidence atomically; `--manifest` remains concrete exact-SHA input, and `--run` requires artifacts.
- `release/verify-native-utf8.py` (NEW copy) — retained Ubuntu runner verbatim, sha256 `b135a456f9dae899af79f6249c25cf116ba7917f6141123d36b290a80d94d423`, pins `--tool-gates` + platform evidence (loader ABI, `$ORIGIN/../lib`, exports, tools clippy/test/shell_owned_lifetime). Original retained at `/private/var/folders/.../v2-ubuntu-arm64-release-dqhrz_mn/verify-native-utf8.py`.
- `release/release-gates.manifest.template.json` (NEW) — schema `r1-release-gates/1`. frozen_tests hashes verified at base via `git show ddff2e6:<path> | shasum -a 256` (matches retained a79ae96 build.json for shared 5). binary/library/archive = zero placeholders until a real attested build exists. Platform gates: mac = self-checks + `macos_loader` + frozen `native_bracketed_paste.py` paste-reset on installed binary; web = `web_tool_journey_fixture.py` real Rust router/auth daemon (no Python re-enactment); ubuntu = retained runner `--tool-gates` under `/work` sandbox layout.
- `.github/workflows/release-gates.yml` (NEW) — SHA-pinned actions, timeout-bounded, disposable evidence dirs uploaded; `contract` job static-only on push; heavy platform jobs `workflow_dispatch`-gated with explicit `source_sha` input.
- `tests/release-gates/test_receipt_validation.py` (NEW) — 24 stdlib unit tests: schema, swapped/missing artifact, mutated frozen test, tampered runner/log, stale cross-SHA receipt (manifest + per-gate), wrong platform, exit-code capture (7 preserved), timeout kill, env hygiene, driver subprocess contract pass + wrong-SHA reject + tool-gates gating.
- Unchanged: `release/test_release_contract.py` (SHIP-001 RED preserved: 6 fail at base AND at candidate), all `scripts/**` (install-oc2.sh reused as-is), `Cargo.lock`, crates, existing tests.

## Verification (this candidate)

- `python3 -m py_compile` all new/owned scripts: OK
- `python3 -m unittest discover -s tests/release-gates`: `Ran 24 tests ... OK`
- `python3 release/run_release_gates.py --source-sha ddff2e6d… --manifest release/release-gates.manifest.template.json --platform mac --platform web --platform ubuntu --check-contract`: exit 0 (runner copy hashes to pin; frozen tests byte-identical at base)
- `python3 tools/validate_repository.py`: OK exit 0
- `python3 release/test_release_contract.py`: `FAILED (failures=6)` identical to pristine base export (pre-existing SHIP-001 RED, workflow-signing; not introduced)
- bootstrap discover: candidate FAIL/ERROR set ⊆ base set (no new failures; base-only lines = pre-existing flakiness)
- `native_bracketed_paste.py --self-check` + `web_tool_journey_fixture.py --self-check --artifact-dir <fresh>`: pass
- No Cargo run (per grant).

## Future invocation (exact, once an attested release build exists)

Mac:
`python3 release/run_release_gates.py --source-sha <SHA> --manifest release/gates.<SHA>.json --artifact-dir <dir>/macos-arm64 --platform mac --run`
Web:
`python3 release/run_release_gates.py --source-sha <SHA> --manifest release/gates.<SHA>.json --artifact-dir <dir>/linux-x64 --platform web --run`
Ubuntu (arm64 sandbox, /work layout, retained runner semantics):
`python3 release/verify-native-utf8.py --sha <SHA> --source-name source-<tag> --output-name ubuntu-native-<tag> --tool-gates`
Precondition: regenerate manifest per integrated SHA (zero-hash placeholders replaced from build.json; driver rejects placeholders-as-artifacts via hash binding).

## Blockers (current product REDs preventing gate execution, not contract validation)

1. No attested installed release binary at any canonical SHA: manifest artifact hashes are placeholders; T01-T05 SHIP-001 `release/test_release_contract.py` 6 RED (missing `.github/workflows/release.yml` signing/notarize wiring + installer rollback/arch gaps).
2. Mac paste-reset gate: `native_bracketed_paste.py` is design-RED against current native_loop (no bracketed-paste enable/Paste event/Composer::apply_paste caller) → `--run` on mac fails until native paste loop lands.
3. `macos_loader` cargo test + ubuntu `--tool-gates` cargo clippy/test gates require Cargo execution — out of scope here, owned by active workers.
4. Ubuntu full runner needs the exact integrated SHA's source archive + `/work` arm64 sandbox (retained receipts cover a79ae96/ac635fa/de7e05f lineage only; none at canonical tip).

## Disposition

Candidate on `v2/r1-release-gates`. No canonical landing, no push, no release accepted. Historical retained artifacts untouched (read-only reuse).
