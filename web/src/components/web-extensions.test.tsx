import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Composer } from './composer'
import { SettingsPanel } from './settings-panel'
import { ToolTimeline, deriveStoredToolTimeline } from './tool-timeline'
import { MessageMarkdown } from './message-markdown'
import { SubagentPanel } from './subagent-panel'
import { withBrowserResume } from '@/test/browser-resume-fixture'
import type { WebPreferences } from '@/lib/web-preferences'
import { listHistoryPage } from '@/lib/api'

const models = [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }]
const preferences: WebPreferences = {
  theme: 'dark',
  density: 'comfortable',
  textSize: 'normal',
  reducedMotion: false,
}

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  localStorage.clear()
  sessionStorage.clear()
})

describe('working composer discovery', () => {
  it('suggests and inserts configured slash commands from backend inventory', async () => {
    const user = userEvent.setup()
    render(<Composer
      sessionId="chat-slash"
      models={models}
      selectedModel="openai/gpt-5.6"
      onModelChange={vi.fn()}
      commands={[{ name: 'review', description: 'Check changed files', agent: 'reviewer', model: null }]}
    />)
    const editor = screen.getByRole('textbox', { name: 'Message' })
    await user.click(editor)
    await user.type(editor, '/rev')
    expect(screen.getByRole('listbox', { name: 'Slash commands' })).not.toBeNull()
    await user.click(screen.getByRole('option', { name: /review/ }))
    expect(editor.textContent).toContain('/review ')
  })

  it('suggests and inserts agent mentions using the keyboard', async () => {
    const user = userEvent.setup()
    render(<Composer
      sessionId="chat-agents"
      models={models}
      selectedModel="openai/gpt-5.6"
      onModelChange={vi.fn()}
      agents={[{ name: 'reviewer', description: 'Review code', read_only: true }]}
    />)
    const editor = screen.getByRole('textbox', { name: 'Message' })
    await user.click(editor)
    await user.type(editor, '@rev')
    expect(screen.getByRole('listbox', { name: 'Agent mentions' })).not.toBeNull()
    await user.keyboard('{ArrowDown}{Enter}')
    expect(editor.textContent).toContain('@reviewer ')
  })

  it('dismisses autocomplete on Escape until the user edits the draft again', async () => {
    const user = userEvent.setup()
    render(<Composer
      sessionId="chat-escape"
      models={models}
      selectedModel="openai/gpt-5.6"
      onModelChange={vi.fn()}
      agents={[{ name: 'reviewer', description: 'Review code', read_only: true }]}
    />)
    const editor = screen.getByRole('textbox', { name: 'Message' })
    await user.click(editor)
    await user.type(editor, '@rev')
    expect(screen.getByRole('listbox', { name: 'Agent mentions' })).not.toBeNull()
    await user.keyboard('{Escape}')
    expect(screen.queryByRole('listbox', { name: 'Agent mentions' })).toBeNull()
    await user.type(editor, 'i')
    expect(screen.getByRole('listbox', { name: 'Agent mentions' })).not.toBeNull()
  })

  it('looks up read-only @file paths through the native session directory and inserts the chosen path', async () => {
    const user = userEvent.setup()
    const fetchMock = vi.fn(async (input: RequestInfo | URL) => {
      const path = String(input)
      if (path.endsWith('/files/lookup?q=App')) {
        return new Response(JSON.stringify({ results: ['src/App.tsx'] }), {
          status: 200, headers: { 'content-type': 'application/json' },
        })
      }
      return new Response(JSON.stringify({ message: 'not found' }), { status: 404, headers: { 'content-type': 'application/json' } })
    })
    vi.stubGlobal('fetch', withBrowserResume(fetchMock))
    render(<Composer
      sessionId="chat-files"
      models={models}
      selectedModel="openai/gpt-5.6"
      onModelChange={vi.fn()}
    />)
    const editor = screen.getByRole('textbox', { name: 'Message' })
    await user.click(editor)
    await user.type(editor, '@App')
    const option = await screen.findByRole('option', { name: /src\/App.tsx/ })
    await user.click(option)
    expect(editor.textContent).toContain('@src/App.tsx ')
    expect(fetchMock).toHaveBeenCalledWith('/api/sessions/chat-files/files/lookup?q=App', expect.anything())
  })
})

