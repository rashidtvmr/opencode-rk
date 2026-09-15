import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const sessionId = '0195f36a-2997-7a89-a11a-fc3359b0bd6e'
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

    if (url.endsWith('/health')) {
      return json({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    }

    if (url === '/api/models?limit=100') {
      return json({
        models: [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }],
      })
    }

    if (url === '/api/sessions' && method === 'GET') {
      return json({
        sessions: [
          {
            id: sessionId,
            title: 'Accessibility pass',
            state: 'active',
          },
        ],
      })
    }

    if (url === `/api/sessions/${sessionId}/messages?limit=200` && method === 'GET') {
      return json({
        messages: [
          {
            id: 'message-1',
            session_id: sessionId,
            role: 'assistant',
            body: { storage: 'inline', text: 'Stored response' },
            created_at: '2026-09-15T00:00:00Z',
          },
        ],
      })
    }

    if (url === `/api/sessions/${sessionId}/messages` && method === 'POST') {
      const body = JSON.parse(String(init?.body)) as { text: string }
      return json(
        {
          message: {
            id: 'message-2',
            session_id: sessionId,
            role: 'user',
            body: { storage: 'inline', text: body.text },
            created_at: '2026-09-15T00:00:01Z',
          },
        },
        201,
      )
    }

    if (url === `/api/sessions/${sessionId}` && method === 'PATCH') {
      const body = JSON.parse(String(init?.body)) as { title: string }
      return json({
        session: {
          id: sessionId,
          title: body.title,
          state: 'active',
        },
      })
    }

    if (url === `/api/sessions/${sessionId}/archive` && method === 'POST') {
      return json({
        session: {
          id: sessionId,
          title: 'Accessibility pass',
          state: 'archived',
          archived_at: '2026-09-15T00:00:02Z',
        },
      })
    }

    return json({ code: 'not_found', message: `${method} ${url}` }, 404)
  })

  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchMock.mockReset()
})

describe('web chat interactions', () => {
  it('loads persisted messages and appends a user message', async () => {
    const user = userEvent.setup()
    render(<App />)

    expect(await screen.findByText('Stored response')).not.toBeNull()

    const composer = screen.getByRole('textbox', { name: 'Message' })
    await user.type(composer, 'Persist this message')
    await user.click(screen.getByRole('button', { name: 'Send message' }))

    expect(await screen.findByText('Persist this message')).not.toBeNull()
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/sessions/${sessionId}/messages`,
      expect.objectContaining({
        method: 'POST',
        body: JSON.stringify({ text: 'Persist this message' }),
      }),
    )
  })

  it('renames the selected chat from its actions menu', async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole('heading', { name: 'Accessibility pass' })

    await user.click(screen.getByRole('button', { name: 'Chat actions for Accessibility pass' }))
    await user.click(screen.getByRole('menuitem', { name: 'Rename chat' }))

    const input = screen.getByRole('textbox', { name: 'Rename chat' })
    await user.clear(input)
    await user.type(input, 'Renamed workspace')
    await user.click(screen.getByRole('button', { name: 'Save rename' }))

    expect(await screen.findByRole('heading', { name: 'Renamed workspace' })).not.toBeNull()
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/sessions/${sessionId}`,
      expect.objectContaining({
        method: 'PATCH',
        body: JSON.stringify({ title: 'Renamed workspace' }),
      }),
    )
  })

  it('archives the selected chat and returns to the empty state', async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole('heading', { name: 'Accessibility pass' })

    await user.click(screen.getByRole('button', { name: 'Chat actions for Accessibility pass' }))
    await user.click(screen.getByRole('menuitem', { name: 'Archive chat' }))

    await waitFor(() => {
      expect(screen.queryByText('Accessibility pass')).toBeNull()
    })
    expect(screen.getByRole('heading', { name: 'What can I help you build?' })).not.toBeNull()
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/sessions/${sessionId}/archive`,
      expect.objectContaining({ method: 'POST' }),
    )
  })
})
