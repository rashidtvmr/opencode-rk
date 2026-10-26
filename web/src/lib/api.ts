export type SessionState = 'active' | 'archived'

export interface SessionSummary {
  id: string
  title: string
  state: SessionState
  created_at?: unknown
  updated_at?: unknown
  archived_at?: unknown | null
}

export interface ModelSummary {
  id?: string
  name?: string
  provider?: string
  provider_id?: string
  model_id?: string
  [key: string]: unknown
}

export interface HealthResponse {
  schema_version: number
  status: string
  runtime: string
}

export type MessageRole = 'system' | 'user' | 'assistant' | 'tool'

export interface StoredToolPart {
  type: 'tool_call' | 'tool_output'
  call_id: string
  name: string
  arguments?: string
  output?: string
  status?: 'pending' | 'completed'
  success?: boolean | null
}

export type MessageBody =
  | { storage: 'inline'; text: string }
  | { storage: 'blob'; hash: string; bytes: number }

export interface MessageRecord {
  id: string
  session_id: string
  role: MessageRole
  body: MessageBody
  parts?: StoredToolPart[]
  created_at?: unknown
}

export interface ForkProvenance {
  parent_session_id: string
  fork_message_seq: number
  boundary_message_id: string | null
}

export interface BranchResult {
  session: SessionSummary
  fork: ForkProvenance
}

export interface AssistantActivity {
  message_id: string
  reasoning_summary: string
}

export interface DraftAttachment {
  id: string
  session_id: string
  name: string
  mime: string
  hash: string
  bytes: number
  created_at?: unknown
}

export interface DraftAttachmentState {
  attachments: DraftAttachment[]
  available: boolean
  reason?: string
}

export interface WebToolCapability {
  id: string
  name: string
  description: string
  enabled: boolean
  type?: string | null
  tags: string[]
  available_for_web_turn: boolean
  reason: string
}

export interface WebCapabilities {
  tools: WebToolCapability[]
  plugins: { available_for_web_turn: boolean; reason: string }
  approvals: { available_for_web_turn: boolean; reason: string }
  attachments: {
    draft_ingest: boolean
    available_for_web_turn: boolean
    reason: string
  }
  search: { available: boolean; reason: string }
  deep_research: { available: boolean; reason: string }
  voice: { available: boolean; reason: string }
  artifacts: { available: boolean; reason: string }
}

export interface ConfiguredCommand { name: string; description: string; agent: string | null; model: string | null }
export interface AgentMention { name: string; description: string; read_only: boolean }
export interface CommandInventory { commands: ConfiguredCommand[]; agents: AgentMention[] }

export interface SubagentSnapshot {
  session_id: string
  parent_id: string
  description: string
  agent: string
  model: string
  state: string
  output_tail: string
  error: string | null
  read_only: boolean
  has_tool_calls: boolean
  generation: number
  auto_retry_attempts: number
  manual_retries: number
  pending_followups: number
  awaiting_permission: boolean
}

function normalizeSubagent(value: unknown): SubagentSnapshot | null {
  if (!value || typeof value !== 'object') return null
  const raw = value as Record<string, unknown>
  if (typeof raw.session_id !== 'string' || typeof raw.parent_id !== 'string') return null
  return {
    session_id: raw.session_id,
    parent_id: raw.parent_id,
    description: typeof raw.description === 'string' ? raw.description : '',
    agent: typeof raw.agent === 'string' ? raw.agent : '',
    model: typeof raw.model === 'string' ? raw.model : '',
    state: typeof raw.state === 'string' ? raw.state : 'unknown',
    output_tail: typeof raw.output_tail === 'string' ? raw.output_tail.slice(-8192) : '',
    error: typeof raw.error === 'string' ? raw.error : null,
    read_only: raw.read_only === true,
    has_tool_calls: raw.has_tool_calls === true,
    generation: typeof raw.generation === 'number' ? raw.generation : 0,
    auto_retry_attempts: typeof raw.auto_retry_attempts === 'number' ? raw.auto_retry_attempts : 0,
    manual_retries: typeof raw.manual_retries === 'number' ? raw.manual_retries : 0,
    pending_followups: typeof raw.pending_followups === 'number' ? raw.pending_followups : 0,
    awaiting_permission: raw.awaiting_permission === true,
  }
}

export interface WorkspaceSummary {
  id: string
  label: string
  project_root: string | null
  status: number
  created_at_us: number
}

export interface WorkspaceCatalogState {
  available: boolean
  workspaces: WorkspaceSummary[]
  session_scope_available: boolean
  memory_available: boolean
  reason?: string
}

export type ArtifactKind = 'writing' | 'code'

export interface ArtifactSummary {
  id: string
  session_id: string
  source_message_id: string
  kind: ArtifactKind
  title: string
  language: string | null
  current_version: number
  created_at?: unknown
  updated_at?: unknown
}

export interface ArtifactVersion {
  version: number
  bytes: number
  created_at?: unknown
}

export interface ArtifactDocument extends ArtifactSummary {
  content: string
  versions: ArtifactVersion[]
}

export interface ArtifactCatalogState {
  available: boolean
  artifacts: ArtifactSummary[]
  runAvailable: boolean
  applyAvailable: boolean
  reason?: string
}

function unavailableCapabilities(reason: string): WebCapabilities {
  return {
    tools: [],
    plugins: { available_for_web_turn: false, reason },
    approvals: { available_for_web_turn: false, reason },
    attachments: { draft_ingest: false, available_for_web_turn: false, reason },
    search: { available: false, reason },
    deep_research: { available: false, reason },
    voice: { available: false, reason },
    artifacts: { available: false, reason },
  }
}

class ApiError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
}

const RESUME_STORAGE_KEY = 'opencode.browser-resume'
const RESUME_TOKEN = /^[A-Za-z0-9_-]{43}$/
let nativeLaunchToken: string | undefined
let browserCapability: string | undefined
let resumeReady: Promise<void> | undefined

function consumeNativeLaunchToken(): string | undefined {
  if (nativeLaunchToken || typeof window === 'undefined') return nativeLaunchToken
  const match = /^#oc2-token=([a-fA-F0-9]{64})$/.exec(window.location.hash)
  if (!match) return undefined
  const page = new URL(window.location.href)
  window.history.replaceState(window.history.state, '', `${page.pathname}${page.search}`)
  nativeLaunchToken = match[1]
  return nativeLaunchToken
}

function storeBrowserCapability(value: unknown): string {
  const capability = typeof value === 'string' ? value : ''
  if (!RESUME_TOKEN.test(capability)) throw new ApiError(403, 'Browser resume credential rejected')
  window.sessionStorage.setItem(RESUME_STORAGE_KEY, capability)
  browserCapability = capability
  return capability
}

