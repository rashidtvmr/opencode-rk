# Completion ADR: own account, named tunnel and native phone control

Status: required design/tasks; no production deployment or mobile app is claimed.
Scope of 'control the PC': authorized agent sessions, workspaces, tools, approvals,
files/diffs and opt-in constrained PTY. Full desktop screen/RDP/mouse mirroring is
not implied; it would need a separate explicit permission/media product contract.

## Topology

Phone apps -> HTTPS/WSS -> owned domain/Cloudflare -> named Tunnel -> owned account
and control gateway. A paired PC initiates an authenticated outbound WSS connection
to that gateway. The PC daemon remains local and does not need inbound ports,
manual router forwarding or a separate Cloudflare account/tunnel for every user.
`cloudflared` is an operator-side deployment dependency, not the native TUI runtime.

Local use is offline/account-free by default. `pair`/`connect` explicitly opts in.
One account spans iOS, Android and multiple PCs. Provider credentials and project
execution stay on the PC by default; do not silently upload complete histories.
The gateway stores account/device metadata and only bounded relay/replay data
required by the selected contract, with explicit retention and audit policy.

Cloudflare Tunnel is transport, not application authorization. Every session,
subscription and action is scoped to account/device/workspace/session/operation.
Cloudflare normally terminates edge TLS; do not call this design end-to-end
encrypted against the gateway/edge. Application-layer encryption would be a
separate reviewed protocol and key-management task, not a marketing claim.

## Identity and pairing

Use standards-based OIDC/PKCE or an audited identity service, system browser auth,
verified account/recovery/MFA/session lifecycle and bounded abuse controls. Never
roll ad-hoc cryptography. Store iOS tokens in Keychain and Android credentials with
Keystore-backed protection. Pairing uses a short-lived single-use account-bound
challenge, visible device fingerprint and explicit local human approval. A QR code
must not contain provider secrets or long-lived tokens. Deep/app links validate
state, origin and account. Revocation closes active channels and invalidates queued
operations; reauthorize at execution time with policy/capability epochs.

## Stateful control and mobile UX

Use versioned envelopes with request IDs, device/workspace/session IDs, event
cursors and operation digests. Define idempotency per command; do not promise
universal exactly-once external tool effects. Expired replay cursors require bounded
snapshot/resync. Slow clients have bounded queues; no implicit infinite buffering.
Backgrounded/killed phone apps reconnect, not remain magically active forever.
Offline drafts may persist locally; stale privileged operations must not auto-replay.

Native iOS/Android clients require installable builds, secure login, PC/project
lists, independent session tabs/drafts, streamed Markdown/code/tools, model/effort/
agent selection, interruption, scoped approval details, file/diff/artifact and
explicit PTY controls. Use thin platform UI over the shared versioned Rust-service
protocol; AUD-016 freezes the mobile framework/build/test choice before fan-out.
PWA/browser tests cannot replace installed-device evidence. Include accessibility,
font scaling, touch/keyboard layout, process recreation, battery/network budgets
and privacy-safe optional APNs/FCM notifications.

Remote approval is not equivalent to unrestricted trust. Bind grants to exact
operation and scope; local-only/human-only restrictions remain mandatory. PTY and
file services inherit the same OS sandbox, environment and output restrictions as
local tool execution. Account login alone cannot grant a shell or other-workspace
access. Harden IDOR, CSRF/Origin/CORS, SSRF, DNS rebinding, uploads, replay and floods.

## Deployment and verification

Use a named production/staging tunnel with deny catch-all, loopback/private origin,
verified TLS, least-privilege secrets, protected admin/metrics and tested WebSocket
and event streaming. Quick Tunnels are development/testing only and do not support
SSE; they are not the release target. Test actual configured transports and edge
behavior rather than assuming localhost success means deployed success.

Operator setup, backup/key restore, quota/retention, version compatibility,
rolling upgrades, revocation propagation and diagnostics have dedicated NET tasks.
Real authorized staging credentials and iOS/Android signing/device access are
prerequisites for final evidence; missing access means blocked, not simulated pass.

Official references checked September 17, 2026:
- https://developers.cloudflare.com/tunnel/
- https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/
- https://developers.cloudflare.com/cloudflare-one/networks/connectors/cloudflare-tunnel/do-more-with-tunnels/trycloudflare/

The topology and authorization choices above are project design decisions, not
claims that Cloudflare supplies the account, mobile UI or agent permission model.
