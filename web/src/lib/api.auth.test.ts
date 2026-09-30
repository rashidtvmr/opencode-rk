import { afterEach, beforeEach, describe, expect, it, vi } from 'vitest'

const TOKEN = 'b'.repeat(64)
const fetchMock = vi.fn()

beforeEach(() => {
  vi.resetModules()
  fetchMock.mockReset()
  fetchMock.mockImplementation(async () => new Response(JSON.stringify({ models: [] }), {
    headers: { 'content-type': 'application/json' },
  }))
  vi.stubGlobal('fetch', fetchMock)
})

afterEach(() => {
  vi.unstubAllGlobals()
  window.history.replaceState(null, '', '/')
})

describe('browser daemon credential boundary', () => {
  it('rejects an unauthenticated API request before fetching', async () => {
    window.history.replaceState(null, '', '/')
    const { listModels } = await import('./api')

    await expect(listModels()).rejects.toThrow('Re-authentication required')
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('rejects a malformed launcher credential before fetching', async () => {
    window.history.replaceState(null, '', '/#oc2-token=invalid')
    const { listModels } = await import('./api')

    await expect(listModels()).rejects.toThrow('Re-authentication required')
    expect(fetchMock).not.toHaveBeenCalled()
  })

  it('consumes a valid fragment and retains the bearer only for this document', async () => {
    window.history.replaceState(null, '', `/chat?view=fixture#oc2-token=${TOKEN}`)
    const { listModels } = await import('./api')

    await expect(listModels()).resolves.toEqual([])
    await expect(listModels()).resolves.toEqual([])
    expect(window.location.pathname).toBe('/chat')
    expect(window.location.search).toBe('?view=fixture')
    expect(window.location.hash).toBe('')
    expect(fetchMock).toHaveBeenCalledTimes(2)
    for (const [url, init] of fetchMock.mock.calls) {
      expect(String(url)).not.toContain(TOKEN)
      expect(new Headers(init.headers).get('authorization')).toBe(`Bearer ${TOKEN}`)
    }

    vi.resetModules()
    const freshDocumentApi = await import('./api')
    await expect(freshDocumentApi.listModels()).rejects.toThrow('Re-authentication required')
    expect(fetchMock).toHaveBeenCalledTimes(2)
  })
})
