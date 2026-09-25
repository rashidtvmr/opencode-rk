# AUTHWEB-RED scratchpad

Claim: AUTHWEB-RED, session ses_f3c4de578ffelQv59xDXmOs03B.

Base: `7262682e31c1f473912c9483804c9430eb4ee4c5` on
`red/PHASE1-AUTHENTICATED-WEB`.

Source evidence:

- Current router tests, `crates/server/tests/daemon_auth_api.rs:41-107`, already
  prove missing bearer returns 401, wrong bearer returns 403, the published
  bearer permits `/api/sessions`, and `/health` remains public.
- Current middleware, `crates/server/src/daemon_auth.rs:150-175`, gates only
  `/api/*`; embedded web assets remain public.
- Current descriptor, `crates/server/src/daemon.rs:107-178`, publishes and
  validates a 64-hex bearer for native clients. It provides no browser bootstrap
  URL, cookie exchange, one-time credential, or restart-renewal protocol.
- Current CLI, `crates/cli/src/main.rs:625-643`, opens only
  `descriptor.http_origin`; it does not convey browser authority.
- Current browser client, `web/src/lib/api.ts` (for example `:172`, `:580`,
  `:604`, and `:1003`), calls same-origin `fetch` without an Authorization
  header or another credential bootstrap mechanism.
- Product map `worklog/PHASE1-VERTICAL-PRODUCT-MACOS-MAP.md:36-38` classifies
  middleware and embedded assets as existing but says no authenticated browser
  journey or token discovery/renewal has been demonstrated.
- Synthesis assigns this RED to
  `crates/server/tests/phase1_authenticated_web.rs`, implementation only to
  `crates/server/src/daemon.rs`, and later wiring only to
  `crates/server/src/lib.rs`.

Status: BLOCKED before test authoring. Existing public seams can only reproduce
already-GREEN router authentication with a test-supplied bearer. They cannot
model how an untrusted browser obtains scoped authority, because no browser
bootstrap contract exists. Inventing a query token, cookie name, fragment,
one-time exchange endpoint, or descriptor method would make the test define a
security protocol. A server-only implementation also cannot make the existing
browser `fetch` callers send the credential; the synthesis has no `web/src`
implementation/wiring owner for this stage.

Required authority action: choose and threat-model the browser bootstrap and
restart-renewal protocol, including credential lifetime, origin binding,
history/referrer leakage prevention, CSRF behavior, replay policy, and browser
storage prohibition/allowance. Prewire an implementation-independent compiling
seam and grant ownership for the real web API caller. The RED must then prove a
real embedded-bundle request succeeds through that mechanism while absent,
wrong, stale, and cross-origin authority fail without leaking the bearer.

No test hash was frozen. No product, test, web, manifest, controller, or verifier
file was changed. Existing router auth remains valid but is not browser-journey
evidence. This blocker keeps `V1-FREEZE-RED` open.
