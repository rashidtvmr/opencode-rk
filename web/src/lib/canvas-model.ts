export type NodeKind = 'session' | 'subagent'
export type NodeStatus = 'idle' | 'running' | 'completed' | 'failed'
export type EdgeKind = 'delegation' | 'fork'

export interface CanvasNode {
  id: string
  kind: NodeKind
  label: string
  status: NodeStatus
  x: number
  y: number
}

export interface CanvasEdge {
  from: string
  to: string
  kind: EdgeKind
}

export interface Viewport {
  x: number
  y: number
  zoom: number
}

export interface CanvasState {
  nodes: CanvasNode[]
  edges: CanvasEdge[]
  viewport: Viewport
}

export const MAX_NODES = 200
const MIN_ZOOM = 0.1
const MAX_ZOOM = 5
const NODE_HIT_RADIUS = 24

export function makeCanvas(): CanvasState {
  return { nodes: [], edges: [], viewport: { x: 0, y: 0, zoom: 1 } }
}

export function getNode(c: CanvasState, id: string): CanvasNode | undefined {
  return c.nodes.find((n) => n.id === id)
}

function hasCycle(edges: CanvasEdge[], from: string, to: string): boolean {
  const visited = new Set<string>()
  const stack = [to]
  while (stack.length > 0) {
    const cur = stack.pop()!
    if (cur === from) return true
    if (visited.has(cur)) continue
    visited.add(cur)
    for (const e of edges) {
      if (e.from === cur) stack.push(e.to)
    }
  }
  return false
}

export function addNode(c: CanvasState, node: CanvasNode): CanvasState {
  if (c.nodes.some((n) => n.id === node.id)) {
    throw new Error(`Duplicate node id: ${node.id}`)
  }
  let nodes = [...c.nodes, node]
  if (nodes.length > MAX_NODES) {
    const removed = nodes.slice(0, nodes.length - MAX_NODES)
    const removedIds = new Set(removed.map((n) => n.id))
    const edges = c.edges.filter(
      (e) => !removedIds.has(e.from) && !removedIds.has(e.to),
    )
    nodes = nodes.slice(nodes.length - MAX_NODES)
    return { ...c, nodes, edges }
  }
  return { ...c, nodes }
}

export function removeNode(c: CanvasState, id: string): CanvasState {
  const node = getNode(c, id)
  if (!node) return c
  return {
    ...c,
    nodes: c.nodes.filter((n) => n.id !== id),
    edges: c.edges.filter((e) => e.from !== id && e.to !== id),
  }
}

export function evict(c: CanvasState): CanvasState {
  if (c.nodes.length <= MAX_NODES) return c
  const excess = c.nodes.length - MAX_NODES
  const removedIds = new Set(c.nodes.slice(0, excess).map((n) => n.id))
  return {
    ...c,
    nodes: c.nodes.slice(excess),
    edges: c.edges.filter(
      (e) => !removedIds.has(e.from) && !removedIds.has(e.to),
    ),
  }
}

export function addEdge(c: CanvasState, edge: CanvasEdge): CanvasState {
  if (edge.from === edge.to) {
    throw new Error('Self-edge not allowed')
  }
  if (!getNode(c, edge.from)) {
    throw new Error(`Node not found: ${edge.from}`)
  }
  if (!getNode(c, edge.to)) {
    throw new Error(`Node not found: ${edge.to}`)
  }
  if (hasCycle(c.edges, edge.from, edge.to)) {
    throw new Error('Cycle detected')
  }
  return { ...c, edges: [...c.edges, edge] }
}

export function findNodeAtPoint(
  c: CanvasState,
  sx: number,
  sy: number,
): CanvasNode | undefined {
  const { x: vx, y: vy, zoom } = c.viewport
  const cx = (sx - vx) / zoom
  const cy = (sy - vy) / zoom
  let best: CanvasNode | undefined
  let bestDist = Infinity
  for (const n of c.nodes) {
    const dx = cx - n.x
    const dy = cy - n.y
    const dist = Math.sqrt(dx * dx + dy * dy)
    if (dist <= NODE_HIT_RADIUS && dist < bestDist) {
      best = n
      bestDist = dist
    }
  }
  return best
}

export function setViewport(c: CanvasState, vp: Viewport): CanvasState {
  return {
    ...c,
    viewport: {
      x: vp.x,
      y: vp.y,
      zoom: Math.min(MAX_ZOOM, Math.max(MIN_ZOOM, vp.zoom)),
    },
  }
}

export function canvasToJson(c: CanvasState): string {
  return JSON.stringify(c)
}

export function canvasFromJson(raw: string): CanvasState {
  const parsed = JSON.parse(raw)
  if (
    typeof parsed !== 'object' ||
    parsed === null ||
    !Array.isArray(parsed.nodes) ||
    !Array.isArray(parsed.edges) ||
    typeof parsed.viewport !== 'object' ||
    parsed.viewport === null
  ) {
    throw new Error('Invalid canvas JSON')
  }
  for (const n of parsed.nodes) {
    if (typeof n.id !== 'string' || typeof n.x !== 'number' || typeof n.y !== 'number') {
      throw new Error('Invalid canvas JSON')
    }
  }
  for (const e of parsed.edges) {
    if (typeof e.from !== 'string' || typeof e.to !== 'string') {
      throw new Error('Invalid canvas JSON')
    }
  }
  const vp = parsed.viewport
  if (typeof vp.x !== 'number' || typeof vp.y !== 'number' || typeof vp.zoom !== 'number') {
    throw new Error('Invalid canvas JSON')
  }
  return parsed as CanvasState
}

export function cloneCanvas(c: CanvasState): CanvasState {
  return {
    nodes: c.nodes.map((n) => ({ ...n })),
    edges: c.edges.map((e) => ({ ...e })),
    viewport: { ...c.viewport },
  }
}
