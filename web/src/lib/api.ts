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
    throw new Error(body?.message ?? `Request failed with ${response.status}`)
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

export async function appendMessage(id: string, text: string) {
  const payload = await request<{ message: unknown }>(
    `/api/sessions/${encodeURIComponent(id)}/messages`,
    {
      method: 'POST',
      body: JSON.stringify({ text }),
    },
  )
  const message = normalizeMessage(payload.message)
  if (!message) throw new Error('Server returned an invalid message')
  return message
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
