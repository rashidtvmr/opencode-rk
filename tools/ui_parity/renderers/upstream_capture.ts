// This file is executed with `bun --eval <source>` from the pinned upstream
// workspace's `packages/tui/` directory by
// tools/ui_parity/renderers/upstream_opentui.py (no staging write / copy: the
// exact source bytes travel as an argv element), so bare `@opentui/*` imports
// resolve against that package's node_modules and the fixture import is
// relative to packages/tui/.
//
// Evidence classes:
//  - `production_flow`: the real pinned upstream app mounted through its own
//    `createAppFixture` (packages/tui/test/fixture/app.ts -> src/app.tsx),
//    driven through the real input path (`mockInput` -> production keymap) and
//    captured per stage of the scenario. Frames are the actual renderer output.
//  - `synthetic`: explicit diagnostic mounts (TextRenderable / empty renderer)
//    that must never be counted as production evidence.
//  - `app_first_frame`: the single-frame legacy default, kept for capture02
//    compatibility and explicitly NOT a fixture-driven production capture.
//
// Expected identities arrive ONLY through the request provenance envelope;
// nothing here hardcodes pins or hashes, and any measured mismatch fails
// closed before a single frame is produced.
import { createTestRenderer, type TestRendererSetup } from "@opentui/core/testing"
import { TextRenderable, ansi256IndexToRgb } from "@opentui/core"
import { realpathSync } from "node:fs"

const ATTRS = ["bold", "dim", "italic", "underline", "blink", "reverse", "hidden", "strike"]
const WIDTH_RE = /[\u1100-\u115f\u2329\u232a\u2e80-\u303e\u3040-\ua4cf\uac00-\ud7a3\uf900-\ufaff\ufe10-\ufe19\ufe30-\ufe6f\uff00-\uff60\uffe0-\uffe6]/
const width = (c: string) => c.length === 0 ? 0 : [...c].some(x => /\p{Mark}/u.test(x)) ? 0 : WIDTH_RE.test(c) ? 2 : 1

function color(value: any): string {
  let [r, g, b] = value.intent === "indexed" ? ansi256IndexToRgb(value.slot) : value.toInts()
  // Default colors intentionally become concrete RGB: the screen protocol is
  // compared against a terminal cell, not against OpenTUI's internal metadata.
  return `#${[r, g, b].map((x: number) => x.toString(16).padStart(2, "0")).join("")}`
}

export function normalizeFrame(frame: any, fixtureId: string, theme: any = undefined) {
  const rows = frame.lines.map((line: any) => {
    const cells: any[] = []
    for (const span of line.spans ?? []) {
      const attrs = ATTRS.filter((_, i) => (span.attributes & (1 << i)) !== 0)
      for (const ch of [...span.text]) {
        const w = width(ch)
        if (!w) { if (cells.length) cells[cells.length - 1].ch += ch; continue }
        cells.push({ ch, fg: color(span.fg), bg: color(span.bg), attrs, cont: false })
        if (w === 2) cells.push({ ch: "", fg: color(span.fg), bg: color(span.bg), attrs, cont: true })
      }
      // OpenTUI reports width in cells; an empty span is still useful for
      // preserving styled blank cells.
      while (cells.length < span.width && span.text.length === 0)
        cells.push({ ch: " ", fg: color(span.fg), bg: color(span.bg), attrs, cont: false })
    }
    return cells
  })
  const cols = Number(frame.cols), rowCount = Number(frame.rows)
  const defaults = { fg: "#ffffff", bg: "#000000" }
  const packed = rows.slice(0, rowCount).map((cells: any[]) => {
    while (cells.length < cols) cells.push({ ch: " ", ...defaults, attrs: [], cont: false })
    return { text: cells.slice(0, cols).map(c => c.ch).join(""), w: cols, cells: cells.slice(0, cols) }
  })
  // Keep spans as the transport representation, while retaining explicit
  // continuation cells in the private `cells` field for the normalizer test.
  const outputRows = packed.map(row => {
    const spans: any[] = []
    row.cells.forEach((c: any, i: number) => {
      const prev = spans.at(-1)
      if (prev && prev.fg === c.fg && prev.bg === c.bg && JSON.stringify(prev.attrs) === JSON.stringify(c.attrs)) prev.n++
      else spans.push({ c: i, n: 1, fg: c.fg, bg: c.bg, attrs: c.attrs })
    })
    return { text: row.text, w: cols, spans }
  })
  return { schema: "oc2.ui-parity.screen/v1", fixture_id: fixtureId, viewport: { cols, rows: rowCount },
    default_style: defaults, rows: outputRows, cursor: { col: frame.cursor?.[0] ?? 0, row: frame.cursor?.[1] ?? 0, visible: true },
    focus: null, hit_targets: [], actions: [], theme }
}

function fail(code: string, detail: unknown, status = 2): never {
  console.error(JSON.stringify({ error: code, detail }))
  process.exit(status)
}

