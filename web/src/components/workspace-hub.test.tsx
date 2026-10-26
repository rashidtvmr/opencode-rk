import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { useState } from 'react'
import { WorkspaceHub, type HubPage } from './workspace-hub'
import { withBrowserResume } from '@/test/browser-resume-fixture'

const SESSION = '0195f36a-2997-7a89-a11a-fc3359b0aa99'
const fetchMock = vi.fn()
const mockedTasks = [{ id: 'task-1', content: 'Review code', status: 'pending', priority: 'high' }]
const json = (payload: unknown, status = 200) => new Response(JSON.stringify(payload), {
  status, headers: { 'content-type': 'application/json' },
})

beforeEach(() => {
  fetchMock.mockImplementation(async (input: RequestInfo | URL, init?: RequestInit) => {
    const path = String(input)
    const method = init?.method ?? 'GET'
    if (path === `/api/sessions/${SESSION}/git`) {
      return json({ available: true, root: '/safe/workspace', read_only: true,
        status: ['## main-v2', ' M src/App.tsx'], worktrees: [
          { path: '/safe/workspace', head: 'abc123', branch: 'main-v2', locked: false, prunable: false },
        ] })
    }
    if (path.includes('/git/diff?path=src%2FApp.tsx')) {
      return json({ path: 'src/App.tsx', diff: 'diff --git a/src/App.tsx b/src/App.tsx\n+new line',
        read_only: true, available: true })
    }
    if (path.endsWith('/files?path=.')) {
      return json({ type: 'directory', path: '.', entries: [
        { name: 'src', path: 'src', is_dir: true, bytes: 0 },
      ], truncated: false, read_only: true })
    }
    if (path.endsWith('/files?path=src')) {
      return json({ type: 'directory', path: 'src', entries: [
        { name: 'App.tsx', path: 'src/App.tsx', is_dir: false, bytes: 25 },
      ], truncated: false, read_only: true })
    }
    if (path.endsWith('/files?path=src%2FApp.tsx')) {
      return json({ type: 'text', path: 'src/App.tsx', text: '<img onerror=alert(1)>', bytes: 25, read_only: true })
    }
    if (path === '/api/providers') return json({ providers: [{ id: 'dev', name: 'Dev', base_url: 'https://api.example.test', buffered_model_ids: [] }] })
    if (path === '/api/providers/demo' && method === 'POST') return json(true)
    if (path === '/api/mcp' && method === 'GET') return json({ 'local-test': { status: 'disabled', enabled: false, tools: [] } })
    if (path === '/api/mcp' && method === 'POST') return json({})
    if (path === '/api/permission-rules' || path.startsWith('/api/permission-rules?')) {
      return method === 'POST' ? new Response(null, { status: 204 }) : json({ rules: [] })
    }
    if (path === `/api/sessions/${SESSION}/todo`) {
      return method === 'PUT' ? json(JSON.parse(String(init?.body))) : json({ todos: mockedTasks })
    }
    if (path === `/api/sessions/${SESSION}/shell/stream`) {
      const events = [
        { type: 'user_message', message: { id: 'user-1', session_id: SESSION, role: 'user', body: { storage: 'inline', text: '!pwd' } } },
        { type: 'tool_call', call_id: 'call-1', name: 'bash', arguments: '{"command":"pwd"}' },
        { type: 'tool_output', call_id: 'call-1', name: 'bash', output: '/safe/workspace', success: true },
        { type: 'assistant_message', message: { id: 'assistant-1', session_id: SESSION, role: 'assistant', body: { storage: 'inline', text: '' } } },
      ]
      return new Response(events.map((item) => JSON.stringify(item)).join('\n') + '\n', { status: 200, headers: { 'content-type': 'application/x-ndjson' } })
    }
    return json({ message: 'Not implemented in test fixture' }, 404)
  })
  vi.stubGlobal('fetch', withBrowserResume(fetchMock))
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchMock.mockReset()
  localStorage.clear()
  sessionStorage.clear()
})

function TestHub({ initial = 'projects' }: { initial?: HubPage }) {
  const [page, setPage] = useState<HubPage>(initial)
  return (
    <WorkspaceHub
      page={page}
      onPageChange={setPage}
      onClose={vi.fn()}
      onOpenSubagents={vi.fn()}
      sessionId={SESSION}
      workspaces={{
        available: true, session_scope_available: false, memory_available: false,
        workspaces: [{ id: 'ws1', label: 'Application', status: 1, project_root: '/safe/workspace', created_at_us: 0 }],
      }}
      models={[{ id: 'gpt-5.6', name: 'GPT-5.6', provider: 'openai' }]}
      inventory={{ commands: [], agents: [{ name: 'reviewer', description: 'Code review', read_only: true }] }}
      onChooseModel={vi.fn()}
    />
  )
}

