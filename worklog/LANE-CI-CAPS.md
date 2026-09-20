# LANE-CI-CAPS scratchpad
claim: LANE-CI-CAPS ses_f423ccebdffemjzOag67siYSxl
source: .github/workflows/ci.yml:12-13 env, :29-47 rust job
observed: top env only CARGO_TERM_COLOR; rust job check/test/clippy uncapped
target: top-level env CARGO_BUILD_JOBS=2 RUST_TEST_THREADS=2 (caps all cargo steps, both matrix OS)
tests: tools/validate_repository.py
decision: global env cap, no matrix change, no src/tests touch