async function sha256File(path: string): Promise<string> {
  const buffer = new Uint8Array(await Bun.file(path).arrayBuffer())
  const hasher = new Bun.CryptoHasher("sha256")
  hasher.update(buffer)
  return hasher.digest("hex")
}

// Verify the loaded production files against the request provenance expected
// identities (fail closed before any capture). Returns the measured identity.
// F2: the files map must be a non-null plain object with EXACTLY the required
// production relpaths (missing/null/primitive/empty/extra keys are denied) and
// every digest must be 64-hex BEFORE any file I/O or hashing. F4: only the
// listed files are authenticity-checked; the pinned checkout itself and all
// transitive node_modules dependencies remain external trust boundaries
// (dependency closure is NOT verified by these hashes) — declared in output.
const REQUIRED_FILES = [
  "src/app.tsx",
  "src/routes/home.tsx",
  "src/component/command-palette.tsx",
  "src/ui/dialog.tsx",
  "src/ui/dialog-select.tsx",
  "src/config/keybind.ts",
  "src/context/keymap.tsx",
  "test/fixture/app.ts",
]

async function verifyIdentity(request: any) {
  // N5: pins are enforced by the wrapper (upstream_opentui.py) before launch;
  // a directly-run staged adapter only requires their presence here.
  if (!request?.pins || typeof request.pins !== "object") fail("missing_pins", "request.pins required")
  const expected = request?.provenance?.expected
  if (!expected || typeof expected !== "object") fail("missing_provenance", "production request requires provenance.expected")
  const files = expected.files
  if (files === null || typeof files !== "object" || Array.isArray(files)) {
    fail("invalid_provenance_files", "provenance.expected.files must be a plain object")
  }
  const keys = Object.keys(files)
  if (keys.length === 0) fail("empty_provenance_files", "provenance.expected.files is empty")
  const required = [...REQUIRED_FILES].sort()
  const present = [...keys].sort()
  if (present.length !== required.length || present.some((key, index) => key !== required[index])) {
    fail("missing_production_file_set", { missing: required.filter(k => !present.includes(k)), unexpected: present.filter(k => !required.includes(k)) })
  }
  const verified: Record<string, string> = {}
  for (const rel of keys) {
    if (typeof rel !== "string" || !rel || rel.startsWith("/") || rel.includes("..") || rel.includes("\\")) {
      fail("provenance_path_rejected", rel)
    }
    const digest = files[rel]
    // F2: digest shape is validated BEFORE any file I/O or hashing.
    if (typeof digest !== "string" || !/^[0-9a-f]{64}$/.test(digest)) {
      fail("invalid_provenance_digest", { file: rel, digest })
    }
    const path = `${import.meta.dir}/${rel}`
    if (!(await Bun.file(path).exists())) fail("production_file_missing", rel, 3)
    const measured = await sha256File(path)
    if (measured !== digest) fail("production_file_identity_mismatch", { file: rel, measured, expected: digest }, 3)
    verified[rel] = measured
  }
  // The platform dylib is installed next to the real @opentui/core package
  // (bun links it through the package's own node_modules); resolve through the
  // core symlink first, with the plain layout as fallback.
  const candidates = [
    `${import.meta.dir}/node_modules/@opentui/core-darwin-arm64/libopentui.dylib`,
    `${realpathSync(`${import.meta.dir}/node_modules/@opentui/core`)}/../core-darwin-arm64/libopentui.dylib`,
  ]
  let dylib: string | undefined
  for (const candidate of candidates) {
    if (await Bun.file(candidate).exists()) { dylib = candidate; break }
  }
  if (!dylib) fail("native_dylib_missing", candidates, 3)
  const measuredDylib = await sha256File(dylib)
  if (measuredDylib !== expected.native_dylib_sha256) {
    fail("native_dylib_identity_mismatch", { measured: measuredDylib, expected: expected.native_dylib_sha256 }, 3)
  }
  if (expected.excluded_bridge_dylib_sha256 && measuredDylib === expected.excluded_bridge_dylib_sha256) {
    fail("native_dylib_is_excluded_bridge", measuredDylib, 3)
  }
  const corePackage = `${import.meta.dir}/node_modules/@opentui/core/package.json`
  const core = await Bun.file(corePackage).json()
  if (core?.version !== expected.opentui_core_version) {
    fail("opentui_core_version_mismatch", { measured: core?.version, expected: expected.opentui_core_version }, 3)
  }
  return { verified_files: verified, native_dylib_sha256: measuredDylib,
    opentui_core_version: core?.version, bun_version: Bun.version, runtime: "bun",
    // F4: honesty about what these hashes cover.
    trust_boundary: "only the provenance-listed files are authenticity-checked; the pinned upstream checkout and all transitive node_modules dependencies are external trust boundaries and are NOT verified by these hashes",
    // N5: pin equality is enforced by upstream_opentui.py before launch.
    pins_enforced_by: "upstream_opentui.py wrapper" }
}