async function establishBrowserCapability(): Promise<void> {
  if (typeof window === 'undefined' || browserCapability) return
  if (resumeReady) return resumeReady
  resumeReady = (async () => {
    try {
      const native = consumeNativeLaunchToken()
      if (native) {
        try {
          const response = await fetch('/auth/browser-resume', {
            method: 'POST',
            headers: { authorization: `Bearer ${native}`, 'content-type': 'application/json' },
            body: JSON.stringify({}),
          })
          if (!response.ok) throw new ApiError(response.status, 'Unable to establish browser resume')
          const issued = (await response.json()) as { browser_capability?: unknown }
          storeBrowserCapability(issued.browser_capability)
          return
        } finally {
          nativeLaunchToken = undefined
        }
      }
      const stored = window.sessionStorage.getItem(RESUME_STORAGE_KEY)
      if (!stored || !RESUME_TOKEN.test(stored)) {
        window.sessionStorage.removeItem(RESUME_STORAGE_KEY)
        throw new ApiError(401, 'Re-authentication required: relaunch the authenticated browser session')
      }
      const response = await fetch('/auth/browser-resume/exchange', {
        method: 'POST',
        headers: { authorization: `BrowserResume ${stored}`, 'content-type': 'application/json' },
        body: JSON.stringify({}),
      })
      if (!response.ok) {
        window.sessionStorage.removeItem(RESUME_STORAGE_KEY)
        throw new ApiError(403, 'Browser resume credential expired or revoked')
      }
      const exchanged = (await response.json()) as { browser_capability?: unknown }
      storeBrowserCapability(exchanged.browser_capability)
    } finally {
      resumeReady = undefined
    }
  })()
  return resumeReady
}

function clearBrowserCapability(): void {
  if (typeof window !== 'undefined') window.sessionStorage.removeItem(RESUME_STORAGE_KEY)
  browserCapability = undefined
}

async function clearRejectedBrowserCapability(headers: Headers, response: Response): Promise<void> {
  if (!headers.get('authorization')?.startsWith('BrowserResume ') || (response.status !== 401 && response.status !== 403)) return
  const body = (await response.clone().json().catch(() => null)) as { error?: unknown } | null
  if (body?.error === 'bad bearer credential' || body?.error === 'missing bearer credential') clearBrowserCapability()
}

async function apiHeaders(path: string, headers?: HeadersInit): Promise<Headers> {
  const result = new Headers(headers)
  if (path.startsWith('/api/') && typeof window !== 'undefined') {
    await establishBrowserCapability()
    if (!browserCapability) throw new ApiError(401, 'Re-authentication required: relaunch the authenticated browser session')
    result.set('authorization', `BrowserResume ${browserCapability}`)
  }
  return result
}

export async function revokeBrowserResume(): Promise<void> {
  if (typeof window === 'undefined') return
  if (resumeReady) await resumeReady.catch(() => undefined)
  const capability = browserCapability ?? window.sessionStorage.getItem(RESUME_STORAGE_KEY)
  window.sessionStorage.removeItem(RESUME_STORAGE_KEY)
  browserCapability = undefined
  if (capability && RESUME_TOKEN.test(capability)) {
    await fetch('/auth/browser-resume', {
      method: 'DELETE',
      headers: { authorization: `BrowserResume ${capability}` },
    }).catch(() => undefined)
  }
}

async function authenticatedFetch(path: string, init?: RequestInit): Promise<Response> {
  const headers = await apiHeaders(path, init?.headers)
  const response = await fetch(path, { ...init, headers })
  await clearRejectedBrowserCapability(headers, response)
  return response
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const headers = new Headers(init?.headers)
  if (!headers.has('content-type')) headers.set('content-type', 'application/json')
  const response = await authenticatedFetch(path, { ...init, headers })

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as
      | { message?: string }
      | null
    throw new ApiError(response.status, body?.message ?? `Request failed with ${response.status}`)
  }

  if (response.status === 204) return undefined as T
  return (await response.json()) as T
}

export type PermissionReply = 'once' | 'always' | 'reject'
export interface PermissionRequest {
  id: string
  sessionID: string
  action: string
  resources: string[]
  source: { type: string; messageID: string; callID: string }
}

function normalizePermission(value: unknown): PermissionRequest | null {
  if (!value || typeof value !== 'object') return null
  const v = value as Record<string, unknown>
  const source = v.source
  const nonemptyBounded = (x: unknown, bytes: number): x is string => typeof x === 'string' && x.length > 0 && x.length <= bytes && new TextEncoder().encode(x).byteLength <= bytes
  if (!nonemptyBounded(v.id, 256) || !nonemptyBounded(v.sessionID, 256) || !nonemptyBounded(v.action, 64) || !Array.isArray(v.resources) || v.resources.length > 32 || !source || typeof source !== 'object') return null
  const s = source as Record<string, unknown>
  const resources = v.resources as unknown[]
  if (!resources.every((r) => nonemptyBounded(r, 4096)) || !nonemptyBounded(s.type, 32) || !nonemptyBounded(s.messageID, 256) || !nonemptyBounded(s.callID, 256)) return null
  return { id: v.id, sessionID: v.sessionID, action: v.action, resources: resources as string[], source: { type: s.type, messageID: s.messageID, callID: s.callID } }
}

export async function listPermissions(sessionId: string, signal?: AbortSignal): Promise<PermissionRequest[]> {
  const payload = await request<{ data?: unknown }>(`/api/session/${encodeURIComponent(sessionId)}/permission`, { signal })
  if (!Array.isArray(payload.data)) throw new Error('Server returned invalid pending permissions')
  const bounded = payload.data.slice(0, 16)
  if (payload.data.length > 16) throw new Error('Server returned too many pending permission requests')
  const permissions = bounded.map(normalizePermission)
  if (permissions.some((p) => p === null)) throw new Error('Server returned invalid pending permission')
  return permissions as PermissionRequest[]
}

export async function replyPermission(sessionId: string, requestId: string, reply: PermissionReply, signal?: AbortSignal): Promise<void> {
  await request<void>(`/api/session/${encodeURIComponent(sessionId)}/permission/${encodeURIComponent(requestId)}/reply`, { method: 'POST', body: JSON.stringify({ reply }), signal })
}

function normalizeSession(value: unknown): SessionSummary | null {
  if (!value || typeof value !== 'object') return null
  const candidate = value as Record<string, unknown>
  if (typeof candidate.id !== 'string' || typeof candidate.title !== 'string') return null

  const state: SessionState =
    candidate.state === 'archived' || candidate.archived === true ? 'archived' : 'active'

  return {
    id: candidate.id,
    title: candidate.title,
    state,
    created_at: candidate.created_at,
    updated_at: candidate.updated_at,
    archived_at: candidate.archived_at ?? null,
  }
}

function normalizeMessage(value: unknown): MessageRecord | null {
  if (!value || typeof value !== 'object') return null
  const candidate = value as Record<string, unknown>
  if (
    typeof candidate.id !== 'string' ||
    typeof candidate.session_id !== 'string' ||
    !['system', 'user', 'assistant', 'tool'].includes(String(candidate.role)) ||
    !candidate.body ||
    typeof candidate.body !== 'object'
  ) {
    return null
  }

  const rawBody = candidate.body as Record<string, unknown>
  let body: MessageBody
  if (rawBody.storage === 'inline' && typeof rawBody.text === 'string') {
    body = { storage: 'inline', text: rawBody.text }
  } else if (
    rawBody.storage === 'blob' &&
    typeof rawBody.hash === 'string' &&
    typeof rawBody.bytes === 'number'
  ) {
    body = { storage: 'blob', hash: rawBody.hash, bytes: rawBody.bytes }
  } else {
    return null
  }

  const parts: StoredToolPart[] = []
  if (Array.isArray(candidate.parts)) {
    for (const element of candidate.parts.slice(0, 100)) {
      if (!element || typeof element !== 'object') continue
      const p = element as Record<string, unknown>
      if (!['tool_call', 'tool_output'].includes(String(p.type))
        || typeof p.call_id !== 'string' || p.call_id.length > 256
        || typeof p.name !== 'string' || p.name.length > 128) continue
      if (p.type === 'tool_call') {
        const args = typeof p.arguments === 'string' ? p.arguments : JSON.stringify(p.arguments ?? {})
        parts.push({
          type: 'tool_call', call_id: p.call_id, name: p.name,
          arguments: args.slice(0, 65536),
          status: p.status === 'completed' ? 'completed' : 'pending',
        })
      } else {
        parts.push({
          type: 'tool_output', call_id: p.call_id, name: p.name,
          output: typeof p.output === 'string' ? p.output.slice(0, 131072) : '',
          success: typeof p.success === 'boolean' ? p.success : null,
        })
      }
    }
  }

  return {
    id: candidate.id,
    session_id: candidate.session_id,
    role: candidate.role as MessageRole,
    body,
    ...(parts.length ? { parts } : {}),
    created_at: candidate.created_at,
  }
}

