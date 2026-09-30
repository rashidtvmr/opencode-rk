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

## Executable onboarding and independent credential-bound review

The installed `7db4a837684e6ca39569526c5860c8e9748dda7e` release completed native
UI-created masked credentials, catalogue-backed non-default model selection,
first response, daemon/CLI restart, restored model and prior prompt, and a second
response. Exactly two real loopback provider requests used `gpt-5.6-mini` and the
persisted key with the exact durable user/assistant input sequence. All seven
credential controls passed. Evidence:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-g2-wire-7db4a83-bhfwl0l5`
and `v2-auth-control-7db4a83-um3pa0ef` under the same approved artifact parent.

Independent test-owner maintenance corrected dirty-cell VT observation and the
valid Responses default when `stream` is absent. The new G2 contract hash is
`32edfa5c0ac5eacef9cfa949e02fce0d3440fe1cd8f75580efb584bc0a7780dc`.
No auth/model/count/history/byte-budget assertions were weakened.

Independent reviewer `ses_f0c3bda7effeqXZjDfJqgOWKVF` found that an update could
rewrite another API credential over the 16 KiB bound. Its eighth control preserves
all seven prior test-method ASTs and is frozen at
`f5b7dd3372b1d0aac4e285d16fb4295833854c8ab9b6ec836bacdb9205d8809d`.
It failed against installed `7db4a83` with HTTP 200 instead of rejection; evidence:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/v2-auth-bound-red-7db4a83-5hwqcztg`.
The minimal corrective writer now rejects an oversized preserved API entry before
any filesystem mutation. It preserves every byte on rejection and allows explicit
replacement of the selected account. Structurally valid empty legacy keys stay
preserved, matching the inspected upstream schema.

The reviewer also investigated concurrent model-recents writes. Pinned upstream
`packages/tui/src/util/persistence.ts:22–33` uses per-client atomic replacement,
with no cross-process recents merge. This package retains that behavior; it does
not establish second-client work ownership or race-proof filesystem protection.
This final product change requires a new exact release and independent gates.

## Exact integrated acceptance

The final candidate `bdee47eac2e31f25a4787944a230a2ec8554e70b` was independently
PREVERIFIED, including all eight auth controls, native G2, four daemon flows,
73 bridge and ten terminal lifecycle checks, and installed interactive PTY.
The verifier's first G2 invocation used the obsolete raw-marker fixture and is
retained as an infrastructure mismatch. The canonical dirty-cell observer
passed against the same installed bytes; no product repair or assertion change
was made to resolve that mismatch.

All six preserved product commits were integrated on `main-v2`, ending at
`ac635fac5298e9151c6575aa6a089f53c5aa4309`. Exact Mac and Ubuntu native releases
were built, archived, installed and independently rerun with the current frozen
G2 and eight auth controls. Both completed two real provider requests across
restart and **8/8 auth controls**; the real browser completed the tool, second
turn and resumed third turn on the same source SHA. Status **ACCEPTED for the
settled onboarding and credential-safety scope on integrated `ac635fa`**.
Full commands, upstream evidence, artifact hashes and remaining failures are
recorded in `worklog/V2-INTEGRATION-20260930.md`. Native streaming is the next
observed gap and is not established by this acceptance.