// Production focus comes from the renderer's own focused-editable state, not
// from the terminal cursor or any inferred channel.
function focusInfo(setup: TestRendererSetup) {
  const editor: any = (setup.renderer as any).currentFocusedEditor
  return editor
    ? { value: { focused_editable: true, kind: editor.constructor?.name ?? "unknown" }, source: "renderer.currentFocusedEditor" }
    : { value: null, source: "renderer_no_focused_editable" }
}

// B2: the protocol `actions` member is the input-application trace, in the
// exact shape the Rust adapters emit and ui_parity.py compares
// ({seq, type, key, applied, cursor_after}). Each entry is built ONLY from a
// real dispatch and its own measured outcome: the cursor is captured right
// after that dispatch + deterministic settle, BEFORE the next input, as an
// immutable per-event snapshot (`actions_cursor_after_source:
// "measured_per_dispatch"` on the frame). The raw replay detail (modifier
// state, per-key dispatch) is kept under the separate `input_trace` member.
// Key vocabulary follows this repository's protocol (hyphen: `ctrl-p`, as the
// local fixtures and oc2_pty's parser expect), not `ctrl+p`. No semantic
// action or cursor is invented; upstream app-semantic actions are NOT
// captured (semantic_actions stays null). `applied` is true when the key was
// dispatched without error and the stage outcome containing it was actually
// measured (a missed stage predicate aborts the whole flow via waitForFrame
// throwing — no false applied).
function protocolActions(trace: any[]) {
  return trace.map((entry, index) => ({
    seq: index,
    type: "key",
    key: entry.ctrl ? `ctrl-${entry.name}` : entry.name,
    applied: true,
    cursor_after: { row: entry.measured_cursor[1], col: entry.measured_cursor[0], visible: true },
  }))
}

function channelFields(setup: TestRendererSetup, trace: any[]) {
  return {
    focus: focusInfo(setup),
    hit_targets: [],
    hit_targets_source: "unavailable_no_production_hit_target_channel",
    actions: protocolActions(trace),
    actions_source: "upstream_input_replay_measured",
    actions_cursor_after_source: "measured_per_dispatch",
    input_trace: trace.map(entry => ({ ...entry })),
    input_trace_source: "upstream_input_replay_trace",
    semantic_actions: null,
    semantic_actions_source: "not_captured_from_upstream",
  }
}

// Only the home flow lives in this map. The dialog scenarios are dispatched to
// runDialogFlow by main(); deliberately NOT listed here, so if a dialog
// scenario ever reached the home flow it would fail closed as
// unsupported_scenario instead of running the wrong journey.
const SCENARIOS: Record<string, { search_text: string; expected_visible: string; home_marker: string }> = {
  "home-command-palette-v1": { search_text: "new", expected_visible: "New session", home_marker: "Ask anything" },
}

const sleep = (ms: number) => new Promise<void>(resolve => setTimeout(resolve, ms))

// One production checkpoint: real HOME route -> ctrl+p -> real palette input
// and search -> Escape, captured as actual renderer frames at each stage.
async function runProductionFlow(request: any, fixture: any, cols: number, rows: number) {
  if (request.events?.length || Object.keys(request.state_overrides ?? {}).length) {
    fail("unsupported_state_events", "state/event replay is not supported by this checkpoint")
  }
  const scenario = SCENARIOS[String(request.scenario)]
  if (!scenario) fail("unsupported_scenario", request.scenario)
  const measured = await verifyIdentity(request)
  const trace: any[] = []
  const { createAppFixture } = await import("./test/fixture/app.ts")
  const setup = await createAppFixture({ width: cols, height: rows, config: { animations: false } as any, args: {} as any })
  try {
    await Promise.race([setup.ready, sleep(8000)])
    const capture = (stage: string) => {
      const spans = setup.captureSpans()
      const screen = normalizeFrame(spans, fixture.id, request.theme)
      screen.cursor_source = "renderer_captured_frame"
      Object.assign(screen, channelFields(setup, trace))
      return { stage, screen }
    }
    const settle = async () => {
      // N3: deterministic renderer quiescence first (OpenTUI 0.6.8
      // waitForVisualIdle), then the production app's own 50ms afterPaint
      // timer (src/app.tsx:520) is given its real timer — the timer is not
      // replaced by an idle-only shortcut.
      await setup.waitForVisualIdle()
      await sleep(60)
    }
    // B2: dispatch → deterministic settle → per-event immutable cursor
    // snapshot, BEFORE the next input. The stage predicates afterwards stay
    // fail-closed readiness gates (waitForFrame throws on timeout).
    const pressAndMeasure = async (name: string, modifiers?: Record<string, boolean>) => {
      trace.push({ input: "key", name, ...(modifiers ?? {}) })
      setup.mockInput.pressKey(name, modifiers as any)
      await settle()
      const cursor = setup.captureSpans().cursor ?? [0, 0]
      trace[trace.length - 1].measured_cursor = cursor
    }
    await setup.waitForFrame((frame: string) => frame.includes("commands"))
    await settle()
    const home = capture("home")
    await pressAndMeasure("p", { ctrl: true })
    await setup.waitForFrame((frame: string) => frame.includes("Suggested"))
    const paletteOpen = capture("palette_open")
    for (const key of scenario.search_text) await pressAndMeasure(key)
    await setup.waitForFrame((frame: string) => frame.includes(scenario.expected_visible))
    const paletteSearch = capture("palette_search")
    trace.push({ input: "key", name: "escape" })
    setup.mockInput.pressEscape()
    await settle()
    trace[trace.length - 1].measured_cursor = setup.captureSpans().cursor ?? [0, 0]
    await setup.waitForFrame((frame: string) => frame.includes(scenario.home_marker))
    const paletteClosed = capture("palette_closed")
    return {
      schema: "oc2.ui-parity.screen.flow/v1",
      "evidence_class": "production_flow",
      fixture_id: fixture.id,
      scenario: request.scenario,
      viewport: { cols, rows },
      // N4: these are echoes of the request, not measurements — the rendered
      // theme is the app's own and the pins are enforced by the wrapper.
      "requested_theme": request.theme,
      "requested_pins": request.pins,
      "requested_clock_ms": request.clock_ms,
      "events_replayed": false,
      provenance: measured,
      frames: [home, paletteOpen, paletteSearch, paletteClosed],
    }
  } finally {
    await setup[Symbol.asyncDispose]()
  }
}

