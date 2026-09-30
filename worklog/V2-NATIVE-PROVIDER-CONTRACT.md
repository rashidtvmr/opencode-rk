# V2 native provider contract

Contract-only test owner handoff; no product paths were changed. Base revision:
`6c52b419d4e4029f1baf15e4b25f289872cd0bb8`.

## Authority and scope

Pinned OpenCode `95daf90670b7c039c436c85537da5fbfe2205b41` native evidence is
`packages/tui/src/app.tsx:739-745` (`/connect`),
`packages/tui/src/component/dialog-provider.tsx:352-417` (API-key prompt and
`sdk.auth.set({type:"api", key})`, then sync/bootstrap and model dialog),
`packages/tui/src/component/dialog-model.tsx:23-154` (catalogue and
`local.model.set`), and `packages/tui/src/context/local.tsx` (durable model
selection/recents). The test freezes those observable roles while allowing the
native terminal renderer to remain compact.

## Test

`tests/e2e/native_provider_setup.py` owns a bounded stdlib loopback OpenAI
Responses/SSE fixture and a real PTY. It requires `--binary`,
`--native-library`, `--build-json`, and `--artifact-dir`; it validates the
attested source SHA and binary SHA, stages the executable plus adjacent native
library, launches with no subcommand, and sets neither ambient API credentials
nor preseeded auth. The UI must connect OpenAI, enter the generated fixture key,
choose non-default `gpt-5.6-mini`, send two prompts across a restart, and cause
two real requests with exact bearer auth, model, and durable user history.

The test writes no auth/config before onboarding. It only checks the UI-created
`auth.json` schema and mode `0600` afterward. Provider evidence is sanitized and
contains no Authorization header or key.

## Verification status

The fixture supports both valid OpenAI Responses wire formats: streaming SSE
and non-stream JSON. G2 only asserts the settled response, auth/model/history;
streaming parity remains outside this gate. Preparation only; no product
PTY/Cargo/build execution was performed. The
stdlib helper checks cover artifact path/profile and catalog validation, fresh
marker matching including stale-buffer/EOF failure, bounded auth/model/count/
history/SSE protocol, and capture redaction. The helper-only
protocol check is:

```sh
python3 tests/e2e/native_provider_setup.py --self-check --artifact-dir /absolute/ignored
```

It checks wrong-key rejection, third-request rejection, model identity, ordered
history, bounded request handling, and SSE framing without a product binary;
this is not G2 acceptance. Suggested actual command
for the parent after an attested release build:

```sh
python3 tests/e2e/native_provider_setup.py \
  --binary /absolute/path/to/oc2 \
  --native-library /absolute/path/to/libopentui.dylib \
  --build-json /absolute/path/to/build.json \
  --artifact-dir /private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/g2-native
```

No acceptance claim is made. A current product failure should be recorded as a
semantic G2 RED only after a healthy owned daemon publishes its authenticated
descriptor, `/api/models` proves both fixture models, the frame reaches
`/connect`, and the missing native provider/model binding is observed—not as a
timeout-only failure. Failure artifacts retain sanitized result/PTTY evidence
and preserve the disposable fixture root; successful runs remove it only after
bounded fixture shutdown.

## Controller pre-freeze maintenance

Before the product RED run, the independent integration controller corrected a
descriptor return-key mismatch in cleanup, exact catalogue-ID readiness checks,
duplicate/stale helper input, and ensured capture-overflow failure occurs after
owned process/FD cleanup. The controller also records the current UI phase and
preserved fixture root. The contract has not yet been frozen; provider/auth/model,
no-key-echo, request-count and durable-history assertions remain mandatory.

## Terminal observer maintenance

The native renderer legitimately emits VT100 dirty-cell updates rather than
repeating full frames. The contract now reconstructs an 80x24 visible screen,
with bounded CSI cursor/erase/save-restore handling, OSC/DCS/APC skip, and
incremental UTF-8 decoding. The model assertion is the current status
projection `model: openai/gpt-5.6-mini`, not an arbitrary stale picker/filter
occurrence. Pinned native roles remain `Connect a provider`, `API key`, and the
catalogue's advertised non-default model. This is mechanical observer
maintenance only; auth schema/mode, exact bearer, non-default payload model,
two-request limit, prompt/assistant history, and key non-echo assertions are
unchanged.