function normalizeDraftAttachment(value: unknown): DraftAttachment | null {
  if (!value || typeof value !== 'object') return null
  const candidate = value as Record<string, unknown>
  if (
    typeof candidate.id !== 'string' ||
    typeof candidate.session_id !== 'string' ||
    typeof candidate.name !== 'string' ||
    typeof candidate.mime !== 'string' ||
    typeof candidate.hash !== 'string' ||
    typeof candidate.bytes !== 'number'
  ) {
    return null
  }
  return {
    id: candidate.id,
    session_id: candidate.session_id,
    name: candidate.name,
    mime: candidate.mime,
    hash: candidate.hash,
    bytes: candidate.bytes,
    created_at: candidate.created_at,
  }
}

function normalizeArtifactSummary(value: unknown): ArtifactSummary | null {
  if (!value || typeof value !== 'object') return null
  const candidate = value as Record<string, unknown>
  if (
    typeof candidate.id !== 'string' ||
    typeof candidate.session_id !== 'string' ||
    typeof candidate.source_message_id !== 'string' ||
    (candidate.kind !== 'writing' && candidate.kind !== 'code') ||
    typeof candidate.title !== 'string' ||
    !(candidate.language === null || typeof candidate.language === 'string') ||
    typeof candidate.current_version !== 'number'
  ) {
    return null
  }
  return {
    id: candidate.id,
    session_id: candidate.session_id,
    source_message_id: candidate.source_message_id,
    kind: candidate.kind,
    title: candidate.title,
    language: candidate.language,
    current_version: candidate.current_version,
    created_at: candidate.created_at,
    updated_at: candidate.updated_at,
  }
}

function normalizeArtifactDocument(value: unknown): ArtifactDocument | null {
  const summary = normalizeArtifactSummary(value)
  if (!summary || !value || typeof value !== 'object') return null
  const candidate = value as Record<string, unknown>
  if (typeof candidate.content !== 'string' || !Array.isArray(candidate.versions)) return null
  const versions = candidate.versions.map((version) => {
    if (!version || typeof version !== 'object') return null
    const row = version as Record<string, unknown>
    if (typeof row.version !== 'number' || typeof row.bytes !== 'number') return null
    return {
      version: row.version,
      bytes: row.bytes,
      created_at: row.created_at,
    } satisfies ArtifactVersion
  })
  if (versions.some((version) => version === null)) return null
  return {
    ...summary,
    content: candidate.content,
    versions: versions as ArtifactVersion[],
  }
}

export async function getHealth() {
  return request<HealthResponse>('/health')
}

export async function getWebCapabilities() {
  try {
    const payload = await request<Record<string, unknown>>('/api/capabilities')
    const field = (name: string) => {
      const value = payload[name]
      if (!value || typeof value !== 'object') throw new Error('Server returned invalid capabilities')
      return value as Record<string, unknown>
    }
    const tools = Array.isArray(payload.tools)
      ? payload.tools.map((value) => {
          if (!value || typeof value !== 'object') throw new Error('Server returned invalid tool capability')
          const candidate = value as Record<string, unknown>
          if (
            typeof candidate.id !== 'string' ||
            typeof candidate.name !== 'string' ||
            typeof candidate.description !== 'string' ||
            typeof candidate.enabled !== 'boolean' ||
            typeof candidate.available_for_web_turn !== 'boolean' ||
            typeof candidate.reason !== 'string' ||
            !Array.isArray(candidate.tags) ||
            candidate.tags.some((tag) => typeof tag !== 'string')
          ) {
            throw new Error('Server returned invalid tool capability')
          }
          return {
            id: candidate.id,
            name: candidate.name,
            description: candidate.description,
            enabled: candidate.enabled,
            type: typeof candidate.type === 'string' ? candidate.type : null,
            tags: candidate.tags as string[],
            available_for_web_turn: candidate.available_for_web_turn,
            reason: candidate.reason,
          } satisfies WebToolCapability
        })
      : []
    const booleanCapability = (name: string, key: string) => {
      const value = field(name)
      if (typeof value[key] !== 'boolean' || typeof value.reason !== 'string') {
        throw new Error('Server returned invalid capabilities')
      }
      return { available: value[key] as boolean, reason: value.reason }
    }
    const plugins = field('plugins')
    const approvals = field('approvals')
    const attachments = field('attachments')
    if (
      typeof plugins.available_for_web_turn !== 'boolean' ||
      typeof plugins.reason !== 'string' ||
      typeof approvals.available_for_web_turn !== 'boolean' ||
      typeof approvals.reason !== 'string' ||
      typeof attachments.draft_ingest !== 'boolean' ||
      typeof attachments.available_for_web_turn !== 'boolean' ||
      typeof attachments.reason !== 'string'
    ) {
      throw new Error('Server returned invalid capabilities')
    }
    return {
      tools,
      plugins: {
        available_for_web_turn: plugins.available_for_web_turn,
        reason: plugins.reason,
      },
      approvals: {
        available_for_web_turn: approvals.available_for_web_turn,
        reason: approvals.reason,
      },
      attachments: {
        draft_ingest: attachments.draft_ingest,
        available_for_web_turn: attachments.available_for_web_turn,
        reason: attachments.reason,
      },
      search: booleanCapability('search', 'available'),
      deep_research: booleanCapability('deep_research', 'available'),
      voice: booleanCapability('voice', 'available'),
      artifacts: booleanCapability('artifacts', 'available'),
    } satisfies WebCapabilities
  } catch (cause) {
    if (cause instanceof ApiError && cause.status === 404) {
      return unavailableCapabilities('This native server does not expose web capability discovery.')
    }
    throw cause
  }
}

export async function listWorkspaces(): Promise<WorkspaceCatalogState> {
  try {
    const payload = await request<Record<string, unknown>>('/api/workspaces')
    if (
      typeof payload.available !== 'boolean' ||
      !Array.isArray(payload.workspaces) ||
      typeof payload.session_scope_available !== 'boolean' ||
      typeof payload.memory_available !== 'boolean'
    ) {
      throw new Error('Server returned invalid workspace catalog state')
    }
    const workspaces = payload.workspaces.map((value) => {
      if (!value || typeof value !== 'object') {
        throw new Error('Server returned invalid workspace metadata')
      }
      const candidate = value as Record<string, unknown>
      if (
        typeof candidate.id !== 'string' ||
        typeof candidate.label !== 'string' ||
        !(candidate.project_root === null || typeof candidate.project_root === 'string') ||
        typeof candidate.status !== 'number' ||
        typeof candidate.created_at_us !== 'number'
      ) {
        throw new Error('Server returned invalid workspace metadata')
      }
      return {
        id: candidate.id,
        label: candidate.label,
        project_root: candidate.project_root,
        status: candidate.status,
        created_at_us: candidate.created_at_us,
      } satisfies WorkspaceSummary
    })
    return {
      available: payload.available,
      workspaces,
      session_scope_available: payload.session_scope_available,
      memory_available: payload.memory_available,
      reason: typeof payload.reason === 'string' ? payload.reason : undefined,
    }
  } catch (cause) {
    if (cause instanceof ApiError && cause.status === 404) {
      return {
        available: false,
        workspaces: [],
        session_scope_available: false,
        memory_available: false,
        reason: 'This native server does not expose workspace catalog metadata.',
      }
    }
    throw cause
  }
}

