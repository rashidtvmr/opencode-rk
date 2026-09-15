import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'
import { cleanup, render, screen } from '@testing-library/react'
import axe from 'axe-core'
import App from './App'

const fetchMock = vi.fn()

beforeEach(() => {
  fetchMock.mockImplementation(async (input: RequestInfo | URL) => {
    const url = String(input)

    if (url.endsWith('/health')) {
      return new Response(
        JSON.stringify({ schema_version: 1, status: 'ok', runtime: 'native-rust' }),
        { status: 200, headers: { 'content-type': 'application/json' } },
      )
    }

    if (url.includes('/api/sessions')) {
      return new Response(
        JSON.stringify({
          sessions: [
            {
              id: '0195f36a-2997-7a89-a11a-fc3359b0bd6e',
              title: 'Accessibility pass',
              archived: false,
            },
          ],
        }),
        { status: 200, headers: { 'content-type': 'application/json' } },
      )
    }

    if (url.includes('/api/models')) {
      return new Response(
        JSON.stringify({
          schema_version: 1,
          source: 'models.dev',
          count: 1,
          models: [{ id: 'gpt-5.6', provider: 'openai', name: 'GPT-5.6' }],
        }),
        { status: 200, headers: { 'content-type': 'application/json' } },
      )
    }

    return new Response(JSON.stringify({ code: 'not_found', message: 'missing fixture' }), {
      status: 404,
      headers: { 'content-type': 'application/json' },
    })
  })

  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  fetchMock.mockReset()
})

describe('web application shell', () => {
  it('provides the landmarks and skip navigation needed for keyboard users', async () => {
    render(<App />)

    expect(screen.getByRole('link', { name: /skip to main content/i }).getAttribute('href')).toBe(
      '#main-content',
    )
    expect(screen.getByRole('navigation', { name: /primary/i })).not.toBeNull()
    expect(screen.getByRole('complementary', { name: /sessions/i })).not.toBeNull()
    expect(screen.getByRole('main').id).toBe('main-content')
    expect(await screen.findByRole('heading', { name: /accessibility pass/i })).not.toBeNull()
  })

  it('renders the initial shell without automated axe violations', async () => {
    const { container } = render(<App />)
    await screen.findByRole('main')

    const result = await axe.run(container, {
      rules: {
        'color-contrast': { enabled: false },
      },
    })

    expect(result.violations).toEqual([])
  })
})
