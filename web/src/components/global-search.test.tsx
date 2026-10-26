import { afterEach, expect, it, vi } from 'vitest'
import { cleanup, render, screen, waitFor } from '@testing-library/react'
import userEvent from '@testing-library/user-event'
import { GlobalSearch } from './global-search'
import { withBrowserResume } from '@/test/browser-resume-fixture'

const SESSION = '0195f36a-2997-7a89-a11a-fc3359b0aa99'

afterEach(() => {
  cleanup()
  vi.unstubAllGlobals()
  window.sessionStorage.clear()
})

it('queries authenticated native global search and opens the actual conversation', async () => {
  const user = userEvent.setup()
  const calls = vi.fn(async (input: RequestInfo | URL) => {
    const url = String(input)
    if (url === '/api/search?q=review') return new Response(JSON.stringify({
      sessions: [{ id: SESSION, title: 'Review project' }],
      messages: [{ id: 'one', session_id: SESSION, session_title: 'Review project',
        role: 'user', snippet: '<img src=x onerror=alert(1)> review' }],
      limitations: 'inline only',
    }), { status: 200, headers: { 'content-type': 'application/json' } })
    if (url.includes('/files/lookup?q=review')) return new Response(JSON.stringify({ results: [] }), {
      status: 200, headers: { 'content-type': 'application/json' },
    })
    return new Response(JSON.stringify({ message: 'not found' }), {
      status: 404, headers: { 'content-type': 'application/json' },
    })
  })
  vi.stubGlobal('fetch', withBrowserResume(calls))
  const open = vi.fn()
  const close = vi.fn()
  const { container } = render(<GlobalSearch sessionId={SESSION} agents={[]}
    onClose={close} onOpenChat={open} />)
  await user.type(screen.getByRole('searchbox', { name: /Search conversations/ }), 'review')
  const matches = await screen.findAllByRole('button', { name: /Review project/ })
  expect(container.querySelector('img')).toBeNull()
  expect(screen.getByText(/<img src=x onerror=alert\(1\)> review/)).not.toBeNull()
  await user.click(matches[0])
  expect(open).toHaveBeenCalledWith(SESSION)
  expect(close).toHaveBeenCalledOnce()
  await waitFor(() => expect(calls).toHaveBeenCalledWith('/api/search?q=review', expect.anything()))
})