export async function listSessions() {
  const payload = await request<{ sessions?: unknown[] }>('/api/sessions')
  return (payload.sessions ?? []).map(normalizeSession).filter((session): session is SessionSummary => Boolean(session))
}

export async function createSession(title: string) {
  const payload = await request<{ session: unknown }>('/api/sessions', {
    method: 'POST',
    body: JSON.stringify({ title }),
  })
  const session = normalizeSession(payload.session)
  if (!session) throw new Error('Server returned an invalid session')
  return session
}

export async function renameSession(id: string, title: string) {
  const payload = await request<{ session: unknown }>(`/api/sessions/${encodeURIComponent(id)}`, {
    method: 'PATCH',
    body: JSON.stringify({ title }),
  })
  const session = normalizeSession(payload.session)
  if (!session) throw new Error('Server returned an invalid session')
  return session
}

export async function archiveSession(id: string) {
  const payload = await request<{ session: unknown }>(
    `/api/sessions/${encodeURIComponent(id)}/archive`,
    { method: 'POST' },
  )
  const session = normalizeSession(payload.session)
  if (!session) throw new Error('Server returned an invalid session')
  return session
}

export async function listMessages(id: string, limit = 200, signal?: AbortSignal) {
  const boundedLimit = Math.max(1, Math.min(500, Math.trunc(limit)))
  const payload = await request<{ messages?: unknown[] }>(
    `/api/sessions/${encodeURIComponent(id)}/messages?limit=${boundedLimit}`,
    { signal },
  )
  return (payload.messages ?? [])
    .map(normalizeMessage)
    .filter((message): message is MessageRecord => Boolean(message))
}

export interface HistoryPage {
  messages: MessageRecord[]
  nextBefore: string | null
}

export async function listHistoryPage(
  id: string,
  limit = 50,
  before?: string | null,
  signal?: AbortSignal,
): Promise<HistoryPage> {
  const boundedLimit = Math.max(1, Math.min(100, Math.trunc(limit)))
  const params = new URLSearchParams({ limit: String(boundedLimit) })
  if (before) params.set('before', before)
  try {
    const payload = await request<{ messages?: unknown[]; next_before?: unknown }>(
      `/api/sessions/${encodeURIComponent(id)}/history?${params.toString()}`,
      { signal },
    )
    const messages = (payload.messages ?? [])
      .map(normalizeMessage)
      .filter((message): message is MessageRecord => Boolean(message))
    if (payload.next_before != null && typeof payload.next_before !== 'string') {
      throw new Error('Server returned an invalid history cursor')
    }
    return { messages, nextBefore: payload.next_before ?? null }
  } catch (cause) {
    // Older native servers without the /history endpoint still serve the
    // durable flat message list. The fallback always requests the canonical
    // 200-message page so the URL matches the documented contract.
    if (cause instanceof ApiError && cause.status === 404 && !before) {
      return { messages: await listMessages(id, 200, signal), nextBefore: null }
    }
    throw cause
  }
}

export async function listDraftAttachments(id: string, signal?: AbortSignal) {
  try {
    const payload = await request<{
      attachments?: unknown[]
      available?: unknown
      reason?: unknown
    }>(
      `/api/sessions/${encodeURIComponent(id)}/attachments`,
      { signal },
    )
    const attachments = (payload.attachments ?? [])
      .map(normalizeDraftAttachment)
      .filter((attachment): attachment is DraftAttachment => Boolean(attachment))
    return {
      attachments,
      available: typeof payload.available === 'boolean' ? payload.available : true,
      reason: typeof payload.reason === 'string' ? payload.reason : undefined,
    } satisfies DraftAttachmentState
  } catch (cause) {
    if (cause instanceof ApiError && cause.status === 404) {
      return {
        attachments: [],
        available: false,
        reason: 'Draft attachments require a newer native server.',
      } satisfies DraftAttachmentState
    }
    throw cause
  }
}

export async function uploadDraftAttachment(
  id: string,
  file: File,
  signal?: AbortSignal,
) {
  const params = new URLSearchParams({ name: file.name || 'attachment' })
  const path = `/api/sessions/${encodeURIComponent(id)}/attachments?${params.toString()}`
  const response = await authenticatedFetch(path, {
    method: 'POST',
    headers: { 'content-type': file.type || 'application/octet-stream' },
    body: file,
    signal,
  })
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { message?: string } | null
    throw new ApiError(response.status, body?.message ?? `Request failed with ${response.status}`)
  }
  const payload = (await response.json()) as { attachment?: unknown }
  const attachment = normalizeDraftAttachment(payload.attachment)
  if (!attachment) throw new Error('Server returned an invalid attachment')
  return attachment
}

export async function deleteDraftAttachment(
  sessionId: string,
  attachmentId: string,
  signal?: AbortSignal,
) {
  const path = `/api/sessions/${encodeURIComponent(sessionId)}/attachments/${encodeURIComponent(attachmentId)}`
  const response = await authenticatedFetch(path, { method: 'DELETE', signal })
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { message?: string } | null
    throw new ApiError(response.status, body?.message ?? `Request failed with ${response.status}`)
  }
}

const MAX_ARTIFACT_CONTENT_BYTES = 64 * 1024

function assertArtifactContent(content: string) {
  if (new TextEncoder().encode(content).byteLength > MAX_ARTIFACT_CONTENT_BYTES) {
    throw new Error('Artifact content exceeds the 64 KiB editor limit')
  }
}

export async function listArtifacts(
  sessionId: string,
  signal?: AbortSignal,
): Promise<ArtifactCatalogState> {
  try {
    const payload = await request<Record<string, unknown>>(
      `/api/sessions/${encodeURIComponent(sessionId)}/artifacts`,
      { signal },
    )
    if (!Array.isArray(payload.artifacts)) throw new Error('Server returned invalid artifact catalog')
    const artifacts = payload.artifacts.map(normalizeArtifactSummary)
    if (artifacts.some((artifact) => artifact === null)) {
      throw new Error('Server returned invalid artifact metadata')
    }
    return {
      available: typeof payload.available === 'boolean' ? payload.available : true,
      artifacts: artifacts as ArtifactSummary[],
      runAvailable: payload.run_available === true,
      applyAvailable: payload.apply_available === true,
      reason: typeof payload.reason === 'string' ? payload.reason : undefined,
    }
  } catch (cause) {
    if (cause instanceof ApiError && cause.status === 404) {
      return {
        available: false,
        artifacts: [],
        runAvailable: false,
        applyAvailable: false,
        reason: 'Editable artifacts require a newer native server.',
      }
    }
    throw cause
  }
}

