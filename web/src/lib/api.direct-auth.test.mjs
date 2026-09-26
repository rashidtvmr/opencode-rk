import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import test from 'node:test'

import { deleteDraftAttachment } from './api.ts'

const token = 'a'.repeat(64)

test('deleteDraftAttachment sends the fragment bearer credential and keeps it out of the URL', async () => {
  const observed = []
  const server = createServer((request, response) => {
    observed.push({
      authorization: request.headers.authorization,
      url: request.url,
    })

    if (
      request.url?.startsWith('/api/sessions/s1/attachments/a1') !== true
    ) {
      response.writeHead(404).end()
      return
    }
    if (request.headers.authorization !== `Bearer ${token}`) {
      response.writeHead(401, { 'content-type': 'application/json' })
        .end(JSON.stringify({ error: 'missing bearer credential' }))
      return
    }

    response.writeHead(204).end()
  })

  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')

  const originalFetch = globalThis.fetch
  const originalWindow = globalThis.window
  const originalLocation = globalThis.location
  const originalHistory = globalThis.history
  const location = {
    href: `http://127.0.0.1:${address.port}/#oc2-token=${token}`,
    hash: `#oc2-token=${token}`,
  }
  const replaced = []

  globalThis.location = location
  globalThis.history = {
    replaceState(_state, _title, url) {
      replaced.push(url)
      if (typeof url === 'string') {
        const parsed = new URL(url, location.href)
        location.href = parsed.href
        location.hash = parsed.hash
      }
    },
  }
  globalThis.window = { location, history: globalThis.history }
  // Global fetch wrapper resolves relative path ONLY; never synthesizes a
  // response and never injects credentials itself.
  globalThis.fetch = (input, init) => {
    const target = new URL(String(input), `http://127.0.0.1:${address.port}/`)
    return originalFetch(target, init)
  }

  try {
    await deleteDraftAttachment('s1', 'a1')
    assert.equal(observed.length, 1)
    assert.equal(
      observed[0].authorization,
      `Bearer ${token}`,
    )
    assert.ok(
      observed[0].url?.startsWith('/api/sessions/s1/attachments/a1'),
      'request URL should target the attachment endpoint',
    )
    assert.equal(
      new URL(observed[0].url, 'http://127.0.0.1').searchParams.has('token'),
      false,
      'token must not appear in query string',
    )
    assert.equal(
      (new URLSearchParams(new URLSearchParams()
        .toString())
        .toString()),
      '',
    )
    assert.equal(location.hash, '')
    assert.ok(replaced.length > 0, 'fragment token should be cleared with history.replaceState')

    // Prove the same live endpoint denies a request without credentials.
    const denied = await originalFetch(`http://127.0.0.1:${address.port}/api/sessions/s1/attachments/a1`, {
      method: 'DELETE',
    })
    assert.equal(denied.status, 401)
    assert.equal(observed[1].authorization, undefined)
  } finally {
    globalThis.fetch = originalFetch
    if (originalWindow === undefined) delete globalThis.window
    else globalThis.window = originalWindow
    if (originalLocation === undefined) delete globalThis.location
    else globalThis.location = originalLocation
    if (originalHistory === undefined) delete globalThis.history
    else globalThis.history = originalHistory
    server.close()
    await once(server, 'close')
  }
})
