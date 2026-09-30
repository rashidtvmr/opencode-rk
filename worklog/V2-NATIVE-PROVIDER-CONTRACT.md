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

Preparation only; no product PTY/Cargo/build execution was performed. The
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
