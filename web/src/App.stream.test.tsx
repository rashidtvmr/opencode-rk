import { act, cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0bd6e'
const fetchMock = vi.fn()
const encoder = new TextEncoder()
let streamController: ReadableStreamDefaultController<Uint8Array> | null = null

function jsonResponse(body: unknown, status = 200) {
  return new Response(JSON.stringify(body), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

function line(value: unknown) {
  return `${JSON.stringify(value)}\n`
}

function streamingResponse() {
  return new Response(
    new ReadableStream<Uint8Array>({
      start(controller) {
        streamController = controller
        controller.enqueue(
          encoder.encode(
            `${line({
              type: 'user_message',
              message: {
                id: 'user-stream-1',
                session_id: SESSION_ID,
                role: 'user',
                body: { storage: 'inline', text: 'Stream this change' },
              },
            })}{"type":"assistant_del`,
          ),
        )
        controller.enqueue(encoder.encode(`ta","delta":"Fixture"}\n`))
      },
    }),
    {
      status: 201,
      headers: { 'content-type': 'application/x-ndjson' },
    },
  )
}

beforeEach(() => {
  streamController = null
  fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    const method = init?.method ?? 'GET'

    if (url.endsWith('/health')) {
      return jsonResponse({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    }
    if (url === '/api/sessions' && method === 'GET') {
      return jsonResponse({
        sessions: [{ id: SESSION_ID, title: 'Streaming turn', state: 'active' }],
      })
    }
    if (url === '/api/models?limit=100') {
      return jsonResponse({
        models: [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }],
      })
    }
    if (url === `/api/sessions/${SESSION_ID}/messages?limit=200` && method === 'GET') {
      return jsonResponse({ messages: [] })
    }
    if (url === `/api/sessions/${SESSION_ID}/turns/stream` && method === 'POST') {
      return streamingResponse()
    }
    if (url === `/api/sessions/${SESSION_ID}/turns` && method === 'POST') {
      return jsonResponse(
        {
          user_message: {
            id: 'user-buffered-1',
            session_id: SESSION_ID,
            role: 'user',
            body: { storage: 'inline', text: 'Stream this change' },
          },
          assistant_message: {
            id: 'assistant-buffered-1',
            session_id: SESSION_ID,
            role: 'assistant',
            body: { storage: 'inline', text: 'Buffered fallback reply' },
          },
        },
        201,
      )
    }

    return jsonResponse({ code: 'not_found', message: `${method} ${url}` }, 404)
  })
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchMock.mockReset()
  streamController = null
})

describe('web turn streaming', () => {
  it('renders deltas before completion and reconciles the persisted assistant once', async () => {
    const user = userEvent.setup()
    render(<App />)

    await screen.findByRole('heading', { name: 'Streaming turn' })
    const composer = screen.getByRole('textbox', { name: 'Message' })
    await user.type(composer, 'Stream this change')
    await user.click(screen.getByRole('button', { name: 'Send message' }))

    expect(await screen.findByText('Fixture')).not.toBeNull()
    expect(composer.getAttribute('aria-busy')).toBe('true')
    const liveRegion = document.querySelector('[aria-live="polite"]')
    expect(liveRegion?.textContent).toBe('')

    expect(streamController).not.toBeNull()
    await act(async () => {
      streamController?.enqueue(
        encoder.encode(line({ type: 'assistant_delta', delta: ' assistant reply.' })),
      )
      streamController?.enqueue(
        encoder.encode(
          line({
            type: 'assistant_message',
            message: {
              id: 'assistant-stream-1',
              session_id: SESSION_ID,
              role: 'assistant',
              body: { storage: 'inline', text: 'Fixture assistant reply.' },
            },
          }),
        ),
      )
      streamController?.close()
    })

    expect(await screen.findByText('Fixture assistant reply.')).not.toBeNull()
    await waitFor(() => expect(composer.getAttribute('aria-busy')).toBeNull())
    expect(liveRegion?.textContent).toBe('Assistant reply received.')
    expect(screen.getAllByText('Fixture assistant reply.')).toHaveLength(1)

    const streamCall = fetchMock.mock.calls.find(
      ([input, init]) =>
        String(input) === `/api/sessions/${SESSION_ID}/turns/stream` && init?.method === 'POST',
    )
    expect(streamCall).toBeDefined()
    expect(JSON.parse(String(streamCall?.[1]?.body))).toEqual({
      text: 'Stream this change',
      model: 'openai/gpt-5.6',
      reasoning_effort: 'high',
    })
  })
})
