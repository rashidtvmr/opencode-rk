import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const PARENT_ID = '0195f36a-2997-7a89-a11a-fc3359b0bd6e'
const CHILD_ID = '0195f36a-2997-7a89-a11a-fc3359b0bd7f'
const fetchMock = vi.fn()

function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

const parentMessages = [
  {
    id: '0195f36a-2997-7a89-a11a-fc3359b0bd81',
    session_id: PARENT_ID,
    role: 'user',
    body: { storage: 'inline', text: 'First request' },
    created_at: '2026-09-15T00:00:00Z',
  },
  {
    id: '0195f36a-2997-7a89-a11a-fc3359b0bd82',
    session_id: PARENT_ID,
    role: 'assistant',
    body: { storage: 'inline', text: 'First answer' },
    created_at: '2026-09-15T00:00:01Z',
  },
  {
    id: '0195f36a-2997-7a89-a11a-fc3359b0bd83',
    session_id: PARENT_ID,
    role: 'user',
    body: { storage: 'inline', text: 'Later source-only request' },
    created_at: '2026-09-15T00:00:02Z',
  },
]

const childMessages = [
  {
    ...parentMessages[0],
    id: '0195f36a-2997-7a89-a11a-fc3359b0bd91',
    session_id: CHILD_ID,
  },
  {
    ...parentMessages[1],
    id: '0195f36a-2997-7a89-a11a-fc3359b0bd92',
    session_id: CHILD_ID,
  },
]

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
      return json({ sessions: [{ id: PARENT_ID, title: 'Parent chat', state: 'active' }] })
    }
    if (url === `/api/sessions/${PARENT_ID}/messages?limit=200`) {
      return json({ messages: parentMessages })
    }
    if (url === `/api/sessions/${PARENT_ID}/fork`) {
      return json({ fork: null })
    }
    if (url === `/api/sessions/${CHILD_ID}/messages?limit=200`) {
      return json({ messages: childMessages })
    }
    if (url === `/api/sessions/${CHILD_ID}/fork`) {
      return json({
        fork: {
          parent_session_id: PARENT_ID,
          fork_message_seq: 2,
          boundary_message_id: parentMessages[1].id,
        },
      })
    }
    if (
      url ===
        `/api/sessions/${PARENT_ID}/messages/${parentMessages[1].id}/branch` &&
      method === 'POST'
    ) {
      return json(
        {
          session: { id: CHILD_ID, title: 'Branch: Parent chat', state: 'active' },
          fork: {
            parent_session_id: PARENT_ID,
            fork_message_seq: 2,
            boundary_message_id: parentMessages[1].id,
          },
        },
        201,
      )
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

describe('WEB-008 Branch in new chat', () => {
  it('branches from the selected assistant response, opens the child, and leaves the parent transcript unchanged', async () => {
    const user = userEvent.setup()
    render(<App />)

    await screen.findByText('Later source-only request')
    const assistantActions = screen.getByRole('group', { name: 'Actions for assistant message' })
    await user.click(within(assistantActions).getByRole('button', { name: 'Fork assistant message' }))
    await user.click(await screen.findByRole('menuitem', { name: 'Branch in new chat' }))

    expect(await screen.findByRole('heading', { name: 'Branch: Parent chat' })).not.toBeNull()
    expect(await screen.findByText('First answer')).not.toBeNull()
    expect(screen.queryByText('Later source-only request')).toBeNull()
    expect(screen.getByLabelText('Branch lineage').textContent).toContain('Parent chat')
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/sessions/${PARENT_ID}/messages/${parentMessages[1].id}/branch`,
      expect.objectContaining({ method: 'POST', body: '{}' }),
    )

    await user.click(screen.getByRole('button', { name: 'Parent chat' }))
    expect(await screen.findByText('Later source-only request')).not.toBeNull()
    await waitFor(() => {
      expect(screen.getByRole('heading', { name: 'Parent chat' })).not.toBeNull()
    })
  })
})
