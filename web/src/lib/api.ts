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

export type MessageBody =
  | { storage: 'inline'; text: string }
  | { storage: 'blob'; hash: string; bytes: number }

export interface MessageRecord {
  id: string
  session_id: string
  role: MessageRole
  body: MessageBody
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

class ApiError extends Error {
  readonly status: number

  constructor(status: number, message: string) {
    super(message)
    this.name = 'ApiError'
    this.status = status
  }
}

async function request<T>(path: string, init?: RequestInit): Promise<T> {
  const response = await fetch(path, {
    ...init,
    headers: {
      'content-type': 'application/json',
      ...init?.headers,
    },
  })

  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as
      | { message?: string }
      | null
    throw new ApiError(response.status, body?.message ?? `Request failed with ${response.status}`)
  }

  return (await response.json()) as T
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

  return {
    id: candidate.id,
    session_id: candidate.session_id,
    role: candidate.role as MessageRole,
    body,
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

export async function getHealth() {
  return request<HealthResponse>('/health')
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
  const response = await fetch(
    `/api/sessions/${encodeURIComponent(id)}/attachments?${params.toString()}`,
    {
      method: 'POST',
      headers: { 'content-type': file.type || 'application/octet-stream' },
      body: file,
      signal,
    },
  )
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
  const response = await fetch(
    `/api/sessions/${encodeURIComponent(sessionId)}/attachments/${encodeURIComponent(attachmentId)}`,
    { method: 'DELETE', signal },
  )
  if (!response.ok) {
    const body = (await response.json().catch(() => null)) as { message?: string } | null
    throw new ApiError(response.status, body?.message ?? `Request failed with ${response.status}`)
  }
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
  onReasoningSummaryDelta?: (delta: string) => void
  onAssistantDelta?: (delta: string) => void
  onAssistantActivity?: (activity: AssistantActivity) => void
  onAssistantMessage?: (message: MessageRecord) => void
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
): Promise<TurnResult> {
  const response = await fetch(`/api/sessions/${encodeURIComponent(id)}/turns/stream`, {
    method: 'POST',
    headers: { 'content-type': 'application/json' },
    body: JSON.stringify({
      text,
      model,
      reasoning_effort: reasoningEffort,
    }),
    signal,
  })

  if (response.status === 404) {
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
