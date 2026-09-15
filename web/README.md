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

The web client currently uses the native server for health, model discovery,
session create/rename/archive, and bounded persisted message history. Sending a
message stores the user message in the native session database. Agent/model turn
execution, streaming assistant responses, attachments, screenshots, and voice
input still require native runtime endpoints and are not simulated by the UI.

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
