import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor, within } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import App from './App'

const PARENT_ID = '0195f36a-2997-7a89-a11a-fc3359b0be00'
const EDIT_CHILD_ID = '0195f36a-2997-7a89-a11a-fc3359b0be01'
const REGEN_CHILD_ID = '0195f36a-2997-7a89-a11a-fc3359b0be02'
const fetchMock = vi.fn()

function json(value: unknown, status = 200) {
  return new Response(JSON.stringify(value), {
    status,
    headers: { 'content-type': 'application/json' },
  })
}

function ndjson(events: unknown[], status = 201) {
  return new Response(events.map((event) => `${JSON.stringify(event)}\n`).join(''), {
    status,
    headers: { 'content-type': 'application/x-ndjson' },
  })
}

const parentMessages = [
  {
    id: '0195f36a-2997-7a89-a11a-fc3359b0be11',
    session_id: PARENT_ID,
    role: 'user',
    body: { storage: 'inline', text: 'First request' },
  },
  {
    id: '0195f36a-2997-7a89-a11a-fc3359b0be12',
    session_id: PARENT_ID,
    role: 'assistant',
    body: { storage: 'inline', text: 'First answer' },
  },
  {
    id: '0195f36a-2997-7a89-a11a-fc3359b0be13',
    session_id: PARENT_ID,
    role: 'user',
    body: { storage: 'inline', text: 'Second request' },
  },
  {
    id: '0195f36a-2997-7a89-a11a-fc3359b0be14',
    session_id: PARENT_ID,
    role: 'assistant',
    body: { storage: 'inline', text: 'Second answer' },
  },
]

function prefixFor(sessionId: string) {
  return parentMessages.slice(0, 2).map((message, index) => ({
    ...message,
    id: `${sessionId}-${index + 1}`,
    session_id: sessionId,
  }))
}

let regenChildReads = 0

beforeEach(() => {
  regenChildReads = 0
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

    if (
      url === `/api/sessions/${PARENT_ID}/messages/${parentMessages[2].id}/retry-branch` &&
      method === 'POST'
    ) {
      return json(
        {
          session: { id: EDIT_CHILD_ID, title: 'Branch: Parent chat', state: 'active' },
          fork: {
            parent_session_id: PARENT_ID,
            fork_message_seq: 2,
            boundary_message_id: null,
          },
          request_text: 'Second request',
          trigger_message_id: parentMessages[2].id,
        },
        201,
      )
    }
    if (url === `/api/sessions/${EDIT_CHILD_ID}/messages?limit=200`) {
      return json({ messages: prefixFor(EDIT_CHILD_ID) })
    }
    if (url === `/api/sessions/${EDIT_CHILD_ID}/turns/stream` && method === 'POST') {
      return ndjson([
        {
          type: 'user_message',
          message: {
            id: 'edited-user',
            session_id: EDIT_CHILD_ID,
            role: 'user',
            body: { storage: 'inline', text: 'Edited request' },
          },
        },
        { type: 'assistant_delta', delta: 'Edited answer' },
        {
          type: 'assistant_message',
          message: {
            id: 'edited-assistant',
            session_id: EDIT_CHILD_ID,
            role: 'assistant',
            body: { storage: 'inline', text: 'Edited answer' },
          },
        },
      ])
    }

    if (
      url === `/api/sessions/${PARENT_ID}/messages/${parentMessages[3].id}/retry-branch` &&
      method === 'POST'
    ) {
      return json(
        {
          session: { id: REGEN_CHILD_ID, title: 'Branch: Parent chat', state: 'active' },
          fork: {
            parent_session_id: PARENT_ID,
            fork_message_seq: 2,
            boundary_message_id: null,
          },
          request_text: 'Second request',
          trigger_message_id: parentMessages[3].id,
        },
        201,
      )
    }
    if (url === `/api/sessions/${REGEN_CHILD_ID}/messages?limit=200`) {
      regenChildReads += 1
      return json({
        messages:
          regenChildReads === 1
            ? prefixFor(REGEN_CHILD_ID)
            : [
                ...prefixFor(REGEN_CHILD_ID),
                {
                  id: 'persisted-retry-user',
                  session_id: REGEN_CHILD_ID,
                  role: 'user',
                  body: { storage: 'inline', text: 'Second request' },
                },
              ],
      })
    }
    if (url === `/api/sessions/${REGEN_CHILD_ID}/turns/stream` && method === 'POST') {
      return json({ code: 'provider_failure', message: 'Provider unavailable' }, 502)
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

describe('WEB-008 edit, retry and regenerate', () => {
  it('edits a persisted user request by branching before it and executing the edited request once', async () => {
    const user = userEvent.setup()
    render(<App />)

    const originalRequest = await screen.findByText('Second request')
    const row = originalRequest.closest('li')
    expect(row).not.toBeNull()
    await user.click(within(row as HTMLElement).getByRole('button', { name: 'Edit user message' }))

    const editor = screen.getByRole('textbox', { name: 'Edit user message' })
    await user.clear(editor)
    await user.type(editor, 'Edited request')
    await user.click(screen.getByRole('button', { name: 'Send edit in new chat' }))

    expect(await screen.findByRole('heading', { name: 'Branch: Parent chat' })).not.toBeNull()
    expect(await screen.findByText('Edited answer')).not.toBeNull()
    expect(screen.queryByText('Second answer')).toBeNull()

    const streamCall = fetchMock.mock.calls.find(
      ([input, init]) =>
        String(input) === `/api/sessions/${EDIT_CHILD_ID}/turns/stream` && init?.method === 'POST',
    )
    expect(streamCall).toBeDefined()
    expect(JSON.parse(String(streamCall?.[1]?.body))).toEqual({
      text: 'Edited request',
      model: 'openai/gpt-5.6',
      reasoning_effort: 'high',
    })

    const lineage = screen.getByLabelText('Branch lineage')
    await user.click(within(lineage).getByRole('button', { name: 'Parent chat' }))
    expect(await screen.findByText('Second answer')).not.toBeNull()
    expect(await screen.findByText('Second request')).not.toBeNull()
  })

  it('keeps the retry branch and persisted user request when regenerate fails without inventing an assistant reply', async () => {
    const user = userEvent.setup()
    render(<App />)

    const originalAnswer = await screen.findByText('Second answer')
    const row = originalAnswer.closest('li')
    expect(row).not.toBeNull()
    await user.click(
      within(row as HTMLElement).getByRole('button', { name: 'Regenerate assistant response' }),
    )

    expect(await screen.findByRole('heading', { name: 'Branch: Parent chat' })).not.toBeNull()
    await waitFor(() => expect(screen.getByText('Second request')).not.toBeNull())
    expect(screen.queryByText('Second answer')).toBeNull()
    expect(document.querySelector('[aria-live="polite"]')?.textContent).toContain('Provider unavailable')
  })
})
