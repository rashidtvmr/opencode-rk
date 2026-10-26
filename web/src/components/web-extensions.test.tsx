import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { Composer } from './composer'
import { SettingsPanel } from './settings-panel'
import { ToolTimeline } from './tool-timeline'
import { MessageMarkdown } from './message-markdown'
import { SubagentPanel } from './subagent-panel'
import { withBrowserResume } from '@/test/browser-resume-fixture'
import type { WebPreferences } from '@/lib/web-preferences'

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
})

describe('developer panels', () => {
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
