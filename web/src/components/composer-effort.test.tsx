import { afterEach, describe, expect, it, vi } from 'vitest'
import { cleanup, fireEvent, render, screen } from '@testing-library/react'

import {
  ALL_EFFORTS,
  COMPOSER_EFFORT_EMPTY_LABEL,
  EFFORT_LEVELS,
  providerHint,
  ComposerEffort,
} from './composer-effort'

const models = [
  { id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' },
  { id: 'claude-opus', provider: 'anthropic', name: 'Claude' },
]

afterEach(cleanup)

describe('WEB-HINT six-effort menu', () => {
  it('lists all six server efforts including none and minimal', () => {
    expect(ALL_EFFORTS).toEqual(['none', 'minimal', 'low', 'medium', 'high', 'xhigh'])
    const labels = EFFORT_LEVELS.map((level) => level.label)
    expect(labels).toEqual(['None', 'Minimal', 'Low', 'Medium', 'High', 'Extra High'])
    render(
      <ComposerEffort
        models={models}
        selectedModel="openai/gpt-5.6"
        effort="high"
        onEffortChange={vi.fn()}
      />,
    )
    for (const label of labels) {
      expect(screen.getByRole('option', { name: label })).not.toBeNull()
    }
  })

  it('selecting none notifies the owner with the exact value', () => {
    const onEffortChange = vi.fn()
    render(
      <ComposerEffort
        models={models}
        selectedModel="openai/gpt-5.6"
        effort="high"
        onEffortChange={onEffortChange}
      />,
    )
    fireEvent.click(screen.getByRole('option', { name: 'None' }))
    expect(onEffortChange).toHaveBeenCalledTimes(1)
    expect(onEffortChange).toHaveBeenCalledWith('none')
  })

  it('selecting minimal via keyboard notifies the owner', () => {
    const onEffortChange = vi.fn()
    render(
      <ComposerEffort
        models={models}
        selectedModel="openai/gpt-5.6"
        effort="high"
        onEffortChange={onEffortChange}
      />,
    )
    fireEvent.keyDown(screen.getByRole('option', { name: 'Minimal' }), { key: 'Enter' })
    expect(onEffortChange).toHaveBeenCalledWith('minimal')
  })
})

describe('WEB-HINT provider hint', () => {
  it('warns that a non-openai model has no native turn adapter', () => {
    expect(providerHint('anthropic/claude-opus')).toContain(
      'does not have a native turn adapter yet',
    )
    render(
      <ComposerEffort
        models={models}
        selectedModel="anthropic/claude-opus"
        effort="high"
        onEffortChange={vi.fn()}
      />,
    )
    expect(screen.getByRole('status').textContent).toContain(
      "provider 'anthropic' does not have a native turn adapter yet",
    )
  })

  it('shows no hint for the supported openai provider', () => {
    expect(providerHint('openai/gpt-5.6')).toBeNull()
    render(
      <ComposerEffort
        models={models}
        selectedModel="openai/gpt-5.6"
        effort="high"
        onEffortChange={vi.fn()}
      />,
    )
    expect(screen.queryByRole('status')).toBeNull()
  })

  it('flags a malformed model selector that the server would reject', () => {
    expect(providerHint('gpt-5.6')).toContain('provider/model format')
    render(
      <ComposerEffort
        models={models}
        selectedModel="gpt-5.6"
        effort="high"
        onEffortChange={vi.fn()}
      />,
    )
    expect(screen.getByRole('status').textContent).toContain('provider/model format')
  })
})

describe('WEB-HINT empty state', () => {
  it('renders a placeholder when no models are reported', () => {
    render(<ComposerEffort models={[]} selectedModel="" effort="high" onEffortChange={vi.fn()} />)
    expect(screen.getByText(COMPOSER_EFFORT_EMPTY_LABEL)).not.toBeNull()
  })
})

describe('WEB-HINT denied path', () => {
  it('an unknown effort selects nothing and causes no side effects', () => {
    const onEffortChange = vi.fn()
    render(
      <ComposerEffort
        models={models}
        selectedModel="openai/gpt-5.6"
        effort="ghost"
        onEffortChange={onEffortChange}
      />,
    )
    for (const option of screen.getAllByRole('option')) {
      expect(option.getAttribute('aria-selected')).toBe('false')
    }
    expect(onEffortChange).not.toHaveBeenCalled()
  })
})
