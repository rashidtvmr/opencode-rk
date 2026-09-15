import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0bd6e'
const fetchMock = vi.fn()

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

beforeEach(() => {
  fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)

    if (url.endsWith('/health')) {
      return jsonResponse({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    }
    if (url === '/api/sessions') {
      return jsonResponse({
        sessions: [{ id: SESSION_ID, title: 'Turn execution', archived: false }],
      })
    }
    if (url.includes('/api/models')) {
      return jsonResponse({
        models: [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }],
      })
    }
    if (url.includes(`/api/sessions/${SESSION_ID}/messages`)) {
      return jsonResponse({ messages: [] })
    }
    if (url === `/api/sessions/${SESSION_ID}/turns` && init?.method === 'POST') {
      return jsonResponse(
        {
          user_message: {
            id: 'user-1',
            session_id: SESSION_ID,
            role: 'user',
            body: { storage: 'inline', text: 'Explain this change' },
          },
          assistant_message: {
            id: 'assistant-1',
            session_id: SESSION_ID,
            role: 'assistant',
            body: { storage: 'inline', text: 'Here is the assistant reply.' },
          },
        },
        201,
      )
    }

    return jsonResponse({ code: 'not_found', message: 'missing fixture' }, 404)
  })
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchMock.mockReset()
})

describe('web turn execution', () => {
  it('submits the selected model and reasoning effort and renders the assistant reply', async () => {
    const user = userEvent.setup()
    render(<App />)

    await screen.findByRole('heading', { name: 'Turn execution' })
    const composer = screen.getByRole('textbox', { name: 'Message' })
    await user.type(composer, 'Explain this change')
    await user.click(screen.getByRole('button', { name: 'Send message' }))

    expect(await screen.findByText('Here is the assistant reply.')).not.toBeNull()

    const turnCall = fetchMock.mock.calls.find(
      ([input, init]) =>
        String(input) === `/api/sessions/${SESSION_ID}/turns` && init?.method === 'POST',
    )
    expect(turnCall).toBeDefined()
    const body = JSON.parse(String(turnCall?.[1]?.body))
    expect(body).toEqual({
      text: 'Explain this change',
      model: 'openai/gpt-5.6',
      reasoning_effort: 'high',
    })
  })
})
