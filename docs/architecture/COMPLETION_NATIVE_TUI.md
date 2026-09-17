# Completion ADR: native application and owned OpenTUI bridge

Status: selected approach, subject to TUI-001 build/ABI/measurement acceptance.
This does not claim an implemented renderer or benchmark result.

## Decision

Retain the owned OpenTUI Zig renderer and build a narrow Rust C-ABI wrapper. Do not
start with a wholesale Rust port. Rust/Tokio owns application services, UI state,
input/event scheduling and native presentation orchestration. No Node/Bun/TS
runtime starts in native mode. An installed user receives native artifacts, not
a requirement to compile Zig, install npm packages or run a development server.

Evidence: `rashidtvmr/opentui@c01292fd0837bafd07ce458c74416b2b375a41ab`,
`packages/native/src/lib.zig` exports handle-based native functions and extern
layouts; `packages/native/build.zig` defines the native build. At this fork the
native tree is `packages/native`, not `packages/core/src/zig`.
`packages/core/src/renderer.ts` includes input parsing, renderables, key handling,
selection, palette/capability handling and scheduling. These are real behaviors to
implement in Rust, not just a TypeScript bridge to delete.

## Boundaries and safety

Isolate unsafe FFI in an audited `opentui-sys` boundary and expose safe RAII handles
with explicit ownership, allocator/free pairing, ABI/version checks, error codes,
validated pointer/length arguments and callback lifetime rules. Domain crates keep
`forbid(unsafe_code)`. One renderer owner thread/task consumes bounded/coalesced
input and application events. Blocking terminal work must not stall Tokio.
Validate grapheme/width behavior, paste/input protocols, resize, cleanup and every
supported terminal/platform. Panic/exit policy must restore the terminal through
actual supported mechanisms; do not assume Drop runs under panic=abort.

Audit native dependency closure before excluding audio/image/embedded-terminal or
other functionality. Retain anything required by the selected product features;
feature-disabled code must not start hidden runtimes. Publish fork commit, Zig
version, flags, SBOM and licenses. Ship deterministic native libraries with correct
loader paths or statically link where supported. An ABI smoke test is not the
complete TUI; TUI-003..010 supply actual application behavior.

Arbitrary Solid/TS presentation plugins are a separate optional compatibility
frontend, not something a native Rust renderer can silently execute. Preserve the
existing plugin requirement with an honest capability/overhead distinction.

## Installed app lifecycle

`opencode2` with no subcommand discovers/starts one authenticated daemon per OS
user/data directory and opens the native TUI. `serve`, `web`, headless run/export
and service controls remain explicit modes; none are manual prerequisites for
the default journey. Two UI processes are allowed; they share one Rust/Tokio
backend/store owner. This is one installed product, not necessarily one OS process.

Authenticate/validate daemon descriptors and version negotiation, handle launch
races and stale/reused PIDs safely, and keep identity scoped to the data directory.
Do not kill a process based only on a recorded PID. First-run project/session
creation and provider consent happen inside the UI. CLI/TUI/web/mobile invoke one
execution service, permission broker, durable store and event/replay contract.

The upstream baseline default handler obtains `Daemon.transport()` then launches
`runTui`; source: `anomalyco/opencode@95daf90670b7c039c436c85537da5fbfe2205b41`,
`packages/cli/src/commands/handlers/default.ts` and `packages/cli/src/services/daemon.ts`.
Mirror the user-visible lifecycle, not unsafe or language-specific details blindly.

A Rust-only renderer remains an alternative only after a comparison ADR with the
same terminal/Unicode/plugin/packaging tests and measured whole-app costs. Language
uniformity alone is not a reason to rewrite a working rendering engine.
