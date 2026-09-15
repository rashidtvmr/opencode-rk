import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const SESSION_ID = '0195f36a-2997-7a89-a11a-fc3359b0d001'
const MESSAGE_ID = '0195f36a-2997-7a89-a11a-fc3359b0d002'
const ARTIFACT_ID = '0195f36a-2997-7a89-a11a-fc3359b0d003'
const fetchMock = vi.fn()

function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

function artifact(version: number, content: string) {
  return {
    id: ARTIFACT_ID,
    session_id: SESSION_ID,
    source_message_id: MESSAGE_ID,
    kind: 'writing',
    title: 'Assistant draft',
    language: null,
    current_version: version,
    content,
    versions: Array.from({ length: version }, (_, index) => ({
      version: index + 1,
      bytes: content.length,
      created_at: '2026-09-15T00:00:00Z',
    })),
    created_at: '2026-09-15T00:00:00Z',
    updated_at: '2026-09-15T00:00:00Z',
  }
}

beforeEach(() => {
  fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const url = String(input)
    const method = init?.method ?? 'GET'
    if (url.endsWith('/health')) return json({ schema_version: 1, status: 'ok', runtime: 'native-rust' })
    if (url === '/api/models?limit=100') return json({ models: [] })
    if (url === '/api/capabilities' || url === '/api/workspaces') return json({}, 404)
    if (url === '/api/sessions' && method === 'GET') {
      return json({ sessions: [{ id: SESSION_ID, title: 'Artifact chat', state: 'active' }] })
    }
    if (url === `/api/sessions/${SESSION_ID}/history?limit=50`) {
      return json({
        messages: [
          {
            id: MESSAGE_ID,
            session_id: SESSION_ID,
            role: 'assistant',
            body: { storage: 'inline', text: 'Original assistant output' },
            created_at: '2026-09-15T00:00:00Z',
          },
        ],
        next_before: null,
      })
    }
    if (url === `/api/sessions/${SESSION_ID}/activity?limit=200`) return json({ activity: [] })
    if (url === `/api/sessions/${SESSION_ID}/attachments`) return json({ attachments: [], available: true })
    if (url === `/api/sessions/${SESSION_ID}/fork`) return json({ fork: null })
    if (url === `/api/sessions/${SESSION_ID}/artifacts` && method === 'GET') {
      return json({ available: true, artifacts: [], run_available: false, apply_available: false })
    }
    if (url === `/api/sessions/${SESSION_ID}/messages/${MESSAGE_ID}/artifacts` && method === 'POST') {
      return json({ artifact: artifact(1, 'Original assistant output') }, 201)
    }
    if (url === `/api/sessions/${SESSION_ID}/artifacts/${ARTIFACT_ID}/versions` && method === 'POST') {
      const request = JSON.parse(String(init?.body)) as { content: string }
      return json({ artifact: artifact(2, request.content) }, 201)
    }
    if (url === `/api/sessions/${SESSION_ID}/artifacts/${ARTIFACT_ID}` && method === 'GET') {
      return json({ artifact: artifact(2, 'Edited artifact') })
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

describe('WEB-017 editable artifacts', () => {
  it('creates an artifact from an assistant message, saves an explicit version, and keeps run/apply disabled', async () => {
    const user = userEvent.setup()
    render(<App />)

    await screen.findByText('Original assistant output')
    const actions = screen.getByRole('group', { name: 'Actions for assistant message' })
    await user.click(within(actions).getByRole('button', { name: 'Open assistant response as artifact' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Open as writing artifact' }))

    expect(await screen.findByRole('complementary', { name: 'Artifact editor' })).not.toBeNull()
    const editor = screen.getByRole('textbox', { name: 'Edit Assistant draft' }) as HTMLTextAreaElement
    await waitFor(() => expect(document.activeElement).toBe(editor))
    expect(editor.value).toBe('Original assistant output')
    expect(screen.getByText('Original assistant output')).not.toBeNull()
    expect(screen.getByRole('button', { name: 'Run unavailable' }).hasAttribute('disabled')).toBe(true)
    expect(screen.getByRole('button', { name: 'Apply unavailable' }).hasAttribute('disabled')).toBe(true)

    await user.clear(editor)
    await user.type(editor, 'Edited artifact')
    await user.click(screen.getByRole('button', { name: 'Save version' }))
    await waitFor(() => expect(screen.getByText(/Version 2 · 2 saved versions/)).not.toBeNull())

    await user.type(screen.getByRole('textbox', { name: 'Edit Assistant draft' }), '!')
    await user.click(screen.getByRole('button', { name: 'Undo' }))
    expect((screen.getByRole('textbox', { name: 'Edit Assistant draft' }) as HTMLTextAreaElement).value).toBe('Edited artifact')
    expect(document.activeElement).toBe(screen.getByRole('textbox', { name: 'Edit Assistant draft' }))
    await user.click(screen.getByRole('button', { name: 'Redo' }))
    expect((screen.getByRole('textbox', { name: 'Edit Assistant draft' }) as HTMLTextAreaElement).value).toBe('Edited artifact!')
    expect(document.activeElement).toBe(screen.getByRole('textbox', { name: 'Edit Assistant draft' }))

    await user.click(screen.getByRole('button', { name: 'Close artifact editor' }))
    await waitFor(() => {
      expect(document.activeElement).toBe(
        within(actions).getByRole('button', { name: 'Open assistant response as artifact' }),
      )
    })
  })
})
