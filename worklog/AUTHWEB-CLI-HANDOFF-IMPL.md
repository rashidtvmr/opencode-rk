# AUTHWEB-CLI-HANDOFF-IMPL

## Claim and scope

- Task: `AUTHWEB-CLI-HANDOFF-IMPL`; session `ses_f1ea90f38ffeOBYPTdwG0FAKvP`.
- Owned product file: `crates/cli/src/main.rs` only. Frozen test remains unchanged.
- Base: `ecec045`; frozen test SHA: `3c8eea31b80924c4f3f0cb912c2ae03234bd8077a9f32bc55d533e23bd4c9767`.

## Source evidence

- At base, `web` lines 668-674, `serve` lines 697-710 and 742-747 all passed only `descriptor.http_origin` to `open_web_browser`.
- `open_web_browser` lines 761-789 used platform-specific `Command` argv, but did not include the descriptor credential.
- `BackendDescriptor` in `crates/server/src/daemon.rs:118-127` carries `auth_token`; the server validates it as a 64-hex bearer credential.
- Frozen `tests/e2e/browser_credential_launch.rs` requires exact `<origin>#oc2-token=<64hex>`, bare origin on stdout, and no token in stdout/stderr.

## Contract and security

Every browser-opening path supplies an in-memory fragment URL. The fragment is same-origin and uses the exact token from the authenticated descriptor. Standard output remains `descriptor.http_origin`; `--no-open` does not call the launcher. The URL is passed as one argv value (`Command::arg`/`args`) without shell-string concatenation. No credential is logged. Allocation is bounded by the descriptor's existing validated fields.

## Implementation

Changed all three browser call sites to pass `descriptor.auth_token`; `open_web_browser` builds the fragment URL and passes it safely on Windows, macOS, and Unix.

## Tests and remaining verification

- Convergence gate was run and is blocked by pre-existing ledger/off-plan findings; no controller files changed.
- Frozen test source hash verified unchanged.

## Validation receipt

- `rtk rustc --edition=2021 --test tests/e2e/browser_credential_launch.rs -o /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/browser_credential_launch_red` — compiled; SHA remained `3c8eea31b80924c4f3f0cb912c2ae03234bd8077a9f32bc55d533e23bd4c9767`.
- `rtk env CARGO_BUILD_JOBS=1 cargo build -p opencode-rk-cli --bin oc2` — GREEN.
- `rtk env OC2_BIN=target/debug/oc2 /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/browser_credential_launch_red --exact web_command_delivers_token_fragment_to_browser` — GREEN (1 passed).
- First build invocation used invalid package `oc2`; corrected to actual package `opencode-rk-cli`; no product impact.
