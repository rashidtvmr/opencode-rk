// This file is copied into the pinned upstream workspace's `packages/tui/`
// directory by tools/ui_parity/renderers/upstream_opentui.py before it is run,
// so bare `@opentui/*` imports resolve against that package's node_modules and
// the fixture import is relative to packages/tui/.
import { createTestRenderer } from "@opentui/core/testing"
import { ansi256IndexToRgb } from "@opentui/core"

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

async function main() {
  const input = await new Response(Bun.stdin).text()
  const request = JSON.parse(input || "{}")
  const fixture = request.fixture ?? { id: "synthetic", viewport: { columns: 80, rows: 24 } }
  const cols = Number(request.viewport?.cols ?? fixture.viewport?.columns ?? 80)
  const rows = Number(request.viewport?.rows ?? fixture.viewport?.rows ?? 24)
  let frame: any
  if (process.argv.includes("--synthetic")) {
    const setup = await createTestRenderer({ width: cols, height: rows, bufferedOutput: "memory" })
    setup.renderer.start()
    frame = setup.captureSpans(); setup.renderer.destroy()
  } else {
    const { createAppFixture } = await import("./test/fixture/app.ts")
    const app = await createAppFixture({ width: cols, height: rows, config: { animations: false } as any, args: {} as any })
    // App startup is asynchronous and may be blocked by an unavailable native
    // backend. Capture whatever deterministic first frame it produced.
    await Promise.race([app.ready, new Promise(resolve => setTimeout(resolve, 1500))])
    frame = app.captureSpans()
    await app[Symbol.asyncDispose]()
  }
  console.log(JSON.stringify(normalizeFrame(frame, fixture.id, request.theme)))
}
if (import.meta.main) main().catch(error => { console.error(error.stack ?? error); process.exit(1) })
