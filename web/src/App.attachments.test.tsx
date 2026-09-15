import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0c100'
const ATTACHMENT_ID = '0195f36a-2997-7a89-a11a-fc3359b0c101'
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
    if (url === '/api/models?limit=100') {
      return json({ models: [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }] })
    }
    if (url === '/api/sessions' && method === 'GET') {
      return json({ sessions: [{ id: SESSION_ID, title: 'Attachment chat', state: 'active' }] })
    }
    if (url === `/api/sessions/${SESSION_ID}/messages?limit=200`) return json({ messages: [] })
    if (url === `/api/sessions/${SESSION_ID}/activity?limit=200`) return json({ activity: [] })
    if (url === `/api/sessions/${SESSION_ID}/fork`) return json({ fork: null })
    if (url === `/api/sessions/${SESSION_ID}/attachments` && method === 'GET') {
      return json({ attachments: [], available: true })
    }
    if (url === `/api/sessions/${SESSION_ID}/attachments?name=notes.txt` && method === 'POST') {
      return json(
        {
          attachment: {
            id: ATTACHMENT_ID,
            session_id: SESSION_ID,
            name: 'notes.txt',
            mime: 'text/plain',
            hash: 'a'.repeat(64),
            bytes: 5,
          },
        },
        201,
      )
    }
    if (
      url === `/api/sessions/${SESSION_ID}/attachments/${ATTACHMENT_ID}` &&
      method === 'DELETE'
    ) {
      return new Response(null, { status: 204 })
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

describe('WEB-011 draft attachments', () => {
  it('stores a real draft file, renders an accessible removable chip, and prevents fake attached send', async () => {
    const user = userEvent.setup()
    const { container } = render(<App />)
    await screen.findByRole('heading', { name: 'Attachment chat' })

    await user.click(screen.getByRole('button', { name: 'Add to message' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Add files' }))
    const input = container.querySelector('input[type="file"]') as HTMLInputElement
    expect(input).not.toBeNull()
    fireEvent.change(input, {
      target: { files: [new File(['hello'], 'notes.txt', { type: 'text/plain' })] },
    })

    const attachments = await screen.findByRole('list', { name: 'Draft attachments' })
    expect(within(attachments).getByText('notes.txt')).not.toBeNull()
    expect(screen.getByRole('button', { name: 'Send message' }).hasAttribute('disabled')).toBe(true)
    expect(fetchMock.mock.calls.some(([input]) => String(input).includes('/turns'))).toBe(false)

    await user.click(within(attachments).getByRole('button', { name: 'Remove notes.txt' }))
    await waitFor(() => expect(screen.queryByRole('list', { name: 'Draft attachments' })).toBeNull())
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/sessions/${SESSION_ID}/attachments/${ATTACHMENT_ID}`,
      expect.objectContaining({ method: 'DELETE' }),
    )
  })
})
