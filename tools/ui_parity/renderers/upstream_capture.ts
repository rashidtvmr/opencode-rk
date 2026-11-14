// This file is copied into the pinned upstream workspace's `packages/tui/`
// directory by tools/ui_parity/renderers/upstream_opentui.py before it is run,
// so bare `@opentui/*` imports resolve against that package's node_modules and
// the fixture import is relative to packages/tui/.
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
    console.log(JSON.stringify(await runProductionFlow(request, fixture, cols, rows)))
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