// Production form/permission checkpoint. The requests are injected through the
// fixture's real event stream using the upstream SSE event envelope (id/type/
// properties — the same wire shape our ported server publishes in
// crates/server/src/event_http.rs:124), then driven through the mounted
// upstream handlers and their actual keymap. No component is mounted directly:
// the dialog is produced by the pinned app reacting to a real `permission.asked`
// / `form.created` event, and every frame is the real renderer output.
//
// Fail-closed capability probes: every fixture-API member this flow relies on
// beyond the proven home-flow surface (the events channel, mockInput's enter
// dispatch, the fetch-option transport) is verified before use. A missing
// capability is a precise diagnostic — never a silent wrong capture.
const requestPath = (url: unknown): string => {
  const request = url as any
  const value = typeof url === "string" ? url
    : typeof request?.pathname === "string" ? request.pathname
    : typeof request?.url === "string" ? request.url
    : String(url)
  try { return new URL(value, "http://localhost").pathname } catch { return value.split("?")[0] }
}

const requestMethod = (input: unknown, original: unknown): string => {
  for (const candidate of [input, original]) {
    if (!candidate || typeof candidate !== "object") continue
    const method = (candidate as any).method
    if (typeof method === "string") return method.toUpperCase()
  }
  return "GET"
}

const requestBodyText = async (input: unknown, original: unknown): Promise<string> => {
  for (const candidate of [input, original]) {
    if (!candidate || typeof candidate !== "object") continue
    const request = candidate as any
    if (typeof request.clone === "function") {
      try { return await request.clone().text() } catch { /* try the other fetch argument */ }
    }
    if (typeof request.body === "string") return request.body
  }
  return ""
}

const rowText = (frame: any): string =>
  (frame.screen.rows as any[]).map((row: any) => row.text).join("\n")

const countMarker = (frame: any, marker: string): number =>
  (frame.screen.rows as any[]).reduce(
    (total: number, row: any) => total + (row.text.split(marker).length - 1), 0)

