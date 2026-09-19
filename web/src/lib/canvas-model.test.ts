import { describe, expect, it } from 'vitest'

import {
  type CanvasEdge,
  type CanvasNode,
  type CanvasState,
  type Viewport,
  addEdge,
  addNode,
  canvasFromJson,
  canvasToJson,
  cloneCanvas,
  evict,
  findNodeAtPoint,
  getNode,
  makeCanvas,
  MAX_NODES,
  removeNode,
  setViewport,
} from './canvas-model'

function makeTestNode(overrides: Partial<CanvasNode> = {}): CanvasNode {
  return {
    id: `node-${Math.random().toString(36).slice(2, 8)}`,
    kind: 'session',
    label: 'test',
    status: 'idle',
    x: 0,
    y: 0,
    ...overrides,
  }
}

// ---------------------------------------------------------------------------
// T01 – makeCanvas returns empty state with default viewport
// ---------------------------------------------------------------------------
it('T01 makeCanvas returns empty state with default viewport', () => {
  const canvas = makeCanvas()
  expect(canvas.nodes).toHaveLength(0)
  expect(canvas.edges).toHaveLength(0)
  expect(canvas.viewport).toEqual({ x: 0, y: 0, zoom: 1 })
})

// ---------------------------------------------------------------------------
// T02 – addNode inserts a node and getNode retrieves it
// ---------------------------------------------------------------------------
it('T02 addNode inserts a node and getNode retrieves it', () => {
  let canvas = makeCanvas()
  const node = makeTestNode({ id: 'n1', kind: 'session', label: 'S1' })
  canvas = addNode(canvas, node)
  expect(canvas.nodes).toHaveLength(1)
  const got = getNode(canvas, 'n1')
  expect(got).toBeDefined()
  expect(got!.label).toBe('S1')
  expect(got!.kind).toBe('session')
})

// ---------------------------------------------------------------------------
// T03 – addNode rejects duplicate id
// ---------------------------------------------------------------------------
it('T03 addNode rejects duplicate id', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'dup' }))
  expect(() => addNode(canvas, makeTestNode({ id: 'dup' }))).toThrow(
    /duplicate/i,
  )
})

// ---------------------------------------------------------------------------
// T04 – addEdge links parent→child and creates cycle rejection
// ---------------------------------------------------------------------------
it('T04 addEdge links parent→child and rejects cycles', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'a' }))
  canvas = addNode(canvas, makeTestNode({ id: 'b' }))
  canvas = addNode(canvas, makeTestNode({ id: 'c' }))
  canvas = addEdge(canvas, { from: 'a', to: 'b', kind: 'delegation' })
  canvas = addEdge(canvas, { from: 'b', to: 'c', kind: 'delegation' })
  // a→b→c exists; adding c→a would create cycle
  expect(() =>
    addEdge(canvas, { from: 'c', to: 'a', kind: 'delegation' }),
  ).toThrow(/cycle/i)
})

// ---------------------------------------------------------------------------
// T05 – removeNode removes node and its connected edges
// ---------------------------------------------------------------------------
it('T05 removeNode removes node and its connected edges', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'x' }))
  canvas = addNode(canvas, makeTestNode({ id: 'y' }))
  canvas = addEdge(canvas, { from: 'x', to: 'y', kind: 'delegation' })
  canvas = removeNode(canvas, 'x')
  expect(canvas.nodes).toHaveLength(1)
  expect(canvas.edges).toHaveLength(0)
  expect(getNode(canvas, 'x')).toBeUndefined()
})

// ---------------------------------------------------------------------------
// T06 – removeNode with non-existent id is no-op
// ---------------------------------------------------------------------------
it('T06 removeNode with non-existent id is no-op', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'a' }))
  canvas = removeNode(canvas, 'ghost')
  expect(canvas.nodes).toHaveLength(1)
})

// ---------------------------------------------------------------------------
// T07 – evict oldest nodes when cap exceeded
// ---------------------------------------------------------------------------
it('T07 evict oldest nodes when cap exceeded', () => {
  let canvas = makeCanvas()
  for (let i = 0; i < MAX_NODES + 2; i++) {
    canvas = addNode(canvas, makeTestNode({ id: `n${i}` }))
  }
  // After exceeding cap, oldest should be evicted
  expect(canvas.nodes.length).toBeLessThanOrEqual(MAX_NODES)
  // n0 and n1 should be gone (FIFO eviction)
  expect(getNode(canvas, 'n0')).toBeUndefined()
  expect(getNode(canvas, 'n1')).toBeUndefined()
  // Latest nodes survive
  expect(getNode(canvas, `n${MAX_NODES + 1}`)).toBeDefined()
})

