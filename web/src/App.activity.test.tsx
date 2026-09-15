import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0bf00'
const ASSISTANT_ID = '0195f36a-2997-7a89-a11a-fc3359b0bf02'
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
      return json({ models: [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }] })
    }
    if (url === '/api/sessions' && method === 'GET') {
      return json({ sessions: [{ id: SESSION_ID, title: 'Activity chat', state: 'active' }] })
    }
    if (url === `/api/sessions/${SESSION_ID}/messages?limit=200`) {
      return json({
        messages: [
          {
            id: '0195f36a-2997-7a89-a11a-fc3359b0bf01',
            session_id: SESSION_ID,
            role: 'user',
            body: { storage: 'inline', text: 'Explain it' },
          },
          {
            id: ASSISTANT_ID,
            session_id: SESSION_ID,
            role: 'assistant',
            body: { storage: 'inline', text: 'Final answer' },
          },
        ],
      })
    }
    if (url === `/api/sessions/${SESSION_ID}/activity?limit=200`) {
      return json({
        activity: [
          {
            message_id: ASSISTANT_ID,
            reasoning_summary: 'Checked the relevant constraints.',
          },
        ],
      })
    }
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

describe('WEB-009 structured assistant activity', () => {
  it('renders persisted provider reasoning summary separately and collapsed after reload', async () => {
    const user = userEvent.setup()
    render(<App />)

    expect(await screen.findByText('Final answer')).not.toBeNull()
    const summary = screen.getByText('Reasoning summary').closest('details')
    expect(summary).not.toBeNull()
    expect(summary?.hasAttribute('open')).toBe(false)
    expect(within(summary as HTMLElement).getByText('Checked the relevant constraints.')).not.toBeNull()

    await user.click(within(summary as HTMLElement).getByText('Reasoning summary'))
    expect((summary as HTMLDetailsElement).open).toBe(true)
  })
})
