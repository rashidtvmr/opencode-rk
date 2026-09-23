import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0c009'
const ASSISTANT_ID = '0195f36a-2997-7a89-a11a-fc3359b0c00a'
const SOURCE_URL = 'https://platform.openai.com/docs/guides/responses'
const fetchMock = vi.fn()
let fixtureMode: 'stream' | 'reload' | 'malformed' = 'stream'
let persisted = false

function jsonResponse(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

function message(id: string, role: 'user' | 'assistant', text: string) {
  return {
    id,
    session_id: SESSION_ID,
    role,
    body: { storage: 'inline', text },
  }
}

function durableActivity() {
  return {
    message_id: ASSISTANT_ID,
    reasoning_summary: 'Reviewed the constraints before answering.',
    tool_calls: [
      {
        call_id: 'call-source-1',
        name: 'search',
        state: 'completed',
        ok: true,
      },
    ],
    references: [{ label: 'OpenAI API documentation', url: SOURCE_URL }],
  }
}

function historyMessages() {
  return persisted || fixtureMode === 'reload'
    ? [
        message('user-reloaded-1', 'user', 'Explain the source'),
        message(ASSISTANT_ID, 'assistant', 'Durable final answer.'),
      ]
    : []
}

function activityPayload() {
  if (!persisted && fixtureMode === 'stream') return []
  if (fixtureMode === 'malformed') {
    return [
      {
        message_id: ASSISTANT_ID,
        reasoning_summary: 'Reviewed the constraints before answering.',
        tool_calls: [{ call_id: 'missing-state', name: 'search' }],
        references: Array.from({ length: 65 }, (_, index) => ({
          label: `Source ${index + 1}`,
          url: `${SOURCE_URL}?source=${index + 1}`,
        })),
      },
    ]
  }
  return [durableActivity()]
}

function streamResponse() {
  const events = [
    {
      type: 'user_message',
      message: message('user-stream-1', 'user', 'Explain the source'),
    },
    { type: 'reasoning_summary_delta', delta: 'Reviewed the constraints before answering.' },
    { type: 'assistant_activity', activity: durableActivity() },
    { type: 'assistant_delta', delta: 'Durable final answer.' },
    {
      type: 'assistant_message',
      message: message(ASSISTANT_ID, 'assistant', 'Durable final answer.'),
      reasoning_summary: 'Reviewed the constraints before answering.',
      tool_calls: durableActivity().tool_calls,
      references: durableActivity().references,
    },
  ]
  const body = `${events.map((event) => JSON.stringify(event)).join('\n')}\n`
  return new Response(body, {
    status: 201,
    headers: { 'content-type': 'application/x-ndjson' },
  })
}

function malformedStreamResponse() {
  const events = [
    {
      type: 'user_message',
      message: message('user-malformed-1', 'user', 'Explain the source'),
    },
    { type: 'assistant_delta', delta: 'Fabricated final answer.' },
    {
      type: 'assistant_message',
      message: message(ASSISTANT_ID, 'assistant', 'Fabricated final answer.'),
      reasoning_summary: '',
      tool_calls: [{ call_id: 'missing-state', name: 'search' }],
      references: Array.from({ length: 65 }, (_, index) => ({
        label: `Source ${index + 1}`,
        url: `${SOURCE_URL}?source=${index + 1}`,
      })),
    },
  ]
  return new Response(`${events.map((event) => JSON.stringify(event)).join('\n')}\n`, {
    status: 201,
    headers: { 'content-type': 'application/x-ndjson' },
  })
}

beforeEach(() => {
  fixtureMode = 'stream'
  persisted = false
  fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    const method = init?.method ?? 'GET'

    if (url.endsWith('/health')) {
      return jsonResponse({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    }
    if (url === '/api/models?limit=100') {
      return jsonResponse({ models: [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }] })
    }
    if (url === '/api/sessions' && method === 'GET') {
      return jsonResponse({ sessions: [{ id: SESSION_ID, title: 'Structured activity', state: 'active' }] })
    }
    if (url === `/api/sessions/${SESSION_ID}/history?limit=50`) {
      return jsonResponse({ messages: historyMessages(), next_before: null })
    }
    if (url === `/api/sessions/${SESSION_ID}/messages?limit=200`) {
      return jsonResponse({ messages: historyMessages() })
    }
    if (url === `/api/sessions/${SESSION_ID}/activity?limit=200`) {
      return jsonResponse({ activity: activityPayload() })
    }
    if (url === `/api/sessions/${SESSION_ID}/attachments`) {
      return jsonResponse({ attachments: [], available: true })
    }
    if (url === `/api/sessions/${SESSION_ID}/artifacts`) {
      return jsonResponse({ artifacts: [], available: false })
    }
    if (url === `/api/sessions/${SESSION_ID}/fork`) {
      return jsonResponse({ fork: null })
    }
    if (url === `/api/sessions/${SESSION_ID}/turns/stream` && method === 'POST') {
      persisted = true
      return fixtureMode === 'malformed' ? malformedStreamResponse() : streamResponse()
    }
    if (url === '/api/capabilities' || url === '/api/workspaces') {
      return jsonResponse({ code: 'not_found', message: `${method} ${url}` }, 404)
    }
    return jsonResponse({ code: 'not_found', message: `${method} ${url}` }, 404)
  })
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchMock.mockReset()
})