async function runDialogFlow(request: any, fixture: any, cols: number, rows: number) {
  // Fail closed on state/event replay exactly like the home flow: the dialog
  // journey is produced by the events THIS flow injects, so a request that
  // smuggles its own events/state must be refused rather than silently ignored
  // (the wrapper already denies it for production requests; a directly-run
  // adapter must not become the weaker path).
  if (request.events?.length || Object.keys(request.state_overrides ?? {}).length) {
    fail("unsupported_state_events", "state/event replay is not supported by this checkpoint")
  }
  const scenario = String(request.scenario)
  if (scenario !== "form-permission-v1" && scenario !== "form-multiselect-v1") {
    fail("unsupported_scenario", scenario)
  }
  const measured = await verifyIdentity(request)
  const replies: any[] = []
  const sessionID = "ses_oracle_dialog"
  // The reply/cancel mutation is recorded from the real client request and
  // answered with the no-content response the pinned client treats as success.
  // URL and body shapes are normalized: the client may hand us a URL object or
  // a string, and a Request or a plain init object.
  const originalFetch = globalThis.fetch
  const notFound = (url: any) => new Response(
    JSON.stringify({ error: "not_found", path: requestPath(url) }),
    { status: 404, headers: { "content-type": "application/json" } },
  )
  // Route the dialog journey's own requests. `null` means "not this capture's
  // request", so each transport below decides how to answer it.
  const route = async (url: any, input: any): Promise<Response | null> => {
    const path = requestPath(url)
    if (/^\/api\/session\/[^/]+\/permission\/[^/]+\/reply$/.test(path)) {
      replies.push({ method: requestMethod(input, url), path, body: await requestBodyText(input, url), status: 204 })
      return new Response(null, { status: 204 })
    }
    if (path.endsWith("/cancel")) {
      replies.push({ method: requestMethod(input, url), path, body: await requestBodyText(input, url), status: 204 })
      return new Response(null, { status: 204 })
    }
    if (path === `/api/session/${sessionID}`) return new Response(JSON.stringify({ data: {
      id: sessionID, projectID: "proj_test", title: "Oracle dialog", model: { providerID: "demo", id: "model" },
      location: { directory: "/tmp/oracle", project: { id: "proj_test", directory: "/tmp/oracle" } },
      time: { created: 0, updated: 0 }, tokens: { input: 0, output: 0, reasoning: 0, cache: { read: 0, write: 0 } }, cost: 0,
    } }), { headers: { "content-type": "application/json" } })
    if (path.includes(`/api/session/${sessionID}/message`)) return new Response(JSON.stringify({ data: [], cursor: {} }), { headers: { "content-type": "application/json" } })
    if (path.includes(`/api/session/${sessionID}/inbox`)) return new Response(JSON.stringify({ data: [] }), { headers: { "content-type": "application/json" } })
    return null
  }
  // The fixture's `fetch` option is the transport the mounted app uses, and it
  // is hermetic: an unrouted request is answered with an explicit 404 instead
  // of `undefined` (an opaque client failure that would mask the stage under
  // test) and never reaches the network.
  const fixtureFetch = async (url: any, input: any) => (await route(url, input)) ?? notFound(url)
  // Some production code paths call `globalThis.fetch` directly rather than the
  // injected option, so the same router is installed there for the duration of
  // the flow. Everything outside the dialog journey keeps the transport that
  // was already installed, so the capture never takes over unrelated requests.
  const globalFetch = async (url: any, input: any) => {
    const routed = await route(url, input)
    if (routed) return routed
    if (typeof originalFetch === "function") {
      try { return await originalFetch(url, input) } catch { /* fall through to the explicit 404 */ }
    }
    return notFound(url)
  }
  // Install before the fixture module is imported/mounted so no startup request
  // can slip past the interceptor. A locked/non-writable global is tolerated:
  // the `fetch` option below still transports every request.
  let installed = false
  try {
    globalThis.fetch = globalFetch as any
    installed = true
  } catch { /* fall back to the injected fetch option only */ }
  const restoreFetch = () => {
    if (!installed) return
    try { globalThis.fetch = originalFetch } catch { /* nothing further to restore */ }
  }
  let setup: any
  const { createAppFixture } = await import("./test/fixture/app.ts")
  if (typeof createAppFixture !== "function") {
    restoreFetch()
    fail("fixture_entry_missing_createAppFixture", "test/fixture/app.ts")
  }
  try {
    setup = await createAppFixture({
      width: cols, height: rows, config: { animations: false } as any,
      args: { sessionID } as any,
      // The fixture's `fetch` option is the transport the mounted app uses; it
      // routes through the shared router above so either transport records the
      // same mutation exactly once.
      fetch: async (url: any, input: any) => fixtureFetch(url, input),
    })
  } catch (error) {
    restoreFetch()
    fail("fixture_mount_failed", { message: String((error as any)?.stack ?? error) })
  }
  try {
    await Promise.race([setup.ready, sleep(8000)])
    // Capability probes (fail closed, precise codes).
    if (typeof setup?.events?.emit !== "function") {
      fail("fixture_events_channel_missing", { available_keys: Object.keys(setup ?? {}) })
    }
    const mock = setup?.mockInput
    if (typeof mock?.pressKey !== "function" || typeof mock?.pressEnter !== "function") {
      fail("fixture_mock_input_missing", { pressKey: typeof mock?.pressKey, pressEnter: typeof mock?.pressEnter })
    }
    if (typeof setup?.waitForFrame !== "function" || typeof setup?.waitForVisualIdle !== "function"
        || typeof setup?.captureSpans !== "function"
        || typeof (setup as any)?.[Symbol.asyncDispose] !== "function") {
      fail("fixture_renderer_helpers_missing", {
        "waitForFrame": typeof setup?.waitForFrame,
        "waitForVisualIdle": typeof setup?.waitForVisualIdle,
        "captureSpans": typeof setup?.captureSpans,
        // Teardown is part of the measured contract: the flow's disposal must
        // be a real capability too, otherwise a later TypeError would report a
        // misleading crash instead of this precise diagnostic.
        "asyncDispose": typeof (setup as any)?.[Symbol.asyncDispose],
      })
    }
    const settle = async () => { await setup.waitForVisualIdle(); await sleep(60) }
    const trace: any[] = []
    const capture = (stage: string) => {
      const screen = normalizeFrame(setup.captureSpans(), fixture.id, request.theme)
      // Same cursor provenance declaration as the home flow: the cursor is the
      // renderer's own captured frame cursor, not an inferred position.
      screen.cursor_source = "renderer_captured_frame"
      Object.assign(screen, channelFields(setup, trace))
      return { stage, screen }
    }
    // Every dispatched key is recorded with its own measured cursor snapshot
    // (the same per-dispatch protocol the home flow emits), so the action
    // channel is a real replay trace and never a fabricated one.
    const press = async (name: string, modifiers?: Record<string, boolean>) => {
      trace.push({ input: "key", name, ...(modifiers ?? {}) })
      setup.mockInput.pressKey(name, modifiers as any)
      await settle()
      trace[trace.length - 1].measured_cursor = setup.captureSpans().cursor ?? [0, 0]
    }
    const pressEnter = async () => {
      trace.push({ input: "key", name: "enter" })
      setup.mockInput.pressEnter()
      await settle()
      trace[trace.length - 1].measured_cursor = setup.captureSpans().cursor ?? [0, 0]
    }
    // Bounded stage waits: a stage that never paints is a precise failure with
    // the attempted trigger attached, never a silent hang or a skipped frame.
    const waitFrame = async (predicate: (frame: string) => boolean, code: string, detail: any, ms = 12000) => {
      let done = false
      const timer: any = setTimeout(() => { if (!done) fail(code, detail) }, ms)
      timer.unref?.()
      try {
        await setup.waitForFrame(predicate)
      } catch (error) {
        fail(code, { ...detail, wait_error: String((error as any)?.message ?? error) })
      } finally {
        done = true
        clearTimeout(timer)
      }
    }
    const waitCondition = async (predicate: () => boolean, code: string, detail: any, ms = 4000) => {
      const deadline = Date.now() + ms
      while (!predicate()) {
        if (Date.now() > deadline) fail(code, detail)
        await sleep(50)
      }
    }
    const emit = (event: any) => setup.events.emit(event)
    if (scenario === "form-permission-v1") {
      // The pinned upstream fixture event consumes the public session event
      // envelope. Supplies its permission properties directly through the real
      // subscription path; data is controlled fixture input, not a state override.
      emit({ id: "evt_permission", type: "permission.asked", properties: {
        id: "per_oracle", sessionID, type: "shell", action: "shell", title: "Run shell command",
        resources: ["echo oracle"], save: ["echo oracle"],
      } })
      await waitFrame((f: string) => f.includes("Permission required"),
        "permission_prompt_not_observed", { event: "permission.asked", sessionID })
      await settle()
      const permission = capture("permission_initial")
      // The initial option must be the pinned upstream's actual default, not a
      // frame that merely contains the option labels in a different order.
      const permissionRows = permission.screen.rows.map((row: any) => row.text)
      if (!permissionRows.some((line: string) => line.includes("→ Allow once")))
        fail("permission_initial_selected_option", permissionRows)
      if (permissionRows.findIndex((line: string) => line.includes("Allow once")) < 0
          || permissionRows.findIndex((line: string) => line.includes("Always allow")) <= permissionRows.findIndex((line: string) => line.includes("Allow once"))
          || permissionRows.findIndex((line: string) => line.includes("Reject")) <= permissionRows.findIndex((line: string) => line.includes("Always allow")))
        fail("permission_initial_option_order", permissionRows)
      // Option set: the ported labels (`permission.tsx`, native_permissions.rs:159-161)
      // with save rules present, so `always` is offered (FORM-001).
      for (const marker of ["Permission required", "Allow once", "Always allow", "Reject"]) {
        if (!permission.screen.rows.some((row: any) => row.text.includes(marker)))
          fail("permission_initial_missing", { marker, rows: permission.screen.rows.map((r: any) => r.text) })
      }
      // The swap must be a swap: the persisted-rules body is not pre-painted.
      if (permission.screen.rows.some((row: any) => row.text.includes("This will always allow the following patterns for this project.")))
        fail("permission_always_body_pre_painted", permission.screen.rows.map((r: any) => r.text))
      // FORM-001 "changing selected option changes the body": arrow-right / `l`
      // cycles the selected option (audit-messages.md §8: "Arrow-left/h and
      // arrow-right/l cycle options; Enter confirms"; ported at
      // native_permissions.rs:640-655), so selecting `always` swaps the body to
      // the rules it will persist (`util/permission.ts:154-160` via
      // permission_always_lines). The swap frame is captured BEFORE Enter:
      // capturing after Enter would only show the post-submit surface.
      await press("l")
      const alwaysBody = capture("permission_always_body")
      if (rowText(alwaysBody) === rowText(permission))
        fail("permission_body_swap_not_observed", { initial: rowText(permission) })
      if (!alwaysBody.screen.rows.some((row: any) => row.text.includes("→ Always allow")))
        fail("permission_always_option_not_selected", alwaysBody.screen.rows.map((row: any) => row.text))
      if (!alwaysBody.screen.rows.some((row: any) => row.text.includes("This will always allow the following patterns for this project."))
          || !alwaysBody.screen.rows.some((row: any) => row.text.includes("- echo oracle")))
        fail("permission_always_body_missing", alwaysBody.screen.rows.map((r: any) => r.text))
      await pressEnter()
      // The selected decision travels under the `reply` key; no client sends a
      // `decision` key (native_permissions.rs:565 posts { reply, scope };
      // crates/server/src/lib.rs:690-693 accepts reply once|always|reject).
      await waitCondition(() => replies.length >= 1, "permission_reply_not_observed", { dispatched: "enter" })
      if (replies.length !== 1 || replies[0].status !== 204 || !replies[0].body.includes('"reply":"always"'))
        fail("permission_reply_not_successful", replies)
      if (!replies[0].path.includes("/permission/") || !replies[0].path.includes("/reply"))
        fail("permission_reply_path_invalid", replies[0].path)
      await settle()
      const submitted = capture("permission_submitted")
      // Replying resolves the request: the open prompt (and its always body)
      // must be gone from the painted surface.
      if (submitted.screen.rows.some((row: any) =>
          ["Permission required", "Allow once", "Always allow", "Reject",
           "This will always allow the following patterns for this project."]
            .some((marker: string) => row.text.includes(marker))))
        fail("permission_prompt_still_open", submitted.screen.rows.map((r: any) => r.text))
      // Snapshot the recorded replies at return time: the teardown below runs
      // in `finally` after this object is built, and a disposal-triggered
      // cancel must not be counted as a journey mutation (it would otherwise
      // mutate the same array before `main` serializes the result).
      return { schema: "oc2.ui-parity.screen.flow/v1", evidence_class: "production_flow", fixture_id: fixture.id,
        scenario, viewport: { cols, rows }, requested_theme: request.theme, requested_pins: request.pins,
        requested_clock_ms: request.clock_ms, events_replayed: false, provenance: measured,
        frames: [permission, alwaysBody, submitted], replies: [...replies] }
    }
    // Upstream form wire shape per our ported server (`form.created` ->
    // {"form": FormInfo}, event_http.rs:110; fields carry key/type/title/
    // required/options[{value,label,description}] — crates/contracts/src/lib.rs:507-526).
    emit({ id: "evt_form", type: "form.created", properties: { form: {
      id: "frm_oracle", sessionID, title: "Choose targets", fields: [
        { key: "targets", type: "multiselect", title: "Targets", required: true, minItems: 1,
          options: [{ value: "staging", label: "Staging", description: "Deploy to staging" }] },
        { key: "optional", type: "multiselect", title: "Optional", required: false,
          options: [{ value: "logs", label: "Logs", description: "Include logs" }] },
      ] } } })
    await waitFrame((f: string) => f.includes("Choose targets"),
      "form_prompt_not_observed", { event: "form.created", sessionID })
    await settle()
    const initial = capture("form_initial")
    if (!initial.screen.rows.some((row: any) => row.text.includes("[ ] Staging")))
      fail("form_required_initial_check_missing", initial.screen.rows.map((row: any) => row.text))
    // Real space toggle on the focused required multiselect (`form.tsx:753-755`
    // via native_view_dialogs.rs:5820): the first space selects the only option,
    // the second deselects it, and `storeAnswer` normalizes the empty list away
    // (form.tsx:257-267), so the required list ends unanswered. The two
    // intermediate frames make the round-trip observable.
    await press("space")
    const selected = capture("form_required_selected")
    if (rowText(selected) === rowText(initial))
      fail("form_required_toggle_not_observed", { initial: rowText(initial) })
    if (!selected.screen.rows.some((row: any) => row.text.includes("[✓] Staging")))
      fail("form_required_check_not_painted", selected.screen.rows.map((r: any) => r.text))
    await press("space")
    const cleared = capture("form_required_cleared")
    if (rowText(cleared) !== rowText(initial))
      fail("form_required_toggle_not_reversible", { cleared: rowText(cleared), initial: rowText(initial) })
    // Tab past the untouched optional field into the review tab, then capture
    // the PRE-submit review surface: it already marks the required field with
    // the error color (audit-messages.md §8 "review ... marks invalid/required/
    // missing external acknowledgements using error color").
    await press("tab")
    await press("tab")
    const review = capture("form_review")
    const reviewCount = countMarker(review, "Answer required")
    if (reviewCount < 1)
      fail("form_required_error_missing", { stage: "form_review", rows: review.screen.rows.map((r: any) => r.text) })
    if (!review.screen.rows.some((row: any) => row.text.includes("Optional: (not answered)")))
      fail("form_optional_control_missing", { stage: "form_review", rows: review.screen.rows.map((r: any) => r.text) })
    // Enter on the review tab attempts the submit (`form.tsx:513-534` via
    // native_view_dialogs.rs:6034). The empty required field blocks it and the
    // current validation error is painted at the footer's opposite side
    // (audit-messages.md §8) — validation waits until the matching submit point
    // (FORM-006), so this occurrence is NEW: the post-Enter frame must contain
    // strictly more of the marker than the pre-submit review frame. A no-op
    // Enter leaves the counts equal and fails here.
    await pressEnter()
    await waitFrame((f: string) => f.split("Answer required").length - 1 > reviewCount,
      "form_submit_attempt_not_observed", { review_count: reviewCount, dispatched: "enter" })
    const submitted = capture("form_submit_error")
    if (countMarker(submitted, "Answer required") <= reviewCount)
      fail("form_submit_attempt_not_observed", {
        review_count: reviewCount, submit_count: countMarker(submitted, "Answer required"),
        rows: submitted.screen.rows.map((r: any) => r.text),
      })
    if (replies.length !== 0)
      fail("form_reply_unexpectedly_observed", replies)
    if (!submitted.screen.rows.some((row: any) => row.text.includes("Optional: (not answered)")))
      fail("form_optional_control_missing", { stage: "form_submit_error", rows: submitted.screen.rows.map((r: any) => r.text) })
    // Same snapshot discipline as the permission flow: the blocked submit
    // already verified `replies.length === 0` above, and a teardown-time
    // cancel must not mutate the serialized evidence.
    return { schema: "oc2.ui-parity.screen.flow/v1", evidence_class: "production_flow", fixture_id: fixture.id,
      scenario, viewport: { cols, rows }, requested_theme: request.theme, requested_pins: request.pins,
      requested_clock_ms: request.clock_ms, events_replayed: false, provenance: measured,
      frames: [initial, selected, cleared, review, submitted], replies: [...replies] }
  } finally {
    // Restore the process-wide transport before teardown so the mount's own
    // disposal cannot be the last caller of the capture handler.
    restoreFetch()
    // The measured evidence (frames plus the reply snapshot) was already
    // produced before this `finally` runs. A teardown failure must not convert
    // a successful, measured capture into a protocol failure that hides those
    // bytes: disposal is best-effort, and its presence was already checked by
    // the fail-closed capability probe above.
    try {
      await setup[Symbol.asyncDispose]()
    } catch { /* best-effort teardown; evidence already captured */ }
  }
}

