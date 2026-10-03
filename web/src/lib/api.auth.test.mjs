import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import test from 'node:test'

import { listSessions } from './api.ts'

const token = 'a'.repeat(64)
const capability = 'B'.repeat(43)

async function readBody(request) {
  const chunks = []
  let bytes = 0
  for await (const chunk of request) {
    bytes += chunk.length
    if (bytes > 4096) throw new Error('test request body exceeds fixture limit')
    chunks.push(chunk)
  }
  return Buffer.concat(chunks).toString()
}

test('listSessions uses a separate browser capability and removes the launch fragment', async () => {
  const observed = []
  let origin = ''
  const server = createServer(async (request, response) => {
    let body
    try {
      body = await readBody(request)
    } catch {
      response.writeHead(413).end()
      return
    }
    const record = {
      authorization: request.headers.authorization,
      method: request.method,
      origin: request.headers.origin,
      url: request.url,
      body,
    }
    observed.push(record)
    const send = (status, payload) => response.writeHead(status, { 'content-type': 'application/json' }).end(JSON.stringify(payload))

    if (record.url === '/auth/browser-resume' && record.method === 'POST') {
      if (record.origin !== origin) return send(403, { message: 'origin denied' })
      if (record.authorization !== `Bearer ${token}` || record.body !== '{}') return send(401, { message: 'trusted launch required' })
      return send(201, { browser_capability: capability, expires_at_unix_ms: Date.now() + 900000 })
    }
    if (record.url === '/api/sessions' && record.method === 'GET') {
      if (record.origin !== undefined && record.origin !== origin) return send(403, { message: 'origin denied' })
      if (record.authorization !== `BrowserResume ${capability}`) {
        return send(record.authorization ? 403 : 401, { error: 'missing bearer credential' })
      }
      return send(200, { sessions: [{ id: 's1', title: 'Real session' }] })
    }
    return send(404, { message: 'not found' })
  })

  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')
  origin = `http://127.0.0.1:${address.port}`

  const originalFetch = globalThis.fetch
  const originalWindow = globalThis.window
  const originalLocation = globalThis.location
  const originalHistory = globalThis.history
  const originalLocalStorage = globalThis.localStorage
  const originalSessionStorage = globalThis.sessionStorage
  const originalConsole = { log: console.log, warn: console.warn, error: console.error, info: console.info }
  const location = { href: `${origin}/#oc2-token=${token}`, hash: `#oc2-token=${token}` }
  const historyWrites = []
  const makeStorage = () => ({
    length: 0,
    getItem: () => null,
    setItem() {},
    removeItem() {},
    clear() {},
    key: () => null,
  })
  const localStorage = makeStorage()
  const sessionStorage = makeStorage()
  const storageWrites = []
  for (const [name, storage] of [['localStorage', localStorage], ['sessionStorage', sessionStorage]]) {
    storage.setItem = (key, value) => storageWrites.push({ name, key, value: String(value) })
  }
  const consoleCalls = []
  const recordConsole = (level) => (...args) => consoleCalls.push({ level, text: args.map(String).join(' ') })

  globalThis.location = location
  globalThis.history = {
    state: null,
    replaceState(state, _title, url) {
      historyWrites.push(String(url))
      if (typeof url === 'string') {
        const parsed = new URL(url, location.href)
        location.href = parsed.href
        location.hash = parsed.hash
      }
    },
  }
  globalThis.localStorage = localStorage
  globalThis.sessionStorage = sessionStorage
  globalThis.window = { location, history: globalThis.history, localStorage, sessionStorage }
  console.log = recordConsole('log')
  console.warn = recordConsole('warn')
  console.error = recordConsole('error')
  console.info = recordConsole('info')
  globalThis.fetch = (input, init = {}) => {
    const headers = new Headers(init.headers)
    const method = (init.method ?? 'GET').toUpperCase()
    if (method !== 'GET' && method !== 'HEAD') headers.set('origin', new URL(location.href).origin)
    return originalFetch(new URL(String(input), `${origin}/`), { ...init, headers })
  }

  try {
    const sessions = await listSessions()
    assert.deepEqual(sessions.map(({ id, title }) => ({ id, title })), [{ id: 's1', title: 'Real session' }])
    assert.equal(observed.length, 2)
    assert.deepEqual(
      { method: observed[0].method, url: observed[0].url, authorization: observed[0].authorization, body: observed[0].body, origin: observed[0].origin },
      { method: 'POST', url: '/auth/browser-resume', authorization: `Bearer ${token}`, body: '{}', origin },
    )
    assert.equal(observed[1].method, 'GET')
    assert.equal(observed[1].url, '/api/sessions')
    assert.equal(observed[1].authorization, `BrowserResume ${capability}`)
    assert.equal(new URL(observed[1].url, origin).searchParams.has('token'), false)
    assert.equal(location.hash, '')
    assert.equal(location.href.includes(token), false)
    assert.ok(historyWrites.length > 0, 'fragment token should be cleared with history.replaceState')
    assert.ok(historyWrites.every((url) => !url.includes(token) && !url.includes(capability)))
    assert.ok(storageWrites.every(({ value }) => !value.includes(token)))
    assert.equal(storageWrites.length, 1)
    assert.equal(storageWrites[0].name, 'sessionStorage')
    assert.ok(consoleCalls.every(({ text }) => !text.includes(token) && !text.includes(capability)))
    assert.ok(observed.every(({ url }) => !url.includes(token) && !url.includes(capability)))

    const denied = await originalFetch(`${origin}/api/sessions`)
    assert.equal(denied.status, 401)
    assert.equal(observed[2].authorization, undefined)
  } finally {
    globalThis.fetch = originalFetch
    if (originalWindow === undefined) delete globalThis.window
    else globalThis.window = originalWindow
    if (originalLocation === undefined) delete globalThis.location
    else globalThis.location = originalLocation
    if (originalHistory === undefined) delete globalThis.history
    else globalThis.history = originalHistory
    if (originalLocalStorage === undefined) delete globalThis.localStorage
    else globalThis.localStorage = originalLocalStorage
    if (originalSessionStorage === undefined) delete globalThis.sessionStorage
    else globalThis.sessionStorage = originalSessionStorage
    console.log = originalConsole.log
    console.warn = originalConsole.warn
    console.error = originalConsole.error
    console.info = originalConsole.info
    server.close()
    await once(server, 'close')
  }
})
