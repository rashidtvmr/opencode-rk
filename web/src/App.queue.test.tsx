import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'
import { withBrowserResume } from './test/browser-resume-fixture'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0c001'
const fetchMock = vi.fn()

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

function turnStream(index: number, text: string) {
  const events = [
    {
      type: 'user_message',
      message: {
        id: `user-${index}`,
        session_id: SESSION_ID,
        role: 'user',
        body: { storage: 'inline', text },
      },
    },
    { type: 'assistant_delta', delta: `Answer ${index}` },
    {
      type: 'assistant_message',
      message: {
        id: `assistant-${index}`,
        session_id: SESSION_ID,
        role: 'assistant',
        body: { storage: 'inline', text: `Answer ${index}` },
      },
    },
  ]
  return new Response(events.map((event) => `${JSON.stringify(event)}\n`).join(''), {
    status: 201,
    headers: { 'content-type': 'application/x-ndjson' },
  })
}

let resolveFirstTurn: ((response: Response) => void) | null = null
let submittedBodies: Array<{ text: string }> = []

beforeEach(() => {
  submittedBodies = []
  const firstTurn = new Promise<Response>((resolve) => {
    resolveFirstTurn = resolve
  })

  fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    const method = init?.method ?? 'GET'

    if (url.endsWith('/health')) {
      return jsonResponse({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    }
    if (url === '/api/sessions') {
      return jsonResponse({
        sessions: [{ id: SESSION_ID, title: 'Queued turns', archived: false }],
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
    if (url === `/api/sessions/${SESSION_ID}/turns/stream` && method === 'POST') {
      const body = JSON.parse(String(init?.body)) as { text: string }
      submittedBodies.push(body)
      if (submittedBodies.length === 1) return firstTurn
      return turnStream(submittedBodies.length, body.text)
    }

    return jsonResponse({ code: 'not_found', message: `${method} ${url}` }, 404)
  })
  vi.stubGlobal('fetch', withBrowserResume(fetchMock))
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchMock.mockReset()
  resolveFirstTurn = null
})

describe('active-turn follow-up queue', () => {
  it('keeps the composer writable, exposes queue status, and drains follow-ups FIFO', async () => {
    const user = userEvent.setup()
    render(<App />)

    await screen.findByRole('heading', { name: 'Queued turns' })
    const composer = screen.getByRole('textbox', { name: 'Message' })

    await user.type(composer, 'First request')
    await user.click(screen.getByRole('button', { name: 'Send message' }))
    await screen.findByRole('button', { name: 'Stop generating' })
    expect(composer.getAttribute('contenteditable')).toBe('true')

    await user.type(composer, 'Second request')
    await user.click(screen.getByRole('button', { name: 'Queue follow-up' }))
    expect(await screen.findByText('1 follow-up queued · 16 max')).not.toBeNull()

    await user.type(composer, 'Third request')
    await user.click(screen.getByRole('button', { name: 'Queue follow-up' }))
    expect(await screen.findByText('2 follow-ups queued · 16 max')).not.toBeNull()
    expect(submittedBodies.map((body) => body.text)).toEqual(['First request'])

    resolveFirstTurn?.(turnStream(1, 'First request'))

    await waitFor(() => {
      expect(submittedBodies.map((body) => body.text)).toEqual([
        'First request',
        'Second request',
        'Third request',
      ])
    })
    await waitFor(() => expect(screen.queryByText(/follow-ups? queued/)).toBeNull())
  })
})
