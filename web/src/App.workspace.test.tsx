import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0c400'
const fetchMock = vi.fn()

function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

beforeEach(() => {
  fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    const method = init?.method ?? 'GET'
    if (url.endsWith('/health')) return json({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    if (url === '/api/models?limit=100') return json({ models: [] })
    if (url === '/api/capabilities') return json({}, 404)
    if (url === '/api/workspaces') {
      return json({
        available: true,
        workspaces: [
          {
            id: '09090909090909090909090909090909',
            label: 'Main project',
            project_root: '/workspace/main',
            status: 1,
            created_at_us: 11,
          },
        ],
        session_scope_available: false,
        memory_available: false,
        reason: 'registry metadata only',
      })
    }
    if (url === '/api/sessions' && method === 'GET') {
      return json({ sessions: [{ id: SESSION_ID, title: 'Workspace chat', state: 'active' }] })
    }
    if (url === `/api/sessions/${SESSION_ID}/history?limit=50`) {
      return json({ messages: [], next_before: null })
    }
    if (url === `/api/sessions/${SESSION_ID}/activity?limit=200`) return json({ activity: [] })
    if (url === `/api/sessions/${SESSION_ID}/attachments`) return json({ attachments: [], available: true })
    if (url === `/api/sessions/${SESSION_ID}/fork`) return json({ fork: null })
    return json({ code: 'not_found', message: `${method} ${url}` }, 404)
  })
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchMock.mockReset()
})

describe('WEB-015 workspace registry projection', () => {
  it('shows canonical workspace metadata without claiming chat or memory scoping', async () => {
    render(<App />)

    expect(await screen.findByRole('heading', { name: 'Workspace chat' })).not.toBeNull()
    const workspace = screen.getByRole('combobox', { name: 'Workspace' })
    expect(workspace).not.toHaveProperty('disabled', true)
    expect(screen.getByRole('option', { name: 'Main project' })).not.toBeNull()
    expect(screen.getByText('/workspace/main')).not.toBeNull()
    expect(screen.getByText('Ready')).not.toBeNull()
    expect(screen.getByText('Chats, files, context and memory are not workspace-scoped yet.')).not.toBeNull()
  })
})