export async function createArtifact(
  sessionId: string,
  messageId: string,
  input: { kind: ArtifactKind; title: string; language: string | null; content: string },
) {
  assertArtifactContent(input.content)
  const payload = await request<{ artifact?: unknown }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/messages/${encodeURIComponent(messageId)}/artifacts`,
    { method: 'POST', body: JSON.stringify(input) },
  )
  const artifact = normalizeArtifactDocument(payload.artifact)
  if (!artifact) throw new Error('Server returned an invalid artifact')
  return artifact
}

export async function getArtifact(sessionId: string, artifactId: string, signal?: AbortSignal) {
  const payload = await request<{ artifact?: unknown }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/artifacts/${encodeURIComponent(artifactId)}`,
    { signal },
  )
  const artifact = normalizeArtifactDocument(payload.artifact)
  if (!artifact) throw new Error('Server returned an invalid artifact')
  return artifact
}

export async function saveArtifactVersion(sessionId: string, artifactId: string, content: string) {
  assertArtifactContent(content)
  const payload = await request<{ artifact?: unknown }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/artifacts/${encodeURIComponent(artifactId)}/versions`,
    { method: 'POST', body: JSON.stringify({ content }) },
  )
  const artifact = normalizeArtifactDocument(payload.artifact)
  if (!artifact) throw new Error('Server returned an invalid artifact')
  return artifact
}

export async function branchSessionFromMessage(sessionId: string, messageId: string) {
  const payload = await request<{ session: unknown; fork?: unknown }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/messages/${encodeURIComponent(messageId)}/branch`,
    { method: 'POST', body: '{}' },
  )
  const session = normalizeSession(payload.session)
  if (!session) throw new Error('Server returned an invalid branch session')
  if (!payload.fork || typeof payload.fork !== 'object') {
    throw new Error('Server returned invalid branch provenance')
  }
  const fork = payload.fork as Record<string, unknown>
  if (
    typeof fork.parent_session_id !== 'string' ||
    typeof fork.fork_message_seq !== 'number' ||
    typeof fork.boundary_message_id !== 'string'
  ) {
    throw new Error('Server returned invalid branch provenance')
  }
  return {
    session,
    fork: {
      parent_session_id: fork.parent_session_id,
      fork_message_seq: fork.fork_message_seq,
      boundary_message_id: fork.boundary_message_id,
    },
  } satisfies BranchResult
}

export interface RetryBranchResult extends BranchResult {
  requestText: string
  triggerMessageId: string
}

