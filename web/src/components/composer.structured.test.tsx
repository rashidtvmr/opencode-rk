import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'
import userEvent from '@testing-library/user-event'

import {
  Composer,
  MAX_COMPOSER_DRAFT_BYTES,
  lowerComposerDocument,
} from './composer'

const models = [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }]

afterEach(() => {
  cleanup()
  localStorage.clear()
})

describe('WEB-010 structured composer', () => {
  it('lowers text and code blocks deterministically to the native text request', () => {
    expect(
      lowerComposerDocument([
        { type: 'paragraph', text: 'Explain this' },
        { type: 'code', text: 'const value = 1' },
      ]),
    ).toBe('Explain this\n\n```\nconst value = 1\n```')
  })

  it('persists a bounded per-chat draft and restores it on remount', async () => {
    const props = {
      sessionId: 'session-draft',
      models,
      selectedModel: 'openai/gpt-5.6',
      onModelChange: vi.fn(),
    }
    const first = render(<Composer {...props} />)
    const editor = screen.getByRole('textbox', { name: 'Message' })
    editor.innerHTML = '<p>Persistent draft</p>'
    fireEvent.input(editor)
    first.unmount()

    render(<Composer {...props} />)
    expect(screen.getByRole('textbox', { name: 'Message' }).textContent).toContain('Persistent draft')
  })

  it('sanitizes pasted HTML to clipboard plain text and does not activate unsupported chips', async () => {
    const submit = vi.fn().mockResolvedValue(true)
    const user = userEvent.setup()
    render(
      <Composer
        sessionId="session-paste"
        models={models}
        selectedModel="openai/gpt-5.6"
        onModelChange={vi.fn()}
        onSubmit={submit}
      />,
    )
    const editor = screen.getByRole('textbox', { name: 'Message' })
    editor.focus()
    fireEvent.paste(editor, {
      clipboardData: {
        getData: (kind: string) => (kind === 'text/plain' ? 'safe text' : '<img src=x onerror=alert(1)>'),
      },
    })
    expect(editor.innerHTML).not.toContain('img')

    await user.click(screen.getByRole('button', { name: 'Commands and mentions' }))
    expect(screen.getByText('Slash commands · native endpoint unavailable').closest('[aria-disabled="true"]')).not.toBeNull()
  })

  it('shows a real stop control while a native turn is running', () => {
    const stop = vi.fn()
    render(
      <Composer
        sessionId="session-running"
        running
        models={models}
        selectedModel="openai/gpt-5.6"
        onModelChange={vi.fn()}
        onStop={stop}
      />,
    )
    expect(screen.getByRole('button', { name: 'Stop generating' })).not.toBeNull()
    expect(screen.getByRole('textbox', { name: 'Message' }).getAttribute('contenteditable')).toBe('false')
  })

  it('exposes the same 32 KiB draft bound as the native composer state', () => {
    expect(MAX_COMPOSER_DRAFT_BYTES).toBe(32_768)
  })
})
