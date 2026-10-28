import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import App from './App'
import { withBrowserResume } from './test/browser-resume-fixture'

const SESSION = '0195f36a-2997-7a89-a11a-fc3359b0aa77'
const fetchMock = vi.fn()

function json(payload: unknown, status = 200) {
  return new Response(JSON.stringify(payload), { status, headers: { 'content-type': 'application/json' } })
}

function ndjson(text: string, label: string) {
  const events = [
    { type: 'user_message', message: { id: `user-${label}`, session_id: SESSION, role: 'user', body: { storage: 'inline', text } } },
    { type: 'assistant_delta', delta: `Verified ${label}` },
    { type: 'assistant_message', message: { id: `assistant-${label}`, session_id: SESSION, role: 'assistant', body: { storage: 'inline', text: `Verified ${label}` } } },
  ]
  return new Response(events.map((event) => JSON.stringify(event)).join('\n') + '\n', {
    status: 201, headers: { 'content-type': 'application/x-ndjson' },
  })
}

beforeEach(() => {
  fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    if (url === '/health') return json({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    if (url === '/api/sessions') return json({ sessions: [{ id: SESSION, title: 'Developer conversation', state: 'active' }] })
    if (url === '/api/models?limit=100') return json({ models: [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }] })
    if (url === '/api/commands') return json({
      commands: [{ name: 'review', description: 'Review changed files', agent: 'reviewer', model: null }],
      agents: [{ name: 'reviewer', description: 'Review code', read_only: true }],
    })
    if (url === `/api/sessions/${SESSION}/messages?limit=200`) return json({ messages: [] })
    if (url === `/api/sessions/${SESSION}/activity?limit=200`) return json({ activity: [] })
    if (url === `/api/sessions/${SESSION}/command` && init?.method === 'POST') {
      return ndjson('/review inspect files', 'command')
    }
    if (url === `/api/sessions/${SESSION}/agent/stream` && init?.method === 'POST') {
      return ndjson('@reviewer inspect files', 'agent')
    }
    return json({ code: 'not_found', message: url }, 404)
  })
  vi.stubGlobal('fetch', withBrowserResume(fetchMock))
})

afterEach(() => {
  cleanup()
  fetchMock.mockReset()
  vi.unstubAllGlobals()
  sessionStorage.clear()
  localStorage.clear()
})

describe('daemon-backed composer actions', () => {
  it('sends a configured slash command to the command endpoint without silently falling back', async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole('heading', { name: 'Developer conversation' })
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith('/api/commands', expect.anything()))
    await user.type(screen.getByRole('textbox', { name: 'Message' }), '/review inspect files')
    await user.click(screen.getByRole('button', { name: 'Send message' }))
    expect(await screen.findByText('Verified command')).not.toBeNull()
    const commandCalls = fetchMock.mock.calls.filter((call) => String(call[0]).endsWith('/command'))
    expect(commandCalls).toHaveLength(1)
    expect(JSON.parse(commandCalls[0][1].body)).toEqual({
      command: 'review', arguments: 'inspect files', model: 'openai/gpt-5.6',
    })
    expect(fetchMock.mock.calls.filter((call) => String(call[0]).endsWith('/turns/stream'))).toHaveLength(0)
  })

  it('routes an advertised @agent through the server-controlled role endpoint', async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole('heading', { name: 'Developer conversation' })
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith('/api/commands', expect.anything()))
    await user.type(screen.getByRole('textbox', { name: 'Message' }), '@reviewer inspect files')
    await user.click(screen.getByRole('button', { name: 'Send message' }))
    expect(await screen.findByText('Verified agent')).not.toBeNull()
    const agentCalls = fetchMock.mock.calls.filter((call) => String(call[0]).endsWith('/agent/stream'))
    expect(agentCalls).toHaveLength(1)
    expect(JSON.parse(agentCalls[0][1].body)).toMatchObject({
      agent: 'reviewer', text: '@reviewer inspect files', model: 'openai/gpt-5.6',
    })
    expect(fetchMock.mock.calls.filter((call) => String(call[0]).endsWith('/turns/stream'))).toHaveLength(0)
  })
})
