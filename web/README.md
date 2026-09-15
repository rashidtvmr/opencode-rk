# OpenCode RK web

Fast local web client for the native Rust singleton daemon.

Stack: React 19, Vite, TypeScript, Tailwind CSS 4, and **shadcn/ui** using its
`aria-nova` React Aria component base. `components.json` is the shadcn registry
configuration and components under `src/components/ui` are generated shadcn
source owned by this app.

For normal use, no Vite server is required. The production bundle is embedded in
the Rust binary and served from the same loopback origin as `/api/*`:

```sh
opencode-rk models sync
OPENAI_API_KEY=... opencode-rk web
```

`opencode-rk web` reuses the healthy backend for the active data directory when one
already exists (for example, once the native TUI is wired to the same daemon). If no
backend exists, the `web` command itself owns the canonical singleton; a separate
`opencode-rk serve` terminal is not required. Pass `--no-open` to suppress launching
the system browser and print/serve the URL only.

For frontend development only:

```sh
pnpm install
opencode-rk web --no-open
pnpm dev
```

The Vite dev server proxies `/health` and `/api/*` to `http://127.0.0.1:4096` by
default. Override that for another native-server port:

```sh
VITE_API_ORIGIN=http://127.0.0.1:4097 pnpm dev
```

The web client uses the native server for health, model discovery,
session create/rename/archive, bounded persisted message history, and OpenAI
Responses turns. The server keeps `OPENAI_API_KEY` on the native side; the web
client only sends the selected `provider/model`, reasoning effort, and message.
Sync the model catalog before starting a fresh daemon so the model picker is
populated:

```sh
opencode-rk models sync
OPENAI_API_KEY=... opencode-rk web
```

`pnpm build` writes the release bundle to `crates/server/web_dist`; that directory is
embedded into the native server binary at Rust build time.

OpenAI turns stream incrementally through
`POST /api/sessions/{id}/turns/stream`. The native server consumes real Responses
API SSE events and exposes bounded NDJSON to the browser; the UI renders real text
deltas as they arrive and reconciles them to the final persisted assistant
message. Dropping the browser response also drops the native upstream request;
there is no detached background stream producer. The completed JSON `/turns`
endpoint remains available, and older daemons that do not expose the streaming
route fall back to it (then to the durable message-only endpoint on older
message-only daemons).

Attachments, screenshots, voice input, and native turn adapters for providers
other than OpenAI remain separate runtime work and are not simulated by the UI.

Accessibility is treated as an application contract rather than assumed from the
component library: the shell includes semantic landmarks, skip navigation,
visible keyboard focus, labeled controls, reduced-motion handling, responsive
reflow, and an automated axe smoke test. Manual keyboard, zoom, and screen-reader
verification is still required before claiming formal WCAG 2.2 AA conformance.

Validation:

```sh
pnpm test
pnpm typecheck
pnpm build
```
