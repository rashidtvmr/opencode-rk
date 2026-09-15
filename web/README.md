# OpenCode RK web

Fast local web client for the native Rust server.

Stack: React 19, Vite, TypeScript, Tailwind CSS 4, and **shadcn/ui** using its
`aria-nova` React Aria component base. `components.json` is the shadcn registry
configuration and components under `src/components/ui` are generated shadcn
source owned by this app.

```sh
pnpm install
pnpm dev
```

The dev server proxies `/health` and `/api/*` to `http://127.0.0.1:4096` by
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
OPENAI_API_KEY=... opencode-rk serve
```

Turn responses are currently returned after the provider request completes.
Incremental streaming, attachments, screenshots, voice input, and native turn
adapters for providers other than OpenAI remain separate runtime work and are not
simulated by the UI. Older daemons that do not expose `/turns` retain the durable
message-only fallback.

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