describe('authenticated native workspace pages', () => {
  it('shows real workspace metadata and switches into the read-only file tree', async () => {
    const user = userEvent.setup()
    render(<TestHub />)
    expect(screen.getByText('/safe/workspace')).not.toBeNull()
    expect(await screen.findByText('main-v2')).not.toBeNull()
    await user.click(screen.getByRole('button', { name: 'Files' }))
    await user.click(await screen.findByRole('button', { name: /src.*Folder/ }))
    await user.click(await screen.findByRole('button', { name: /App.tsx/ }))
    expect(await screen.findByText('<img onerror=alert(1)>')).not.toBeNull()
    expect(document.querySelector('img')).toBeNull()
    await user.click(screen.getByRole('button', { name: 'Show Git diff' }))
    expect(await screen.findByRole('region', { name: 'Read-only Git diff' })).not.toBeNull()
    expect(screen.getByText(/diff --git a\/src\/App.tsx/)).not.toBeNull()
  })

  it('manages durable tasks using native PUT; never just a local checklist', async () => {
    const user = userEvent.setup()
    render(<TestHub initial="tasks" />)
    expect(await screen.findByText('Review code')).not.toBeNull()
    await user.selectOptions(screen.getByRole('combobox', { name: 'Status for Review code' }), 'completed')
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(
      `/api/sessions/${SESSION}/todo`,
      expect.objectContaining({ method: 'PUT', body: expect.stringContaining('"status":"completed"') }),
    ))
  })

  it('saves provider credentials only to the backend, without echoing the password', async () => {
    const user = userEvent.setup()
    render(<TestHub initial="providers" />)
    await screen.findByText('Dev')
    await user.type(screen.getByLabelText('Provider ID'), 'demo')
    await user.type(screen.getByLabelText('Display name'), 'Demo')
    await user.clear(screen.getByLabelText('API base URL'))
    await user.type(screen.getByLabelText('API base URL'), 'https://api.example.test/v1')
    await user.type(screen.getByLabelText(/API key/), 'secret-fixture')
    await user.click(screen.getByRole('button', { name: /Save provider natively/ }))
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(
      '/api/providers/demo',
      expect.objectContaining({ method: 'POST', body: expect.stringContaining('"api_key":"secret-fixture"') }),
    ))
    expect((screen.getByLabelText(/API key/) as HTMLInputElement).value).toBe('')
  })

  it('registers MCP local tools disabled and uses the exact authenticated endpoint', async () => {
    const user = userEvent.setup()
    render(<TestHub initial="mcp" />)
    expect(await screen.findByText('local-test')).not.toBeNull()
    await user.type(screen.getByLabelText('Server ID'), 'newserver')
    await user.click(screen.getByRole('button', { name: 'Register disabled' }))
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(
      '/api/mcp', expect.objectContaining({ method: 'POST', body: expect.stringContaining('"enabled":false') }),
    ))
  })

  it('stores human permission preferences through native POST', async () => {
    const user = userEvent.setup()
    render(<TestHub initial="permissions" />)
    await screen.findByText('No custom rules. Native approvals remain in effect.')
    await user.selectOptions(screen.getByLabelText('Decision'), 'deny')
    await user.click(screen.getByRole('button', { name: /Save rule/ }))
    await waitFor(() => expect(fetchMock).toHaveBeenCalledWith(
      '/api/permission-rules',
      expect.objectContaining({ method: 'POST', body: expect.stringContaining('"decision":"deny"') }),
    ))
  })

  it('displays native streamed shell output instead of a fake terminal', async () => {
    const user = userEvent.setup()
    render(<TestHub initial="jobs" />)
    await user.type(screen.getByLabelText('Command'), 'pwd')
    await user.click(screen.getByRole('button', { name: /Run with native permissions/ }))
    expect(await screen.findByText(/Native shell stream completed/)).not.toBeNull()
    expect(screen.getByText(/\/safe\/workspace/)).not.toBeNull()
    expect(fetchMock).toHaveBeenCalledWith(
      `/api/sessions/${SESSION}/shell/stream`,
      expect.objectContaining({ method: 'POST', body: JSON.stringify({ command: 'pwd' }) }),
    )
  })
})