export async function prepareRetryBranch(sessionId: string, messageId: string) {
  const payload = await request<{
    session: unknown
    fork?: unknown
    request_text?: unknown
    trigger_message_id?: unknown
  }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/messages/${encodeURIComponent(messageId)}/retry-branch`,
    { method: 'POST', body: '{}' },
  )
  const session = normalizeSession(payload.session)
  if (!session) throw new Error('Server returned an invalid retry branch session')
  if (!payload.fork || typeof payload.fork !== 'object') {
    throw new Error('Server returned invalid retry branch provenance')
  }
  const fork = payload.fork as Record<string, unknown>
  if (
    typeof fork.parent_session_id !== 'string' ||
    typeof fork.fork_message_seq !== 'number' ||
    (fork.boundary_message_id !== null && typeof fork.boundary_message_id !== 'string') ||
    typeof payload.request_text !== 'string' ||
    typeof payload.trigger_message_id !== 'string'
  ) {
    throw new Error('Server returned invalid retry branch data')
  }
  return {
    session,
    fork: {
      parent_session_id: fork.parent_session_id,
      fork_message_seq: fork.fork_message_seq,
      boundary_message_id: fork.boundary_message_id as string | null,
    },
    requestText: payload.request_text,
    triggerMessageId: payload.trigger_message_id,
  } satisfies RetryBranchResult
}

export async function getForkProvenance(sessionId: string, signal?: AbortSignal) {
  try {
    const payload = await request<{ fork?: unknown }>(
      `/api/sessions/${encodeURIComponent(sessionId)}/fork`,
      { signal },
    )
    if (payload.fork == null) return null
    if (typeof payload.fork !== 'object') throw new Error('Server returned invalid fork provenance')
    const fork = payload.fork as Record<string, unknown>
    if (
      typeof fork.parent_session_id !== 'string' ||
      typeof fork.fork_message_seq !== 'number' ||
      (fork.boundary_message_id !== null && typeof fork.boundary_message_id !== 'string')
    ) {
      throw new Error('Server returned invalid fork provenance')
    }
    return {
      parent_session_id: fork.parent_session_id,
      fork_message_seq: fork.fork_message_seq,
      boundary_message_id: fork.boundary_message_id,
    } satisfies ForkProvenance
  } catch (cause) {
    if (cause instanceof ApiError && cause.status === 404) return null
    throw cause
  }
}

const MAX_REASONING_SUMMARY_BYTES = 8 * 1024

export async function listAssistantActivity(sessionId: string, limit = 200, signal?: AbortSignal) {
  const boundedLimit = Math.max(1, Math.min(500, Math.trunc(limit)))
  try {
    const payload = await request<{ activity?: unknown[] }>(
      `/api/sessions/${encodeURIComponent(sessionId)}/activity?limit=${boundedLimit}`,
      { signal },
    )
    const encoder = new TextEncoder()
    return (payload.activity ?? []).map((value) => {
      if (!value || typeof value !== 'object') {
        throw new Error('Server returned invalid assistant activity')
      }
      const candidate = value as Record<string, unknown>
      if (
        typeof candidate.message_id !== 'string' ||
        typeof candidate.reasoning_summary !== 'string' ||
        encoder.encode(candidate.reasoning_summary).byteLength > MAX_REASONING_SUMMARY_BYTES
      ) {
        throw new Error('Server returned invalid assistant activity')
      }
      return {
        message_id: candidate.message_id,
        reasoning_summary: candidate.reasoning_summary,
      } satisfies AssistantActivity
    })
  } catch (cause) {
    // Older native servers do not expose structured assistant activity yet.
    if (cause instanceof ApiError && cause.status === 404) return []
    throw cause
  }
}

export async function appendMessage(id: string, text: string, signal?: AbortSignal) {
  const payload = await request<{ message: unknown }>(
    `/api/sessions/${encodeURIComponent(id)}/messages`,
    {
      method: 'POST',
      body: JSON.stringify({ text }),
      signal,
    },
  )
  const message = normalizeMessage(payload.message)
  if (!message) throw new Error('Server returned an invalid message')
  return message
}

export interface TurnResult {
  userMessage: MessageRecord
  assistantMessage: MessageRecord | null
  executed: boolean
}

export interface TurnStreamHandlers {
  onUserMessage?: (message: MessageRecord) => void
  onToolCall?: (toolCall: ToolCall) => void
  onToolOutput?: (toolOutput: ToolOutput) => void
  onReasoningSummaryDelta?: (delta: string) => void
  onAssistantDelta?: (delta: string) => void
  onAssistantActivity?: (activity: AssistantActivity) => void
  onAssistantMessage?: (message: MessageRecord) => void
}

/** A transient tool invocation reported by the native turn stream. */
export interface ToolCall {
  call_id: string
  name: string
  arguments: string
}

/** A transient tool result reported by the native turn stream. */
export interface ToolOutput {
  call_id: string
  name: string
  output: string
  success?: boolean
}

const MAX_TURN_STREAM_BYTES = 2 * 1024 * 1024
const MAX_TURN_STREAM_LINE_BYTES = 256 * 1024

function streamError(message: string) {
  return new Error(message)
}

function parseTurnStreamEvent(
  line: string,
  handlers: TurnStreamHandlers,
  state: {
    userMessage: MessageRecord | null
    assistantMessage: MessageRecord | null
    assistantText: string
    reasoningSummary: string
  },
) {
  let value: unknown
  try {
    value = JSON.parse(line)
  } catch {
    throw streamError('Server returned invalid turn stream data')
  }
  if (!value || typeof value !== 'object') {
    throw streamError('Server returned invalid turn stream data')
  }

  const event = value as Record<string, unknown>
  switch (event.type) {
    case 'user_message': {
      const message = normalizeMessage(event.message)
      if (!message || message.role !== 'user') {
        throw streamError('Server returned an invalid streamed user message')
      }
      if (state.userMessage) {
        throw streamError('Server returned duplicate streamed user messages')
      }
      state.userMessage = message
      handlers.onUserMessage?.(message)
      break
    }
    case 'reasoning_summary_delta': {
      if (typeof event.delta !== 'string') {
        throw streamError('Server returned an invalid reasoning summary delta')
      }
      state.reasoningSummary += event.delta
      if (new TextEncoder().encode(state.reasoningSummary).byteLength > MAX_REASONING_SUMMARY_BYTES) {
        throw streamError('Reasoning summary exceeded the browser safety limit')
      }
      handlers.onReasoningSummaryDelta?.(event.delta)
      break
    }
    case 'tool_call': {
      if (
        typeof event.call_id !== 'string' ||
        typeof event.name !== 'string' ||
        typeof event.arguments !== 'string'
      ) {
        throw streamError('Server returned an invalid tool call')
      }
      handlers.onToolCall?.({
        call_id: event.call_id,
        name: event.name,
        arguments: event.arguments,
      })
      break
    }
    case 'tool_output': {
      if (
        typeof event.call_id !== 'string' ||
        typeof event.name !== 'string' ||
        typeof event.output !== 'string'
      ) {
        throw streamError('Server returned an invalid tool output')
      }
      handlers.onToolOutput?.({
        call_id: event.call_id,
        name: event.name,
        output: event.output,
        ...(typeof event.success === 'boolean' ? { success: event.success } : {}),
      })
      break
    }
    case 'assistant_delta': {
      if (typeof event.delta !== 'string') {
        throw streamError('Server returned an invalid assistant delta')
      }
      state.assistantText += event.delta
      if (state.assistantText.length > MAX_TURN_STREAM_BYTES) {
        throw streamError('Assistant stream exceeded the browser safety limit')
      }
      handlers.onAssistantDelta?.(event.delta)
      break
    }
    case 'assistant_message': {
      const message = normalizeMessage(event.message)
      if (!message || message.role !== 'assistant' || message.body.storage !== 'inline') {
        throw streamError('Server returned an invalid streamed assistant message')
      }
      if (state.assistantMessage) {
        throw streamError('Server returned duplicate streamed assistant messages')
      }
      if (message.body.text !== state.assistantText) {
        throw streamError('Final assistant message did not match streamed output')
      }
      const rawSummary = event.reasoning_summary
      if (rawSummary !== undefined && rawSummary !== null && typeof rawSummary !== 'string') {
        throw streamError('Server returned an invalid final reasoning summary')
      }
      const reasoningSummary = typeof rawSummary === 'string' ? rawSummary : ''
      if (reasoningSummary !== state.reasoningSummary) {
        throw streamError('Final reasoning summary did not match streamed output')
      }
      if (new TextEncoder().encode(reasoningSummary).byteLength > MAX_REASONING_SUMMARY_BYTES) {
        throw streamError('Reasoning summary exceeded the browser safety limit')
      }
      state.assistantMessage = message
      if (reasoningSummary) {
        handlers.onAssistantActivity?.({
          message_id: message.id,
          reasoning_summary: reasoningSummary,
        })
      }
      handlers.onAssistantMessage?.(message)
      break
    }
    case 'error': {
      throw streamError(
        typeof event.message === 'string' && event.message.trim()
          ? event.message
          : 'Native server reported a turn stream error',
      )
    }
    default:
      throw streamError('Server returned an unknown turn stream event')
  }
}

export async function runTurn(
  id: string,
  text: string,
  model: string,
  reasoningEffort: string,
  signal?: AbortSignal,
): Promise<TurnResult> {
  try {
    const payload = await request<{ user_message: unknown; assistant_message: unknown }>(
      `/api/sessions/${encodeURIComponent(id)}/turns`,
      {
        method: 'POST',
        body: JSON.stringify({
          text,
          model,
          reasoning_effort: reasoningEffort,
        }),
        signal,
      },
    )
    const userMessage = normalizeMessage(payload.user_message)
    const assistantMessage = normalizeMessage(payload.assistant_message)
    if (!userMessage || !assistantMessage) throw new Error('Server returned an invalid turn')
    return { userMessage, assistantMessage, executed: true }
  } catch (cause) {
    // Older native servers support durable messages without turn execution.
    // Preserve that compatibility path while the web client and daemon may be
    // upgraded independently.
    if (cause instanceof ApiError && cause.status === 404) {
      const userMessage = await appendMessage(id, text, signal)
      return { userMessage, assistantMessage: null, executed: false }
    }
    throw cause
  }
}

export async function runTurnStream(
  id: string,
  text: string,
  model: string,
  reasoningEffort: string,
  handlers: TurnStreamHandlers = {},
  signal?: AbortSignal,
  configuredCommand?: { name: string; arguments: string },
  mentionedAgent?: string,
  nativeShellCommand?: string,
): Promise<TurnResult> {
  const path = configuredCommand
    ? `/api/sessions/${encodeURIComponent(id)}/command`
    : mentionedAgent
      ? `/api/sessions/${encodeURIComponent(id)}/agent/stream`
      : nativeShellCommand
        ? `/api/sessions/${encodeURIComponent(id)}/shell/stream`
      : `/api/sessions/${encodeURIComponent(id)}/turns/stream`
  const response = await authenticatedFetch(path, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify(nativeShellCommand
      ? { command: nativeShellCommand }
      : configuredCommand
        ? { command: configuredCommand.name, arguments: configuredCommand.arguments, model }
        : mentionedAgent
        ? { agent: mentionedAgent, text, model, reasoning_effort: reasoningEffort }
        : { text, model, reasoning_effort: reasoningEffort }),
    signal,
  })

  if (response.status === 404) {
    if (configuredCommand || mentionedAgent || nativeShellCommand) {
      throw new ApiError(404, 'Requested native action is unavailable on this daemon')
    }
    const turn = await runTurn(id, text, model, reasoningEffort, signal)
    handlers.onUserMessage?.(turn.userMessage)
    if (turn.assistantMessage) handlers.onAssistantMessage?.(turn.assistantMessage)
    return turn
  }

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { message?: string } | null
    throw new ApiError(response.status, body?.message ?? `Request failed with ${response.status}`)
  }
  if (!response.headers.get('content-type')?.toLowerCase().startsWith('application/x-ndjson')) {
    throw streamError('Native server returned an unsupported turn stream format')
  }
  if (!response.body) throw streamError('Native server returned an empty turn stream')

  const reader = response.body.getReader()
  const decoder = new TextDecoder()
  const encoder = new TextEncoder()
  let buffered = ''
  let receivedBytes = 0
  const state = {
    userMessage: null as MessageRecord | null,
    assistantMessage: null as MessageRecord | null,
    assistantText: '',
    reasoningSummary: '',
  }

  const consumeLine = (line: string) => {
    if (!line.trim()) return
    if (encoder.encode(line).byteLength > MAX_TURN_STREAM_LINE_BYTES) {
      throw streamError('Turn stream event exceeded the browser safety limit')
    }
    parseTurnStreamEvent(line, handlers, state)
  }

  try {
    while (true) {
      const { done, value } = await reader.read()
      if (done) break
      receivedBytes += value.byteLength
      if (receivedBytes > MAX_TURN_STREAM_BYTES) {
        throw streamError('Turn stream exceeded the browser safety limit')
      }

      buffered += decoder.decode(value, { stream: true })
      let newline = buffered.indexOf('\n')
      while (newline !== -1) {
        const line = buffered.slice(0, newline).replace(/\r$/, '')
        buffered = buffered.slice(newline + 1)
        consumeLine(line)
        newline = buffered.indexOf('\n')
      }
      if (buffered.length > MAX_TURN_STREAM_LINE_BYTES) {
        throw streamError('Turn stream event exceeded the browser safety limit')
      }
    }

    buffered += decoder.decode()
    if (buffered.trim()) consumeLine(buffered.replace(/\r$/, ''))
  } catch (cause) {
    await reader.cancel().catch(() => undefined)
    throw cause
  } finally {
    reader.releaseLock()
  }

  if (!state.userMessage) throw streamError('Turn stream ended before the user message was confirmed')
  if (!state.assistantMessage) {
    throw streamError('Turn stream ended before the assistant message was completed')
  }
  return {
    userMessage: state.userMessage,
    assistantMessage: state.assistantMessage,
    executed: true,
  }
}

export function runConfiguredCommandStream(
  sessionId: string,
  command: string,
  argumentsText: string,
  model: string,
  handlers: TurnStreamHandlers = {},
  signal?: AbortSignal,
): Promise<TurnResult> {
  if (!/^[^\s/\u0000-\u001f]{1,128}$/.test(command)) {
    return Promise.reject(new Error('Invalid slash command'))
  }
  return runTurnStream(sessionId, `/${command} ${argumentsText}`.trim(), model, 'high', handlers, signal, {
    name: command, arguments: argumentsText,
  })
}

export function runMentionedAgentStream(
  sessionId: string,
  agent: string,
  text: string,
  model: string,
  reasoningEffort: string,
  handlers: TurnStreamHandlers = {},
  signal?: AbortSignal,
): Promise<TurnResult> {
  if (!/^[^\s@\u0000-\u001f]{1,128}$/.test(agent)) return Promise.reject(new Error('Invalid agent mention'))
  return runTurnStream(sessionId, text, model, reasoningEffort, handlers, signal, undefined, agent)
}

export async function listCommandInventory(signal?: AbortSignal): Promise<CommandInventory> {
  const payload = await request<{ commands?: unknown; agents?: unknown }>('/api/commands', { signal })
  if (!Array.isArray(payload.commands) || !Array.isArray(payload.agents)) {
    throw new Error('Server returned an invalid command inventory')
  }
  const commands = payload.commands.flatMap((value): ConfiguredCommand[] => {
    if (!value || typeof value !== 'object') return []
    const item = value as Record<string, unknown>
    if (typeof item.name !== 'string' || !/^[^\s/\u0000-\u001f]{1,128}$/.test(item.name)) return []
    return [{ name: item.name, description: typeof item.description === 'string' ? item.description : '',
      agent: typeof item.agent === 'string' ? item.agent : null,
      model: typeof item.model === 'string' ? item.model : null }]
  })
  const agents = payload.agents.flatMap((value): AgentMention[] => {
    if (!value || typeof value !== 'object') return []
    const item = value as Record<string, unknown>
    if (typeof item.name !== 'string' || !/^[^\s@\u0000-\u001f]{1,128}$/.test(item.name)) return []
    return [{ name: item.name, description: typeof item.description === 'string' ? item.description : '',
      read_only: item.read_only === true }]
  })
  return { commands, agents }
}

export async function listSubagents(parentId: string, signal?: AbortSignal): Promise<{
  children: SubagentSnapshot[]
  registryScope: string
}> {
  const payload = await request<{ children?: unknown; registry_scope?: unknown }>(
    `/api/sessions/${encodeURIComponent(parentId)}/children`, { signal },
  )
  if (!Array.isArray(payload.children)) throw new Error('Server returned an invalid subagent list')
  return {
    children: payload.children.map(normalizeSubagent).filter((item): item is SubagentSnapshot => item !== null),
    registryScope: typeof payload.registry_scope === 'string' ? payload.registry_scope : '',
  }
}

export async function updateSubagent(
  parentId: string,
  childId: string,
  action: 'stop' | 'retry' | 'continue',
  text?: string,
): Promise<SubagentSnapshot> {
  const payload = await request<unknown>(
    `/api/sessions/${encodeURIComponent(parentId)}/children/${encodeURIComponent(childId)}/${action}`,
    { method: 'POST', body: JSON.stringify(action === 'continue' ? { text } : {}) },
  )
  const result = normalizeSubagent(payload)
  if (!result || result.session_id !== childId || result.parent_id !== parentId) {
    throw new Error('Server returned an invalid subagent update')
  }
  return result
}

export interface BrowserFileEntry {
  name: string
  path: string
  is_dir: boolean
  bytes: number
}

export type BrowserFileResult =
  | { type: 'directory'; path: string; entries: BrowserFileEntry[]; truncated: boolean; read_only: true }
  | { type: 'text'; path: string; text: string; bytes: number; read_only: true }

export interface SessionGitInfo {
  available: boolean
  root: string
  reason?: string
  status?: string[]
  worktrees: Array<{ path: string; head: string; branch: string; locked: boolean; prunable: boolean }>
  read_only?: true
}

export async function getSessionGit(sessionId: string, signal?: AbortSignal): Promise<SessionGitInfo> {
  const result = await request<SessionGitInfo>(`/api/sessions/${encodeURIComponent(sessionId)}/git`, { signal })
  if (typeof result.available !== 'boolean' || !Array.isArray(result.worktrees) || result.worktrees.length > 256) {
    throw new Error('Invalid Git inventory from daemon')
  }
  return result
}

export async function getSessionGitDiff(sessionId: string, path: string, signal?: AbortSignal): Promise<string> {
  if (!path || path.length > 1024 || path.startsWith('/') || path.includes('..') || path.includes('\\')) {
    throw new Error('Invalid Git diff file path')
  }
  const result = await request<{ diff: unknown; available: boolean }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/git/diff?path=${encodeURIComponent(path)}`,
    { signal },
  )
  if (!result.available) throw new Error('Git diff is unavailable for this session')
  if (typeof result.diff !== 'string' || result.diff.length > 256 * 1024) {
    throw new Error('Invalid or oversized Git diff')
  }
  return result.diff
}

