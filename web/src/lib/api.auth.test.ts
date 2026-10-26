import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const TOKEN = 'b'.repeat(64)
const fetchMock = vi.fn()

beforeEach(() => {
  vi.resetModules()
  fetchMock.mockReset()
  window.sessionStorage.clear()
  fetchMock.mockImplementation(async () => new Response(JSON.stringify({ models: [] }), {
    headers: { 'content-type': 'application/json' },
  }))
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  vi.unstubAllGlobals()
  window.sessionStorage.clear()
  window.history.replaceState(null, '', '/')
})

describe('browser daemon credential boundary', () => {
  it('rejects an unauthenticated API request before fetching', async () => {
    window.history.replaceState(null, '', '/')
    const { listModels } = await import('./api')

    await expect(listModels()).rejects.toThrow('Re-authentication required')
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('rejects a malformed fragment before any unauthenticated fetch', async () => {
    window.history.replaceState(null, '', '/#oc2-token=invalid')
    const { listModels } = await import('./api')

    await expect(listModels()).rejects.toThrow('Re-authentication required')
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('exchanges a trusted native token for a scoped browser capability without leaking the token into API requests', async () => {
    window.history.replaceState(null, '', `/chat?view=fixture#oc2-token=${TOKEN}`)
    fetchMock.mockImplementation(async (input: RequestInfo | URL) => {
      const path = String(input)
      const payload = path === '/auth/browser-resume'
        ? { browser_capability: 'v'.repeat(43) }
        : path === '/auth/browser-resume/exchange'
          ? { browser_capability: 'w'.repeat(43) }
          : { models: [] }
      return new Response(JSON.stringify(payload), { headers: { 'content-type': 'application/json' } })
    })
    const { listModels } = await import('./api')

    await expect(listModels()).resolves.toEqual([])
    await expect(listModels()).resolves.toEqual([])
    expect(window.location.pathname).toBe('/chat')
    expect(window.location.search).toBe('?view=fixture')
    expect(window.location.hash).toBe('')
    expect(fetchMock).toHaveBeenCalledTimes(3)
    expect(fetchMock.mock.calls[0][0]).toBe('/auth/browser-resume')
    expect(new Headers(fetchMock.mock.calls[0][1].headers).get('authorization')).toBe(`Bearer ${TOKEN}`)
    for (const [url, init] of fetchMock.mock.calls.slice(1)) {
      expect(String(url)).not.toContain(TOKEN)
      expect(new Headers(init.headers).get('authorization')).toBe(`BrowserResume ${'v'.repeat(43)}`)
    }

    vi.resetModules()
    const freshDocumentApi = await import('./api')
    await expect(freshDocumentApi.listModels()).resolves.toEqual([])
    expect(fetchMock.mock.calls[3][0]).toBe('/auth/browser-resume/exchange')
    expect(new Headers(fetchMock.mock.calls[3][1].headers).get('authorization')).toBe(`BrowserResume ${'v'.repeat(43)}`)
    expect(new Headers(fetchMock.mock.calls[4][1].headers).get('authorization')).toBe(`BrowserResume ${'w'.repeat(43)}`)
    expect(fetchMock).toHaveBeenCalledTimes(5)
  })
})
