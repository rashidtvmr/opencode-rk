import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0c200'
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
    if (url === '/api/models?limit=100') return json({ models: [] })
    if (url === '/api/sessions' && method === 'GET') {
      return json({ sessions: [{ id: SESSION_ID, title: 'Capabilities chat', state: 'active' }] })
    }
    if (url === '/api/capabilities') {
      const turnReason = 'the current web Responses turn adapter does not execute native tools'
      return json({
        tools: [
          {
            id: 'bash',
            name: 'bash',
            description: 'Execute a shell command in the workspace directory.',
            enabled: true,
            type: 'shell',
            tags: ['shell', 'system'],
            available_for_web_turn: false,
            reason: turnReason,
          },
        ],
        plugins: { available_for_web_turn: false, reason: 'plugin host unavailable' },
        approvals: { available_for_web_turn: false, reason: 'approval flow unavailable' },
        attachments: {
          draft_ingest: true,
          available_for_web_turn: false,
          reason: 'provider attachment adapter unavailable',
        },
        search: { available: false, reason: 'local grep is not a web-search adapter' },
        deep_research: { available: false, reason: 'no native research adapter is installed' },
        voice: { available: false, reason: 'no native realtime audio adapter is installed' },
        artifacts: { available: false, reason: 'artifact service unavailable' },
      })
    }
    if (url === `/api/sessions/${SESSION_ID}/messages?limit=200`) return json({ messages: [] })
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

describe('WEB-012/013/016 capability gating', () => {
  it('shows real daemon tools but keeps unavailable turn adapters disabled', async () => {
    const user = userEvent.setup()
    render(<App />)
    await screen.findByRole('heading', { name: 'Capabilities chat' })

    await user.click(screen.getByRole('button', { name: 'Tools and apps' }))
    expect(await screen.findByText(/bash · the current web Responses turn adapter/)).not.toBeNull()
    expect(screen.getByText(/Plugins\/apps · plugin host unavailable/)).not.toBeNull()

    await user.keyboard('{Escape}')
    await user.click(screen.getByRole('button', { name: 'Commands and mentions' }))
    expect(await screen.findByText('Search · local grep is not a web-search adapter')).not.toBeNull()
    expect(screen.getByText('Deep research · no native research adapter is installed')).not.toBeNull()

    expect(
      screen.getByRole('button', {
        name: 'Voice input unavailable: no native realtime audio adapter is installed',
      }),
    ).not.toBeNull()
  })
})