export async function browseSessionFiles(sessionId: string, path = '.', signal?: AbortSignal): Promise<BrowserFileResult> {
  const result = await request<BrowserFileResult>(
    `/api/sessions/${encodeURIComponent(sessionId)}/files?path=${encodeURIComponent(path)}`, { signal },
  )
  if (result.type === 'text' && typeof result.text === 'string' && result.text.length <= 128 * 1024) return result
  if (result.type === 'directory' && Array.isArray(result.entries) && result.entries.length <= 200
    && result.entries.every((entry) => typeof entry.name === 'string' && typeof entry.path === 'string'
      && typeof entry.is_dir === 'boolean' && typeof entry.bytes === 'number')) return result
  throw new Error('Invalid or oversized file listing from daemon')
}

export async function searchSessionFiles(sessionId: string, query: string, signal?: AbortSignal): Promise<string[]> {
  if (!query || query.length > 128 || query.includes('..')) return []
  const payload = await request<{ results: unknown }>(
    `/api/sessions/${encodeURIComponent(sessionId)}/files/lookup?q=${encodeURIComponent(query)}`,
    { signal },
  )
  if (!Array.isArray(payload.results) || payload.results.length > 50
    || !payload.results.every((entry) => typeof entry === 'string' && entry.length <= 1024)) {
    throw new Error('Invalid file mention inventory')
  }
  return payload.results as string[]
}

