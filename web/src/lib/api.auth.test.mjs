import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import test from 'node:test'

import { listSessions } from './api.ts'

const token = 'a'.repeat(64)

test('listSessions authenticates with the fragment token and removes it from the URL', async () => {
  const observed = []
  const server = createServer((request, response) => {
    observed.push({
      authorization: request.headers.authorization,
      url: request.url,
    })

    if (request.url?.startsWith('/api/sessions') !== true) {
      response.writeHead(404).end()
      return
    }
    if (request.headers.authorization !== `Bearer ${token}`) {
      response.writeHead(401, { 'content-type': 'application/json' })
        .end(JSON.stringify({ error: 'missing bearer credential' }))
      return
    }

    response.writeHead(200, { 'content-type': 'application/json' })
      .end(JSON.stringify({ sessions: [{ id: 's1', title: 'Real session' }] }))
  })

  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')

  const originalFetch = globalThis.fetch
  const originalWindow = globalThis.window
  const originalLocation = globalThis.location
  const originalHistory = globalThis.history
  const location = { href: `http://127.0.0.1:${address.port}/#oc2-token=${token}`, hash: `#oc2-token=${token}` }
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
  globalThis.fetch = (input, init) => {
    const target = new URL(String(input), `http://127.0.0.1:${address.port}/`)
    return originalFetch(target, init)
  }

  try {
    const sessions = await listSessions()
    assert.deepEqual(sessions.map(({ id, title }) => ({ id, title })), [
      { id: 's1', title: 'Real session' },
    ])
    assert.equal(observed.length, 1)
    assert.equal(observed[0].authorization, `Bearer ${token}`)
    assert.ok(observed[0].url?.startsWith('/api/sessions'))
    assert.equal(new URL(observed[0].url, 'http://127.0.0.1').searchParams.has('token'), false)
    assert.equal(location.hash, '')
    assert.ok(replaced.length > 0, 'fragment token should be cleared with history.replaceState')

    // Prove the same live endpoint denies a request without credentials.
    const denied = await originalFetch(`http://127.0.0.1:${address.port}/api/sessions`)
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
