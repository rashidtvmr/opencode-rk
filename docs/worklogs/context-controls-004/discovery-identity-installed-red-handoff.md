# Discovery identity boundary — installed probe

The fixed test launches the actual bundled `OC2_E2E_BIN` (falling back to
Cargo's binary), sets `OPENCODE_RK_DAEMON_ADDR` to the foreign listener, and
holds stdin open until a real `/health` request is observed. It clears the
environment, uses disposable HOME/config/workspace paths, drains output into
fixture files, bounds each socket operation to 200ms/8KiB, and guards every
child/thread for panic cleanup. The listener returns `403` for API paths and
records every bearer header.

This is an executable security diagnosis, not a production fix. It intentionally
does not alter the existing observer correction or claim acceptance. The test
is RED only when the parent runs it against the bundled native artifact.
