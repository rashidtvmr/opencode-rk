# V2-WEB-REAL-BROWSER-FIXTURE

Source-ready G6 fixture, not PREVERIFIED or ACCEPTED. Base:
`7e264cead587203d6eb464655911e953fa28d8dc`.

Run:

```sh
python3 tests/e2e/web_tool_journey_fixture.py --binary /path/to/oc2 \
  --artifact-dir /path/to/artifacts --build-json /path/to/artifacts/build.json
```

`build.json` must contain `source_sha` and `binary_sha256` (camelCase aliases
are accepted). The fixture hashes the installed binary, creates disposable
HOME/XDG/data/project under the short `/var/folders/.../T/pp` root, writes a
0600 synthetic `.codex/auth.json`, binds the provider only on loopback, and
records `fixture-metadata.json`. The parent opens `origin` in a real browser
using `launch_fragment`; no browser success is claimed here.

Provider evidence is bounded (8 requests, 512 KiB) and requires the exact
`fixture-key`. It emits the current Responses SSE
`response.output_item.done`, `response.output_text.delta`, and
`response.completed` shapes. Continuation requests must contain exactly one
`function_call` and one `function_call_output` for `g6_write_1`, with
`write success`; the write path is the fixture project marker.

Create `control/restart.request` to restart only the owned daemon while
retaining fixture data; create `control/stop.request` to end the run.

Source evidence: `crates/server/src/lib.rs:create_turn_stream`,
`crates/providers/src/responses.rs:ResponsesItem` and
`ResponsesStreamParser`, plus
`crates/storage/tests/typed_history_http_restart.rs`. G6 is defined at
`docs/CONVERGENCE.md:29`. Only Python syntax checking is authorized now;
Cargo, daemon, browser, and provider execution remain parent-owned blockers.