describe('developer panels', () => {
  it('rehydrates paginated durable tool calls and their failure statuses', async () => {
    const session = '0195f36a-2997-7a89-a11a-fc3359b0aa99'
    const mock = vi.fn(async () => new Response(JSON.stringify({ messages: [{
      id: 'typed-1', session_id: session, role: 'tool', body: { storage: 'inline', text: '[bash] failed' },
      parts: [
        { type: 'tool_call', call_id: 'call-1', name: 'bash', arguments: { command: 'exit 1' }, status: 'completed' },
        { type: 'tool_output', call_id: 'call-1', name: 'bash', output: 'permission denied', success: false },
      ],
    }], next_before: null }), {
      status: 200, headers: { 'content-type': 'application/json' },
    }))
    vi.stubGlobal('fetch', withBrowserResume(mock))
    const page = await listHistoryPage(session, 50)
    const tools = deriveStoredToolTimeline(page.messages)
    expect(tools.get('call-1')).toMatchObject({
      name: 'bash', arguments: '{"command":"exit 1"}', output: 'permission denied', state: 'failed',
    })
    render(<ToolTimeline tools={Array.from(tools.values())} />)
    expect(screen.getByText('Failed')).not.toBeNull()
  })

  it('renders headings, fenced code and tables without executing untrusted markdown HTML', () => {
    const { container } = render(<MessageMarkdown text={
      '# Review results\n\n**Safe** output.\n\n```ts\nconst safe = true\n```\n\n| File | State |\n| --- | --- |\n| src/App.tsx | clean |\n\n<img src=x onerror=alert(1)>'
    } />)
    expect(screen.getByRole('heading', { name: 'Review results' })).not.toBeNull()
    expect(screen.getByText('const safe = true')).not.toBeNull()
    expect(screen.getByRole('table')).not.toBeNull()
    expect(container.querySelector('img')).toBeNull()
    expect(screen.getByText('<img src=x onerror=alert(1)>')).not.toBeNull()
  })

  it('changes browser appearance through real settings controls', async () => {
    const user = userEvent.setup()
    const setPreferences = vi.fn()
    render(<SettingsPanel
      onClose={vi.fn()}
      preferences={preferences}
      setPreferences={setPreferences}
      models={models}
      capabilities={null}
      health={null}
    />)
    await user.click(screen.getByRole('radio', { name: /Ocean/i }))
    expect(setPreferences).toHaveBeenCalledWith({ theme: 'ocean' })
    await user.selectOptions(screen.getByLabelText('Display density'), 'compact')
    expect(setPreferences).toHaveBeenCalledWith({ density: 'compact' })
  })

  it('renders a structured terminal call and output without HTML injection', () => {
    const hostile = '<img src=x onerror=alert(1)>'
    const { container } = render(<ToolTimeline tools={[{
      id: 'tool-1', name: 'bash', arguments: '{"command":"ls"}', output: hostile, state: 'completed',
    }]} />)
    expect(screen.getByLabelText('Tool execution')).not.toBeNull()
    expect(screen.getByText('bash')).not.toBeNull()
    expect(container.querySelector('img')).toBeNull()
  })

  it('uses the live child registry and accepts queued follow-up instructions', async () => {
    const parent = '0195f36a-2997-7a89-a11a-fc3359b0fa01'
    const child = '0195f36a-2997-7a89-a11a-fc3359b0fa02'
    const snapshot = {
      session_id: child, parent_id: parent, description: 'Audit security', agent: 'reviewer',
      model: 'openai/gpt-5.6', state: 'running', output_tail: 'Inspecting code',
      error: null, read_only: true, has_tool_calls: false, generation: 1,
      auto_retry_attempts: 0, manual_retries: 0, pending_followups: 0,
      awaiting_permission: false,
    }
    const mock = vi.fn(async (input: RequestInfo | URL) => {
      const path = String(input)
      return new Response(JSON.stringify(path.endsWith('/continue')
        ? { ...snapshot, pending_followups: 1 }
        : { children: [snapshot], registry_scope: 'current daemon lifetime' }), {
        status: 200, headers: { 'content-type': 'application/json' },
      })
    })
    vi.stubGlobal('fetch', withBrowserResume(mock))
    const user = userEvent.setup()
    const open = vi.fn()
    render(<SubagentPanel sessionId={parent} onClose={vi.fn()} onOpenSession={open} />)
    expect(await screen.findByText('Audit security')).not.toBeNull()
    await user.click(screen.getByRole('button', { name: 'Follow up' }))
    await user.type(screen.getByLabelText('Follow-up instruction'), 'Check authentication next')
    await user.click(screen.getByRole('button', { name: 'Queue follow-up' }))
    await waitFor(() => {
      expect(mock).toHaveBeenCalledWith(
        `/api/sessions/${parent}/children/${child}/continue`,
        expect.objectContaining({ method: 'POST', body: JSON.stringify({ text: 'Check authentication next' }) }),
      )
    })
    expect(await screen.findByText('1 queued follow-up(s)')).not.toBeNull()
  })
})
