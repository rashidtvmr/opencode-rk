import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0bd6e'
const fetchMock = vi.fn()

function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

beforeEach(() => {
  fetchMock.mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input)
    if (url.endsWith('/health')) {
      return json({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    }
    if (url === '/api/models?limit=100') {
      return json({ models: [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }] })
    }
    if (url === '/api/sessions') {
      return json({
        sessions: [{ id: SESSION_ID, title: 'Fork actions', state: 'active' }],
      })
    }
    if (url === `/api/sessions/${SESSION_ID}/messages?limit=200`) {
      return json({
        messages: [
          {
            id: 'user-message',
            session_id: SESSION_ID,
            role: 'user',
            body: { storage: 'inline', text: 'Parent request' },
            created_at: '2026-09-15T00:00:00Z',
          },
          {
            id: 'assistant-message',
            session_id: SESSION_ID,
            role: 'assistant',
            body: { storage: 'inline', text: 'Parent response' },
            created_at: '2026-09-15T00:00:01Z',
          },
        ],
      })
    }
    if (url === `/api/sessions/${SESSION_ID}/fork`) {
      return json({ fork: null })
    }
    return json({ code: 'not_found', message: `GET ${url}` }, 404)
  })
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchMock.mockReset()
})

describe('WEB-007 message action rows', () => {
  it('renders full-row role geometry and opens an accessible Fork popover for persisted user and assistant messages', async () => {
    const user = userEvent.setup()
    const { container } = render(<App />)

    await screen.findByText('Parent response')

    const rows = container.querySelectorAll('.codex-message-row')
    expect(rows).toHaveLength(2)
    expect(rows[0]?.classList.contains('codex-message-user')).toBe(true)
    expect(rows[1]?.classList.contains('codex-message-assistant')).toBe(true)

    const userActions = screen.getByRole('group', { name: 'Actions for user message' })
    const assistantActions = screen.getByRole('group', { name: 'Actions for assistant message' })
    expect(within(userActions).getByRole('button', { name: 'Fork user message' })).not.toBeNull()
    expect(within(userActions).getByRole('button', { name: 'Edit user message' })).not.toBeNull()
    expect(within(userActions).getByRole('button', { name: 'Retry user message' })).not.toBeNull()
    expect(within(assistantActions).getByRole('button', { name: 'Copy assistant message' })).not.toBeNull()
    expect(
      within(assistantActions).getByRole('button', { name: 'Regenerate assistant response' }),
    ).not.toBeNull()

    const fork = within(assistantActions).getByRole('button', { name: 'Fork assistant message' })
    await user.click(fork)
    expect(await screen.findByRole('menuitem', { name: 'Branch in new chat' })).not.toBeNull()

    await user.keyboard('{Escape}')
    expect(document.activeElement).toBe(fork)
  })
})
