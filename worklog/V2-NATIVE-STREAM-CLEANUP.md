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
