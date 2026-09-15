import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0c300'
const fetchMock = vi.fn()

function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

const message = (id: string, text: string) => ({
  id,
  session_id: SESSION_ID,
  role: 'user',
  body: { storage: 'inline', text },
})

beforeEach(() => {
  fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    const method = init?.method ?? 'GET'
    if (url.endsWith('/health')) return json({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    if (url === '/api/models?limit=100') return json({ models: [] })
    if (url === '/api/capabilities') return json({}, 404)
    if (url === '/api/sessions' && method === 'GET') {
      return json({ sessions: [{ id: SESSION_ID, title: 'History chat', state: 'active' }] })
    }
    if (url === `/api/sessions/${SESSION_ID}/history?limit=50`) {
      return json({
        messages: [message('m4', 'message 4'), message('m5', 'message 5')],
        next_before: 'm4',
      })
    }
    if (url === `/api/sessions/${SESSION_ID}/history?limit=50&before=m4`) {
      return json({
        messages: [message('m2', 'message 2'), message('m3', 'message 3')],
        next_before: 'm2',
      })
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

describe('WEB-014 progressive history', () => {
  it('starts at the newest page and prepends older messages without reordering the recent page', async () => {
    const user = userEvent.setup()
    render(<App />)

    expect(await screen.findByText('message 4')).not.toBeNull()
    expect(screen.getByText('message 5')).not.toBeNull()
    expect(screen.queryByText('message 2')).toBeNull()

    await user.click(screen.getByRole('button', { name: 'Load older messages' }))
    expect(await screen.findByText('message 2')).not.toBeNull()
    const transcript = screen.getByRole('list', { name: 'Conversation messages' })
    expect(transcript.textContent?.indexOf('message 2')).toBeLessThan(
      transcript.textContent?.indexOf('message 4') ?? Number.MAX_SAFE_INTEGER,
    )
  })
})