// ---------------------------------------------------------------------------
// T08 – evict removes edges connected to evicted nodes
// ---------------------------------------------------------------------------
it('T08 evict removes edges connected to evicted nodes', () => {
  let canvas = makeCanvas()
  // Add 200 fill nodes, then 'keep' triggers eviction of fill0
  for (let i = 0; i < MAX_NODES; i++) {
    canvas = addNode(canvas, makeTestNode({ id: `fill${i}` }))
  }
  canvas = addNode(canvas, makeTestNode({ id: 'keep' }))
  // keep survived; fill0 was evicted
  expect(getNode(canvas, 'keep')).toBeDefined()
  expect(getNode(canvas, 'fill0')).toBeUndefined()
  // Any remaining edges must reference surviving nodes
  for (const e of canvas.edges) {
    expect(getNode(canvas, e.from)).toBeDefined()
    expect(getNode(canvas, e.to)).toBeDefined()
  }
})

// ---------------------------------------------------------------------------
// T09 – findNodeAtPoint returns node under coordinates
// ---------------------------------------------------------------------------
it('T09 findNodeAtPoint returns node under coordinates', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'h1', x: 100, y: 100 }))
  canvas = addNode(canvas, makeTestNode({ id: 'h2', x: 300, y: 300 }))
  // With default viewport (0,0,1), point at (100,100) hits h1
  const hit = findNodeAtPoint(canvas, 100, 100)
  expect(hit?.id).toBe('h1')
  // Point far away hits nothing
  const miss = findNodeAtPoint(canvas, 999, 999)
  expect(miss).toBeUndefined()
})

// ---------------------------------------------------------------------------
// T10 – findNodeAtPoint accounts for viewport transform
// ---------------------------------------------------------------------------
it('T10 findNodeAtPoint accounts for viewport transform', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'v1', x: 100, y: 100 }))
  // Pan viewport left by 50
  canvas = setViewport(canvas, { x: -50, y: 0, zoom: 1 })
  // Screen point (50,100) → canvas point (50+50, 100) = (100, 100) → hits v1
  const hit = findNodeAtPoint(canvas, 50, 100)
  expect(hit?.id).toBe('v1')
})

// ---------------------------------------------------------------------------
// T11 – setViewport clamps zoom to [0.1, 5]
// ---------------------------------------------------------------------------
it('T11 setViewport clamps zoom to [0.1, 5]', () => {
  let canvas = makeCanvas()
  canvas = setViewport(canvas, { x: 0, y: 0, zoom: 100 })
  expect(canvas.viewport.zoom).toBe(5)
  canvas = setViewport(canvas, { x: 0, y: 0, zoom: 0 })
  expect(canvas.viewport.zoom).toBe(0.1)
})

// ---------------------------------------------------------------------------
// T12 – canvasToJson / canvasFromJson round-trip preserves data
// ---------------------------------------------------------------------------
it('T12 canvasToJson / canvasFromJson round-trip preserves data', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'r1', kind: 'subagent', label: 'Agent-1', status: 'running', x: 50, y: 75 }))
  canvas = addNode(canvas, makeTestNode({ id: 'r2', kind: 'session', label: 'Sess-1', status: 'idle', x: 200, y: 300 }))
  canvas = addEdge(canvas, { from: 'r1', to: 'r2', kind: 'fork' })
  canvas = setViewport(canvas, { x: 10, y: 20, zoom: 1.5 })

  const json = canvasToJson(canvas)
  const restored = canvasFromJson(json)

  expect(restored.nodes).toHaveLength(2)
  expect(restored.edges).toHaveLength(1)
  expect(restored.viewport).toEqual({ x: 10, y: 20, zoom: 1.5 })
  expect(restored.edges[0].kind).toBe('fork')
  expect(getNode(restored, 'r1')?.label).toBe('Agent-1')
  expect(getNode(restored, 'r2')?.status).toBe('idle')
})

// ---------------------------------------------------------------------------
// T13 – canvasToJson throws on invalid data
// ---------------------------------------------------------------------------
it('T13 canvasFromJson throws on invalid JSON shape', () => {
  expect(() => canvasFromJson('{"not":"valid"}')).toThrow(/invalid/i)
  expect(() => canvasFromJson('"just a string"')).toThrow(/invalid/i)
})

// ---------------------------------------------------------------------------
// T14 – cloneCanvas produces independent copy
// ---------------------------------------------------------------------------
it('T14 cloneCanvas produces independent copy', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'orig' }))
  const cloned = cloneCanvas(canvas)
  canvas = removeNode(canvas, 'orig')
  expect(cloned.nodes).toHaveLength(1)
  expect(canvas.nodes).toHaveLength(0)
})

// ---------------------------------------------------------------------------
// T15 – addEdge rejects edge to non-existent node
// ---------------------------------------------------------------------------
it('T15 addEdge rejects edge to non-existent node', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'only' }))
  expect(() =>
    addEdge(canvas, { from: 'only', to: 'ghost', kind: 'delegation' }),
  ).toThrow(/not found/i)
})

// ---------------------------------------------------------------------------
// T16 – addEdge rejects self-edge
// ---------------------------------------------------------------------------
it('T16 addEdge rejects self-edge', () => {
  let canvas = makeCanvas()
  canvas = addNode(canvas, makeTestNode({ id: 'self' }))
  expect(() =>
    addEdge(canvas, { from: 'self', to: 'self', kind: 'delegation' }),
  ).toThrow(/self/i)
})