async function main() {
  const input = await new Response(Bun.stdin).text()
  const request = JSON.parse(input || "{}")
  const fixture = request.fixture ?? { id: "synthetic", viewport: { columns: 80, rows: 24 } }
  const cols = Number(request.viewport?.cols ?? fixture.viewport?.columns ?? 80)
  const rows = Number(request.viewport?.rows ?? fixture.viewport?.rows ?? 24)
  let frame: any
  let evidenceClass = "app_first_frame"
  if (process.argv.includes("--component")) {
    evidenceClass = "synthetic"
    const setup = await createTestRenderer({ width: cols, height: rows, bufferedOutput: "memory" })
    const text = String(fixture.component_text ?? "PAR-03 fixture")
    setup.renderer.root.add(new TextRenderable(setup.renderer, { content: text, width: cols, height: 1 }))
    await setup.renderOnce()
    frame = setup.captureSpans(); await setup[Symbol.asyncDispose]()
  } else if (process.argv.includes("--synthetic")) {
    evidenceClass = "synthetic"
    const setup = await createTestRenderer({ width: cols, height: rows, bufferedOutput: "memory" })
    setup.renderer.start()
    frame = setup.captureSpans(); setup.renderer.destroy()
  } else if (request.production === true) {
    // The home command-palette flow and the dialog flows are distinct
    // production checkpoints; an unknown scenario is refused by the wrapper
    // before launch (SUPPORTED_SCENARIOS), so this dispatch is exhaustive.
    const result = request.scenario === "home-command-palette-v1"
      ? await runProductionFlow(request, fixture, cols, rows)
      : await runDialogFlow(request, fixture, cols, rows)
    console.log(JSON.stringify(result))
    return
  } else {
    const { createAppFixture } = await import("./test/fixture/app.ts")
    const app = await createAppFixture({ width: cols, height: rows, config: { animations: false } as any, args: {} as any })
    // App startup is asynchronous and may be blocked by an unavailable native
    // backend. Capture whatever deterministic first frame it produced.
    await Promise.race([app.ready, new Promise(resolve => setTimeout(resolve, 1500))])
    frame = app.captureSpans()
    await app[Symbol.asyncDispose]()
  }
  const screen = normalizeFrame(frame, fixture.id, request.theme)
  screen.evidence_class = evidenceClass
  // N-b: request events/state_overrides are never replayed by this adapter;
  // declare it instead of implying they are honoured.
  screen.events_replayed = false
  console.log(JSON.stringify(screen))
}
if (import.meta.main) main().catch(error => { console.error(error.stack ?? error); process.exit(1) })
