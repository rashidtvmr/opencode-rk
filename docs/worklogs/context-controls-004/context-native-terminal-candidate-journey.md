# Context inspector — disposable Terminal.app candidate journey

Base: `e3069eee04d9a3414517b2452df803c9c145c9d3`, with the uncommitted context
candidate. **Debug-candidate observation only; not final installed release
acceptance.** Binary and source fingerprints are retained in
`context-inspector-terminal-fixture.json`. The final-candidate fingerprint
includes the strict selection-authority and record-preview corrections.

## Observed behavior

1. Launched the bundled native candidate in a dedicated Terminal.app window,
   using disposable HOME/config/data/workspace and a loopback-only fake provider
   URL. Created two short durable fixture messages through authenticated HTTP.
2. Opened `/context` through the native composer. The page displayed three
   entries: both durable messages and the current AGENTS.md instruction source.
   It showed roles, inclusion, source, bytes/estimated tokens, limits, projection
   provenance, full scrollable identity/content, and key hints.
3. Pressed Enter on `INSPECT_REMOVE`. The UI changed to excluded only after the
   backend acknowledgement and refresh. The selected typed projection changed
   from two items / 201 bytes to one item / 96 bytes.
4. Independently queried the disposable daemon: that message was excluded and
   both original durable messages still existed. Navigated to the memory entry
   and pressed Enter; it remained included and directed instruction-selection
   controls to `/memory`.
5. Exited with Ctrl-D, observed shell restoration, and gracefully stopped only
   the verified disposable daemon. Relaunched the final reviewed-source debug
   candidate with the same data directory and exact session ID. This reopened
   the database in a new daemon process, rather than reusing an old daemon.
6. Reopened `/context`: the original exclusion remained visible and the selected
   projection still contained one item. Authenticated inspection independently
   confirmed persistence after the daemon restart.
7. Pressed Enter to include the item again. Both UI and API acknowledged it;
   the selected projection returned to two items / 201 bytes. Durable history
   still contained both messages. Exited cleanly and stopped the fixture daemon.

Session: `826e73d7-998e-41ca-ad5c-37d00c402103`.

## Retained evidence

Root:
`/private/var/folders/b0/dj81nc_j2yq2bkmg0yd2sgyc0000gn/T/opencode/context-inspector-terminal-sutbylsz/`

- `initial-context.json`
- `after-native-exclusion.json`, `after-native-exclusion-terminal.txt`,
  `after-native-exclusion.png`
- `after-daemon-restart.json`, `after-daemon-restart-terminal.txt`,
  `after-daemon-restart.png`
- `after-native-include.json`, `after-native-include-terminal.txt`,
  `after-native-include.png`
- `terminal-restored.png`

The manual journey exercises native controls, authenticated state changes,
database reopening, and history preservation. Separate real HTTP-provider
regressions verify outbound payload differences, atomic tool pairs, fallback
summary exclusion, native opaque checkpoint selection, and checkpoint replacement.
No real provider request or user database was used for this visual journey.
