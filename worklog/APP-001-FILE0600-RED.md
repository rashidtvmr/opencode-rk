# APP-001-FILE0600-RED

Claim: `ses_f1e6dc07effex9X5y66C0PuKM1`, test-author; authorized route xkiro/openai/gpt-6-luna, user-approved allowlist. Base `798cbff99f4e2a2ad21cc937df428eb6bd871cca`.

Source evidence: `crates/cli/src/main.rs:256-262` routes setup and claims no credentials are persisted; `main.rs:352-371` identifies `MemoryAccountStore`; `main.rs:387-449` performs interactive collection and completes only in memory. New requirement is explicit Atomic File0600 plaintext persistence, protected by data_dir; no encryption or secure-erase claims. Contract: completed setup atomically commits provider/model plus credential in a file mode 0600; cancellation/failure leaves no partial credential; raw credential is never emitted to PTY, argv, URL, or unprotected metadata. Child lifetime must be bounded and reaped; test root is disposable; output capped at 64KiB.

Convergence gate was run before task selection and failed (exit 1, total=85 findings, including extensive off-plan completed IDs); task claim then succeeded. Test authored: `crates/cli/tests/installed_setup_file0600.rs`. Initial test checks real `OC2_E2E_BIN` through macOS `script` PTY and expects discoverable protected credential file. Remaining requested cancellation/symlink/replacement checks were not safely constructible without stable destination convention. No product code touched.

RED command/hash and final status to be appended after bounded attempt.

## 2026-09-26 correction pass (session ses_f1e6ac10bffekPrR3HVz5M4alC)

Prior uncommitted test was contradictory (it asserted the raw key must be
absent from the credential file, which contradicts the approved plaintext
contract) and is NOT frozen. Claim was reclaimed for this session and the owned
test was rewritten to the approved contract.

Approved contract now encoded in `crates/cli/tests/installed_setup_file0600.rs`:
- credential dir exactly `<data_dir>/credentials` (0700);
- file exactly `<data_dir>/credentials/<provider>.json` (0600);
- plaintext JSON MAY contain provider/model/credential (no encryption claim);
- raw credential never re-emitted by the program (PTY echo counted once);
- atomic commit, no `.tmp` residue;
- partial setup creates no credential file;
- pre-created symlink at the exact path is not followed and setup fails;
- existing valid file not replaced by invalid/partial input.

Frozen test sha256: `c99b15bca871f1258f4ae3c886a57cc108a695476083da057caeae7cec0d3e40`
(288 lines). Base HEAD: `798cbff99f4e2a2ad21cc937df428eb6bd871cca`.

RED command (std-only rustc harness; no cargo, avoids opentui native gate):
`rustc --edition 2021 --test crates/cli/tests/installed_setup_file0600.rs -o /tmp/oc2-file0600-build/installed_setup_file0600`
compiles clean (RUSTC_EXIT=0). Run:
`OC2_E2E_BIN=<impl .../target/debug/oc2> RUST_TEST_THREADS=1 /tmp/oc2-file0600-build/installed_setup_file0600 --test-threads=1`
=> `test result: FAILED. 2 passed; 3 failed` (exit 101).
- t01 FAILS "credential directory <data_dir>/credentials missing"
- t02 FAILS "credential directory missing"
- t04 FAILS "setup must fail on a pre-created symlink" (exit 0 != 0)
- t03/t05 pass (absence-only: no file written yet)

This is a genuine RED for the missing Atomic File0600 persistence behavior, not a
test artifact. Status: test-author lane RED frozen; implementation is a separate
lane (owned product file `crates/cli/src/main.rs` is NOT touched here).
