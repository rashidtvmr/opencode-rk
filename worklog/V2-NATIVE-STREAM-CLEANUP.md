# Native stream fixture cleanup maintenance

## Scope

Mechanical source-only maintenance from canonical base `c84b4f7`. The prior
stream contract remains semantically unchanged: early partial visibility,
typed tool continuation, exact four provider requests, restart history,
secret non-echo, and output bounds are untouched.

## Cleanup repair

The installed `ed727a6577527fc560a6dfc736fbdb0479654f06` run reached the
real stream/tool/second-turn assertions, then failed in the frozen
`native_provider_setup.py:241` helper because the fixture killed the owned
daemon group before reaping the native CLI `Popen`. The new local helper
`reap_cli_then_daemon` sends Ctrl-C to the owned CLI, waits up to ten seconds
for the actual child, then probes the validated daemon PID and invokes the
unchanged frozen `stop_owned_daemon` helper only when that daemon is still
running. It does not catch or suppress the helper's EPERM path and does not
signal an unvalidated group.

The same ordered cleanup is used for the first restart and final cleanup.
Missing descriptors are handled fail-closed without indexing an empty list.
Evidence additionally probes each validated daemon PID and records combined
CLI/daemon cleanup status. Existing failure capture, redaction, and bounds
remain active.

## Source checks

No product, Cargo, PTY, Docker, browser, or Node execution was performed.
The fixture's semantic self-checks and installed runtime gate are unchanged
and must be rerun by the parent against the attested installed release.

Expected source review commands:

```sh
python3 -m py_compile tests/e2e/native_stream_tool.py
git diff --check
```

The known initial product RED remains the missing native early-delta path from
the frozen `f7d8` contract; this patch only repairs owned-process cleanup.
It is not a product GREEN, PREVERIFIED, ACCEPTED, or release claim.

## Controller source correction

The committed `86c4d232` tree did not contain the complete reported correction:
`timed_out` was referenced without assignment, final cleanup still suppressed
errors, and a denied ownership probe still counted as a stopped daemon. The
controller completes those mechanical corrections, preserving the provider,
history, framing, barrier and self-check ASTs. Forced CLI termination and
unproven daemon cleanup always fail the gate after writing bounded evidence.
The existing helper's ownership guard remains active. Actual installed runtime
verification and an exact new contract hash are required before acceptance.

## Executable mechanical contract verification

Final integrated cleanup source is `963b88f7356567558950336687f65c1c756ac83d`;
the actual test SHA-256 is
`c6d8d561a96136061c4d6e87e183bad144e762df17847fcfca4a347d0d30633e`.
The provider, history, framing, barrier and self-check ASTs remain identical to
the prior frozen `f7d8` contract. The unchanged G2 fixture and cleanup helper
retain their original hashes.

Controller self-check passes in 0.73s. The new fixture against the accepted
installed `ac635fa` still reproduces genuine missing-stream RED in 11.43s,
with healthy startup, one non-stream request and proven owned cleanup. Against
the already-attested installed candidate `ed727a6`, the full four-request early
delta/tool/second-turn/CLI+daemon restart journey passes in 2.23s, with exact
typed history and both owned groups gone. No rebuild was used for fixture-only
maintenance. Source/test/command/log hashes and both runs are retained at
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-stream-cleanup-reverified-us6dfyl3`.
This is executable test maintenance and candidate evidence; integrated native
product acceptance still requires the exact new release gate.
