# LANE-WEB-CANVAS – scratchpad

## Claim
- Task: RAW_FEATURE 3.7 piece 1 – canvas data model
- Session: ses_worker_web_canvas
- Files: web/src/lib/canvas-model.ts, web/src/lib/canvas-model.test.ts

## Source evidence
- Mission spec: interactive infinite canvas data model (nodes, edges, viewport, hit-test, eviction, JSON)
- web/src/lib/ existing convention: vitest + ts, no React deps in pure lib

## RED phase
- RED hash (test file): a9fdbc497984d3e25957647ce0f67d7c9a670712a46252e53fe802cfd7b1802f
- Confirmed: `Failed to resolve import "./canvas-model"` from test

## Test authoring fixes (post-freeze, not assertion weakening)
- T13: passed raw object instead of JSON string → fixed to `'{"not":"valid"}'`
- T08: tried to addEdge to evicted node → restructured to verify edge cleanup after eviction
- Test file sha after fixes: 2b1c20033bc826553c484aa9ba4b7a69a2dfa3a6c42c388e763f079fbf889839

## GREEN phase
- 16/16 tests pass via `pnpm vitest src/lib/canvas-model.test.ts`
- Zero test edits post-fix

## Implementation summary
- `canvas-model.ts`: pure TypeScript, zero deps
- Types: CanvasNode (id/kind/label/status/x/y), CanvasEdge (from/to/kind), Viewport (x/y/zoom), CanvasState
- Functions: makeCanvas, addNode, removeNode, getNode, evict, addEdge (cycle rejection), findNodeAtPoint, setViewport (zoom clamp), canvasToJson, canvasFromJson, cloneCanvas
- MAX_NODES=200, FIFO eviction, cycle detection via DFS, viewport transform in hit-test

## Follow-up (not owned)
- App.tsx wiring (later lane)
- Rendering layer (separate lane)

## Deviations
- Test file edited 2x after RED to fix authoring errors (see above)
