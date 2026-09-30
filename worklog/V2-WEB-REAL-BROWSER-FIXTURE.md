# V2-WEB-REAL-BROWSER-FIXTURE

Candidate G6 fixture, not PREVERIFIED or ACCEPTED. Repair base:
`ebd319b86963ede570957066a7de69103b6d5581`.

Run:

```sh
python3 tests/e2e/web_tool_journey_fixture.py --binary /path/to/oc2 \
  --artifact-dir /path/to/artifacts --build-json /path/to/artifacts/build.json
```

`build.json` must contain a 40-hex source SHA and matching `binary_sha256`
(camelCase aliases are accepted). The current parent artifact is expected to
identify source `f5cfb012369f3a4187cfa4f50ad01eb4051081c6` and binary SHA
`1870d7627875ae450d2dcea2691d7b00e2127a30860726efeb2b43cdab434321`.
The fixture hashes the absolute installed binary, creates disposable
HOME/XDG/data/project under the short `/var/folders/.../T/pp` root, writes
`XDG_DATA_HOME/opencode/auth.json` mode 0600, removes ambient API credentials,
binds the provider only on loopback, and records `fixture-metadata.json`.
The metadata fragment uses the client’s exact `#oc2-token=<64 hex>` parser.

Provider evidence is bounded (4 requests, 128 KiB per request, 512 KiB total)
and requires the exact
`fixture-key`. It emits the current Responses SSE
`response.output_item.done`, `response.output_text.delta`, and
`response.completed` shapes. Continuation requests must contain exactly one
`function_call` and one `function_call_output` for `g6_write_1`, with
`write success`; the call path is the relative `g6-browser-marker.txt`, and
the provider verifies the resulting marker bytes, exact user-turn sequence,
and exact single typed pair. Restart waits for a fresh healthy descriptor and
updates origin/token/PID metadata atomically before permitting the resumed
request. `--self-check` exercises the real loopback HTTP framing without a
daemon.

Create `control/restart.request` to restart only the owned daemon while
retaining fixture data; create `control/stop.request` to end the run.

Source evidence: `crates/server/src/lib.rs:create_turn_stream`,
`crates/providers/src/responses.rs:ResponsesItem` and
`ResponsesStreamParser`, plus
`crates/storage/tests/typed_history_http_restart.rs`. G6 is defined at
`docs/CONVERGENCE.md:29`. Only Python syntax checking is authorized now;
Cargo, installed daemon, and browser execution remain parent-owned blockers.
This describes the fixture-owner handoff at `c665fd6`; the independent parent
runtime checks below subsequently exercised the installed product.

## Parent runtime verification

Main repaired provider-history accounting to include settled assistant messages,
added an offline fixture catalog, exact model and typed ordering checks, finite
HTTP framing, and authenticated descriptor/PID readiness. The resulting fixture
passed `--self-check` and launched installed integrated `f5cfb01` with the source
and binary hashes above. Real Playwright browser controls completed:
write tool -> settled history -> second turn -> authenticated reopen -> owned
daemon restart with fresh port/token -> resumed third turn. Four provider requests,
seven unique durable messages, one tool message and two exact typed records were
verified; marker bytes matched. Fixture shutdown/reaping exited 0.

Evidence root:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/g6-integrated-f5cfb01-yt__rzd4`.
Scoped real-browser settled-history/restart behavior is accepted on installed
integrated `f5cfb01`. Interactive approval/denial/interruption and release
profile/platform journeys remain open. See the canonical integration worklog
for transcript hashes, screenshots and acceptance scope.