describe('WEB-009 browser structured activity RED', () => {
  it('projects one authenticated stream into reasoning, tools, answer and HTTPS references', async () => {
    const user = userEvent.setup()
    render(<App />)

    await screen.findByRole('heading', { name: 'Structured activity' })
    const composer = screen.getByRole('textbox', { name: 'Message' })
    await user.type(composer, 'Explain the source')
    await user.click(screen.getByRole('button', { name: 'Send message' }))

    expect(await screen.findByText('Durable final answer.')).not.toBeNull()

    const reasoning = screen.getByText('Reasoning summary').closest('details')
    expect(reasoning).not.toBeNull()
    expect(reasoning?.hasAttribute('open')).toBe(false)

    const tools = screen.getByText('Tool activity').closest('details')
    expect(tools).not.toBeNull()
    expect(tools?.hasAttribute('open')).toBe(false)

    const reference = screen.getByRole('link', { name: 'OpenAI API documentation' })
    expect(reference.getAttribute('href')).toBe(SOURCE_URL)
    expect(screen.getByText('Durable final answer.').closest('details')).toBeNull()

    const liveRegions = [...document.querySelectorAll<HTMLElement>('[aria-live]')]
    expect(liveRegions.some((region) => region.textContent?.includes('Durable '))).toBe(false)
  })

  it('reconstructs the same AssistantActivity from the reload response shape', async () => {
    fixtureMode = 'reload'
    persisted = true
    render(<App />)

    expect(await screen.findByText('Durable final answer.')).not.toBeNull()
    const reasoning = screen.getByText('Reasoning summary').closest('details')
    expect(reasoning?.hasAttribute('open')).toBe(false)
    expect(screen.getByText('Tool activity').closest('details')?.hasAttribute('open')).toBe(false)
    expect(screen.getByRole('link', { name: 'OpenAI API documentation' })).not.toBeNull()
  })

  it('fails closed on malformed tool records and over-cap references', async () => {
    fixtureMode = 'malformed'
    persisted = true
    const user = userEvent.setup()
    render(<App />)

    await screen.findByRole('heading', { name: 'Structured activity' })
    const composer = screen.getByRole('textbox', { name: 'Message' })
    await user.type(composer, 'Explain the source')
    await user.click(screen.getByRole('button', { name: 'Send message' }))
    await waitFor(() => expect(screen.queryByText('Fabricated final answer.')).toBeNull())
    expect(screen.queryByText('Fabricated final answer.')).toBeNull()
    expect(screen.queryByRole('link', { name: 'Source 1' })).toBeNull()
  })
})
