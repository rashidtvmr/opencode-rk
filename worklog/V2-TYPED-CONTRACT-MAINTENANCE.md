# DB-022 typed HTTP contract maintenance

- **Owner scope:** `crates/storage/tests/typed_tool_history_http.rs` and this
  worklog only. No product, registration, metadata, pin, or restart-test changes.
- **Base:** `6291caf` (installed typed candidate). The original registered target
  was executed before this repair and failed at its obsolete disclosure assertion:
  the installed product returned `write requires human approval` at line 232.
- **Superseded historical assertions:** the old `output_payload.contains("fixture")`
  assertion is superseded by approved security-905 nondisclosure: the exact output
  is `write success`, with its existing byte-length assertion retained. The old
  `assert_ne!(failed_status, 201)` is superseded by approved correction `61d473ab`:
  late persistence failure retains HTTP 201 because headers were already sent and
  emits terminal NDJSON `error` / `internal_error`.
- **Mechanical fixture repairs:** canonicalize the disposable project before
  embedding its path in broker arguments and daemon startup (avoids `/var` versus
  `/private` alias mismatch); decode chunked HTTP framing before validating the
  late NDJSON stream. These repairs do not weaken semantic assertions.
- **Frozen assertions retained/strengthened:** exact typed call identity, tool
  name, serialized arguments, and call byte length; exact output identity/name,
  nondisclosing output, and output byte length; no failed typed rows; no failed
  ordinary tool message; no provider continuation; and no failed file side effect.
- **Authority evidence:** `61d473ab` changes the late-stream status contract;
  `worklog/V2-TYPED-HISTORY-CONTRACT.md` in canonical `main-v2` records the
  approved security-905 output and the independent restart contract. Pinned
  upstream remains the ordinary protocol authority; this target uses the already
  approved opencode-rk safety deviation for tool output.
- **Mechanical fixture completion:** the fault operation now uses distinct
  `project/fault-output.txt`, so the absence-of-side-effect assertion cannot be
  confused by the successful `typed-output.txt`. The first turn's provider
  continuation is explicitly drained before the fault request, and its exact
  typed call/output pair is validated.
- **Baseline RED (clean product failure):** on installed
  `v2-fc2d201-http-odzd04nk/bin/oc2`, using the required serial command and
  target directory, the test compiled and ran but failed at the typed output
  query with `QueryReturnedNoRows` (line 271), exit 101, 0.09s. This is the
  expected clean product RED; no fixture timeout or request-order failure.
- **Candidate GREEN:** on installed
  `v2-typed-6291caf-ee0dsshf/bin/oc2`, using the same command, the target passed
  `1 passed; 0 failed` in 0.10s, exit 0. Only existing compiler warnings were
  emitted (duplicate CLI target, unused storage/test variables).
- **Corrected source SHA-256:**
  `d4ea1f125f5d84ae85d8a0328c46e727cd08aa89a51f20efad73c17aa089435c`.
- **Validation status:** PREVERIFIED candidate only. Main must rerun the
  registered DB022 target on the exact integrated SHA before ACCEPTED.
