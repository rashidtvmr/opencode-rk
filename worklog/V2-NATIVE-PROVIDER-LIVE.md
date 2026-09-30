# G2 native provider live candidate

Base: `710a410e0ce32c716c40217696b515caa6c52310`.

This candidate uses the pinned upstream auth shape (`type: api`, `key`), the
existing Rust `OpenAiResponsesClient::from_persisted_env`, authenticated
daemon middleware, and the durable `/api/sessions/{id}/turns` engine.

The native `/connect` flow sends the provider key through an authenticated
daemon route, persists bounded auth JSON asynchronously with atomic replacement
and mode `0600`, preserves other entries, and uses the selected model in the
real authenticated turn request. No key is rendered or logged.

The pinned legacy-compatible provider auth route is `PUT /auth/{provider}`,
protected by daemon bearer middleware. It accepts the API payload (`type`,
`key`, optional string metadata), normalizes trailing provider slashes, and
returns exactly JSON `true`. Credential and daemon-descriptor temporary files
are created owner-only before writing and atomically replaced.

## Candidate-local validation

The focused native daemon-flow command completed with 4/4 GREEN. The frozen G2
provider fixture has not yet been run on a committed release candidate in this
handoff; parent must independently verify the exact candidate SHA with the
immutable fixture.

Known open limits: provider streaming remains a separate G5 concern; native
input remains byte-oriented without full UTF-8/escape/resize/mouse parity.

## Controller completion of the candidate

Earlier commits `8d974c06`, `0cbaf618`, and `c6a29470` are preserved. The first
two were incomplete source handoffs; four unchanged daemon-flow checks did not
establish onboarding acceptance. After the source leases ended, the controller
completed catalogue-backed provider/model filtering, masked key input,
credential response validation, restart model recents and bounded native history.
It also corrected metadata/schema validation and atomic-file failure cleanup.

Authority: pinned OpenCode `95daf906`, `packages/tui/src/app.tsx:739–745`,
`component/dialog-provider.tsx:228–230,352–417`,
`component/dialog-model.tsx:136–154`, `context/local.tsx:164–234,320–337`,
`packages/core/src/global.ts:10–14`, and
`packages/opencode/src/auth/index.ts:14–35,58–81`. Native model selection stores
the inspected `recent`/`favorite`/`variant` model.json format under the XDG state
root. The daemon owns API credentials and real durable turn execution.

Frozen required gates: installed native setup hash
`752c97c5d1eee013e4d350e62428be5938987b5bb3491967122f7fa00b29b3ff`,
seven-control installed credential contract hash
`1f08cd2968e4867f7eed26efe2c4534ea005bac1fa9f961c862ce31aa8b4f419`.
Release build/runtime evidence remains pending for this exact candidate. The
controller's product changes require independent verification before acceptance.

The exact installed release candidate `aedc2894397dafcc101cd9249395d9743c840a89`
reached the real provider/key/model setup, persisted auth and accepted the model.
The frozen PTY then failed its 256 KiB capture bound before the first response:
forcing every renderer frame emitted the full terminal for every input byte.
Evidence is retained at
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-native-provider-aedc289-yeclxljv`.
The corrective bridge change uses the pinned persistent renderer's normal dirty
cell presentation (`packages/native/src/renderer.zig:932–970,2372+`). The native
renderer already owns initial/resize/restoration full repaints. The frozen byte
budget and assertions remain unchanged; another exact-candidate build/run is
required after this product change.