export interface NativePermissionRule {
  tool: string
  decision: 'allow' | 'deny' | 'ask'
  scope: 'session' | 'always'
  session_id: string | null
  resource?: string | null
  context?: string | null
}

export async function listPermissionRules(sessionId?: string, signal?: AbortSignal): Promise<NativePermissionRule[]> {
  const query = sessionId ? `?session_id=${encodeURIComponent(sessionId)}` : ''
  const response = await request<{ rules: NativePermissionRule[] }>(`/api/permission-rules${query}`, { signal })
  if (!Array.isArray(response.rules) || response.rules.length > 256) throw new Error('Invalid permission rules response')
  return response.rules
}

export async function savePermissionRule(rule: Pick<NativePermissionRule, 'tool' | 'decision' | 'scope' | 'session_id'>): Promise<void> {
  if (!/^(bash|shell|read|write|edit|glob|grep|webfetch|websearch|task|todowrite|mcp_[A-Za-z0-9_]{1,60})$/.test(rule.tool)
    || !['allow', 'deny', 'ask'].includes(rule.decision)
    || !['session', 'always'].includes(rule.scope)
    || (rule.scope === 'session' && !rule.session_id)) {
    throw new Error('Invalid permission rule')
  }
  await request<void>('/api/permission-rules', { method: 'POST', body: JSON.stringify(rule) })
}

export interface NativeProvider {
  id: string
  name: string
  base_url: string
  buffered_model_ids: string[]
}

export async function listCustomProviders(signal?: AbortSignal): Promise<NativeProvider[]> {
  const response = await request<{ providers: NativeProvider[] }>('/api/providers', { signal })
  if (!Array.isArray(response.providers) || response.providers.length > 256) throw new Error('Invalid provider inventory')
  return response.providers.filter((p) => typeof p?.id === 'string' && typeof p?.name === 'string')
}

export async function saveCustomProvider(provider: {
  id: string; name: string; base_url: string; api_key?: string
}): Promise<void> {
  if (!/^[a-z0-9][a-z0-9_-]{0,127}$/.test(provider.id) || provider.id === 'openai') throw new Error('Invalid provider identifier')
  const parsed = new URL(provider.base_url)
  if (!['http:', 'https:'].includes(parsed.protocol) || !provider.name.trim()) {
    throw new Error('Invalid provider name or URL')
  }
  await request<unknown>(`/api/providers/${encodeURIComponent(provider.id)}`, {
    method: 'POST', body: JSON.stringify({ ...provider }),
  })
}

export async function removeCustomProvider(id: string): Promise<void> {
  if (!/^[a-z0-9][a-z0-9_-]{0,127}$/.test(id) || id === 'openai') throw new Error('Invalid provider identifier')
  await request<unknown>(`/api/providers/${encodeURIComponent(id)}`, { method: 'DELETE' })
}

export interface NativeMcpStatus {
  status: string
  enabled?: boolean
  error?: string
  tools?: Array<{ name: string; description?: string }>
}

export async function listMcpStatuses(signal?: AbortSignal): Promise<Record<string, NativeMcpStatus>> {
  const data = await request<Record<string, NativeMcpStatus>>('/api/mcp', { signal })
  if (!data || typeof data !== 'object' || Array.isArray(data) || Object.keys(data).length > 256) {
    throw new Error('Invalid MCP status map')
  }
  return data
}

export async function addDisabledMcp(name: string, command: string[]): Promise<void> {
  if (!/^[a-zA-Z][a-zA-Z0-9_-]{1,63}$/.test(name) || command.length < 1 || command.length > 32
    || command.some((part) => typeof part !== 'string' || !part || part.length > 4096)) {
    throw new Error('Invalid MCP server name or command')
  }
  await request<unknown>('/api/mcp', {
    method: 'POST',
    body: JSON.stringify({ name, config: { type: 'local', command, enabled: false } }),
  })
}

export async function setMcpConnection(name: string, connect: boolean): Promise<void> {
  if (!/^[a-zA-Z][a-zA-Z0-9_-]{1,63}$/.test(name)) throw new Error('Invalid MCP server name')
  await request<unknown>(`/api/mcp/${encodeURIComponent(name)}/${connect ? 'connect' : 'disconnect'}`, {
    method: 'POST', body: '{}',
  })
}

export async function removeMcp(name: string): Promise<void> {
  if (!/^[a-zA-Z][a-zA-Z0-9_-]{1,63}$/.test(name)) throw new Error('Invalid MCP server name')
  await request<unknown>(`/api/mcp/${encodeURIComponent(name)}`, { method: 'DELETE' })
}

export interface NativeTodo {
  id: string
  content: string
  status: 'pending' | 'in_progress' | 'completed' | 'cancelled'
  priority: 'high' | 'medium' | 'low'
}

export async function listSessionTodos(sessionId: string, signal?: AbortSignal): Promise<NativeTodo[]> {
  const result = await request<{ todos: NativeTodo[] }>(`/api/sessions/${encodeURIComponent(sessionId)}/todo`, { signal })
  if (!Array.isArray(result.todos) || result.todos.length > 100) throw new Error('Invalid todo collection')
  return result.todos
}

export async function saveSessionTodos(sessionId: string, todos: NativeTodo[]): Promise<NativeTodo[]> {
  if (todos.length > 100 || todos.some((todo) => !todo.content.trim() || todo.content.length > 4096)) {
    throw new Error('Todo collection exceeds native limits')
  }
  const result = await request<{ todos: NativeTodo[] }>(`/api/sessions/${encodeURIComponent(sessionId)}/todo`, {
    method: 'PUT', body: JSON.stringify({ todos }),
  })
  if (!Array.isArray(result.todos)) throw new Error('Invalid saved todo response')
  return result.todos
}

export async function runNativeShellStream(
  sessionId: string,
  command: string,
  handlers: TurnStreamHandlers = {},
  signal?: AbortSignal,
): Promise<TurnResult> {
  if (!command.trim() || command.length > 32768 || command.includes('\0')) {
    throw new Error('Invalid shell command')
  }
  return runTurnStream(sessionId, `!${command}`, '', '', handlers, signal, undefined, undefined, command)
}

export async function listModels() {
  const payload = await request<{ models?: ModelSummary[] }>('/api/models?limit=100')
  return Array.isArray(payload.models) ? payload.models : []
}

export function modelKey(model: ModelSummary, index: number) {
  const provider = model.provider ?? model.provider_id
  const id = model.id ?? model.model_id
  if (provider && id) return `${provider}/${id}`
  return id ?? model.name ?? `model-${index}`
}

export function modelLabel(model: ModelSummary, index: number) {
  const provider = model.provider ?? model.provider_id
  const name = model.name ?? model.id ?? model.model_id ?? `Model ${index + 1}`
  return provider ? `${provider} · ${name}` : name
}