The observer now drives the real `read_until(..., screen, predicate)` path in
helper checks and validates the preserved sanitized 7db4a837 provider result
capture shape before any product run. It handles fragmented UTF-8, private VT
queries, split-key visibility, stale picker text, and current status projection
without changing the provider/request/history contract.

Protocol maintenance: the Rust daemon's `create_with_items` omits `stream` for
its normal settled Responses request (`crates/providers/src/responses.rs`,
`responses_request_payload`, lines 189–203). The fixture now treats an absent or explicit
`stream: false` as valid non-stream JSON, `stream: true` as SSE, and rejects
non-boolean values. This corrects the fixture codec without relaxing auth,
model, request-count, prompt-history, or settled-response assertions.

## Executable RED and freeze

The controller independently ran the actual installed Mac native release from
`85c2547f20cfcb34014d80f5ae72fc25702659bc`. A first attempt exposed only a fixture
Unix-socket path-length failure (0.18s); its reproduction is preserved at
`v2-g2-native-red-85c2547-ykkjvll1` below the approved temporary evidence root.
The fixture prefix was mechanically shortened; the 100-byte path bound is intact.

The corrected run reached a healthy authenticated owned daemon and exact fixture
catalogue IDs, painted the live native frame, and rendered the entered `/connect`.
The product treated that command as an ordinary turn and rendered HTTP 503 for
`/api/sessions/...`; it never opened the connection dialog or created `auth.json`.
The provider received zero requests. This is a genuine native command/onboarding
RED, not a loader, readiness or fixture-source failure.

```text
/usr/bin/arch -arm64 /usr/bin/python3 tests/e2e/native_provider_setup.py --binary <release-root>/install/bin/oc2 --native-library <release-root>/install/lib/libopentui.dylib --build-json <release-root>/build.json --artifact-dir <evidence-root>
```

`TMPDIR=/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/pp`.
Release root:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-release-macos-85c2547-rd2cct9_`.
Evidence root:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-g2-native-red-85c2547-opjif3xe`.
Result: exit 1 in 20.77s, `PTY timeout before fresh marker b'Connect a provider'`.
`command.json` retains the exact command/environment/source/test/log hashes;
`native-provider-result.json` retains the sanitized 30,459-byte PTY capture and
owned descriptor provenance. `red-classification.json` confirms the live frame,
HTTP error, absent auth, zero provider requests and no surviving owned group.
The disposable failure HOME/project/data remain preserved at the recorded path.

Frozen test SHA-256:
`752c97c5d1eee013e4d350e62428be5938987b5bb3491967122f7fa00b29b3ff`.
The implementation lease cannot edit this file or weaken its assertions.

## Current observer/wire freeze and executable candidate proof

Independent owner commits `dc55fdc`, `51d5d6e`, and `c377a96` are preserved and
integrated as `8047ade`, `0e5d1f2`, and `3ae8654`. They correct VT dirty-cell
observation and default non-stream Responses framing. The old hash above remains
historical RED evidence; the current contract SHA-256 is
`32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc`.
The exact bearer, selected non-default model, two real requests, ordered durable
user/assistant history, auth `0600`, masked key and 256 KiB capture bounds remain
mandatory. Helper State checks establish codec construction, not an HTTP run.

The controller fed the retained real dirty-cell output from installed `7db4a83`
through the observer in 257-byte chunks; it reconstructed the current selected
model status with the picker closed and zero pending escape bytes. The actual
installed provider journey then passed in 2.24s with exactly two real requests
using the same `gpt-5.6-mini`, prior assistant and user history across daemon/CLI
restart. Manifest, sanitized capture and provider input evidence:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-g2-wire-7db4a83-bhfwl0l5`.
This is candidate evidence; exact integrated release verification remains required.
