import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import test from 'node:test'

const TOKEN = 'a'.repeat(64)
const CAPABILITY = 'B'.repeat(43)
const ROTATED = 'C'.repeat(43)
const LIMIT_MS = 15 * 60 * 1000
const nativeFetch = globalThis.fetch

function startAuthServer() {
  const observed = []
  const leases = new Map()
  const state = { origin: '', expires: 0 }
  const server = createServer(async (request, response) => {
    const url = request.url ?? ''
    let body = ''
    let bodyBytes = 0
    for await (const chunk of request) {
      bodyBytes += chunk.length
      if (bodyBytes > 4096) {
        response.writeHead(413).end()
        return
      }
      body += chunk
    }
    const record = { url, method: request.method, authorization: request.headers.authorization, origin: request.headers.origin, body, status: 0 }
    observed.push(record)
    const send = (status, payload) => {
      record.status = status
      response.writeHead(status, { 'content-type': 'application/json' }).end(JSON.stringify(payload))
    }
    const mutatingOriginOK = record.origin === state.origin
    const readOriginOK = record.origin === undefined || mutatingOriginOK

    if (url === '/auth/browser-resume' && record.method === 'POST') {
      if (!mutatingOriginOK) return send(403, { message: 'origin denied' })
      if (record.authorization !== `Bearer ${TOKEN}` || record.body !== '{}') return send(record.authorization ? 403 : 401, { message: 'trusted launch required' })
      state.expires = Date.now() + LIMIT_MS
      leases.set(CAPABILITY, { expires: state.expires })
      return send(201, { browser_capability: CAPABILITY, expires_at_unix_ms: state.expires })
    }
    if (url === '/auth/browser-resume/exchange' && record.method === 'POST') {
      if (!mutatingOriginOK) return send(403, { message: 'origin denied' })
      const supplied = record.authorization?.startsWith('BrowserResume ')
        ? record.authorization.slice('BrowserResume '.length)
        : undefined
      const lease = leases.get(supplied)
      if (!lease || lease.expires <= Date.now() || record.body !== '{}') return send(403, { message: 'resume denied' })
      leases.delete(supplied)
      leases.set(ROTATED, lease)
      return send(200, { browser_capability: ROTATED, expires_at_unix_ms: lease.expires })
    }
    if (url === '/api/sessions' && record.method === 'GET') {
      const supplied = record.authorization?.startsWith('BrowserResume ')
        ? record.authorization.slice('BrowserResume '.length)
        : undefined
      const lease = leases.get(supplied)
      if (!readOriginOK || !lease || lease.expires <= Date.now()) {
        return send(record.authorization ? 403 : 401, { message: 'browser resume credential required' })
      }
      return send(200, { sessions: [{ id: 's1', title: 'Real session' }] })
    }
    if (url === '/health' && record.method === 'GET') {
      return send(200, { schema_version: 1, status: 'ok', runtime: 'native' })
    }
    return send(404, { message: 'not found' })
  })
  return { server, observed, leases, state }
}

async function listen(server, state) {
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')
  state.origin = `http://127.0.0.1:${address.port}`
  return address.port
}

function installBrowser({ hash = '', port, sessionEntries = new Map() }) {
  const original = {
    fetch: globalThis.fetch,
    window: globalThis.window,
    location: globalThis.location,
    history: globalThis.history,
    localStorage: globalThis.localStorage,
    sessionStorage: globalThis.sessionStorage,
    console: { log: console.log, warn: console.warn, error: console.error, info: console.info },
  }
  const origin = `http://127.0.0.1:${port}`
  const location = { href: `${origin}/${hash}`, hash }
  const historyWrites = []
  const history = {
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
  const storageWrites = []
  const makeStorage = (name, entries = new Map()) => ({
    name,
    entries,
    getItem: (key) => entries.get(key) ?? null,
    setItem(key, value) { storageWrites.push({ name, key, value: String(value) }); entries.set(key, String(value)) },
    removeItem: (key) => entries.delete(key),
    clear: () => entries.clear(),
    key: (index) => [...entries.keys()][index] ?? null,
    get length() { return entries.size },
  })
  const localStorage = makeStorage('localStorage')
  const sessionStorage = makeStorage('sessionStorage', sessionEntries)
  const consoleCalls = []
  const recordConsole = (level) => (...args) => consoleCalls.push({ level, text: args.map(String).join(' ') })
  const rawCalls = []

  globalThis.location = location
  globalThis.history = history
  globalThis.localStorage = localStorage
  globalThis.sessionStorage = sessionStorage
  globalThis.window = { location, history, localStorage, sessionStorage }
  console.log = recordConsole('log')
  console.warn = recordConsole('warn')
  console.error = recordConsole('error')
  console.info = recordConsole('info')
  globalThis.fetch = (input, init = {}) => {
    const raw = String(input)
    const headers = new Headers(init.headers)
    const method = (init.method ?? 'GET').toUpperCase()
    rawCalls.push({ raw, authorization: headers.get('authorization') })
    if (!headers.has('origin') && method !== 'GET' && method !== 'HEAD') headers.set('origin', origin)
    return original.fetch(new URL(raw, `${origin}/`), { ...init, headers })
  }

  return {
    location, historyWrites, storageWrites, localStorage, sessionStorage, consoleCalls, rawCalls,
    restore() {
      globalThis.fetch = original.fetch
      if (original.window === undefined) delete globalThis.window
      else globalThis.window = original.window
      if (original.location === undefined) delete globalThis.location
      else globalThis.location = original.location
      if (original.history === undefined) delete globalThis.history
      else globalThis.history = original.history
      if (original.localStorage === undefined) delete globalThis.localStorage
      else globalThis.localStorage = original.localStorage
      if (original.sessionStorage === undefined) delete globalThis.sessionStorage
      else globalThis.sessionStorage = original.sessionStorage
      console.log = original.console.log
      console.warn = original.console.warn
      console.error = original.console.error
      console.info = original.console.info
    },
  }
}

function assertNoTokenLeak(browser, ...tokens) {
  const evidence = JSON.stringify({
    history: browser.historyWrites,
    console: browser.consoleCalls,
    rawFetchInputs: browser.rawCalls.map(({ raw }) => raw),
  })
  for (const token of tokens) assert.equal(evidence.includes(token), false, 'credential leaked into history, console output or raw fetch URL')
  assert.ok(browser.storageWrites.every(({ value }) => !value.includes(TOKEN)), 'native bearer must never enter browser storage')
}

async function close(server) {
  if (!server.listening) return
  server.close()
  await once(server, 'close')
}

// Maps the old six intents: protected auth and fragment cleanup, mutation route/no-query/401,
// same-tab secrecy, exact reload exchange, malformed-fragment preflight, public health/path scope.
test('same-tab lifecycle issues once and reuses browser capability without leaks', async () => {
  const { server, observed, state } = startAuthServer()
  const port = await listen(server, state)
  const browser = installBrowser({ hash: `#oc2-token=${TOKEN}`, port })
  try {
    const api = await import('./api.ts?case=same-tab-corrected')
    const first = await api.listSessions()
    const second = await api.listSessions()
    const expected = [{ id: 's1', title: 'Real session' }]
    assert.deepEqual(first.map(({ id, title }) => ({ id, title })), expected)
    assert.deepEqual(second.map(({ id, title }) => ({ id, title })), expected)
    assert.equal(observed.length, 3)
    assert.deepEqual(
      { method: observed[0].method, url: observed[0].url, authorization: observed[0].authorization, body: observed[0].body, origin: observed[0].origin },
      { method: 'POST', url: '/auth/browser-resume', authorization: `Bearer ${TOKEN}`, body: '{}', origin: `http://127.0.0.1:${port}` },
    )
    for (const request of observed.slice(1)) {
      assert.equal(request.method, 'GET')
      assert.equal(request.url, '/api/sessions')
      assert.equal(request.authorization, `BrowserResume ${CAPABILITY}`)
      assert.equal(request.status, 200)
      assert.equal(new URL(request.url, `http://127.0.0.1:${port}`).searchParams.has('token'), false)
    }
    assert.equal(browser.location.hash, '')
    assert.equal(browser.location.href.includes(TOKEN), false)
    assert.ok(browser.historyWrites.length > 0)
    assert.ok(browser.historyWrites.every((url) => !url.includes(TOKEN) && !url.includes(CAPABILITY)))
    assert.equal(browser.localStorage.length, 0)
    assert.deepEqual(browser.storageWrites.map(({ name, key, value }) => ({ name, key, value })), [
      { name: 'sessionStorage', key: 'opencode.browser-resume', value: CAPABILITY },
    ])
    assertNoTokenLeak(browser, TOKEN, CAPABILITY)
    assert.ok(browser.rawCalls.every(({ raw }) => raw.startsWith('/')))

    const noCredential = await nativeFetch(`http://127.0.0.1:${port}/api/sessions`)
    assert.equal(noCredential.status, 401)
    const nativeCredential = await nativeFetch(`http://127.0.0.1:${port}/api/sessions`, { headers: { authorization: `Bearer ${TOKEN}` } })
    assert.equal(nativeCredential.status, 403, 'native launch bearer is not accepted on protected API')
    assert.equal(observed[3].authorization, undefined)
    assert.equal(observed[4].authorization, `Bearer ${TOKEN}`)
  } finally {
    browser.restore()
    await close(server)
  }
})

test('fresh module exchanges retained tab capability before protected API and rotates it', async () => {
  const { server, observed, leases, state } = startAuthServer()
  const port = await listen(server, state)
  const launchBrowser = installBrowser({ hash: `#oc2-token=${TOKEN}`, port })
  try {
    const launchApi = await import('./api.ts?case=reload-boot-corrected')
    assert.equal((await launchApi.listSessions()).length, 1)
    assert.equal(launchBrowser.location.hash, '')
    const entries = launchBrowser.sessionStorage.entries
    assert.equal(launchBrowser.sessionStorage.getItem('opencode.browser-resume'), CAPABILITY)
    const expiryBeforeExchange = leases.get(CAPABILITY)?.expires
    assert.ok(expiryBeforeExchange > Date.now())

    observed.length = 0
    const reloadBrowser = installBrowser({ port, sessionEntries: entries })
    try {
      const reloadApi = await import('./api.ts?case=reload-fresh-corrected')
      const sessions = await reloadApi.listSessions()
      assert.equal(sessions[0]?.id, 's1')
      assert.equal(observed.length, 2)
      assert.equal(observed[0].method, 'POST')
      assert.equal(observed[0].url, '/auth/browser-resume/exchange')
      assert.equal(observed[0].authorization, `BrowserResume ${CAPABILITY}`)
      assert.equal(observed[0].origin, state.origin)
      assert.equal(observed[0].body, '{}')
      assert.equal(observed[1].method, 'GET')
      assert.equal(observed[1].url, '/api/sessions')
      assert.equal(observed[1].authorization, `BrowserResume ${ROTATED}`)
      assert.equal(observed[0].status, 200)
      assert.equal(observed[1].status, 200)
      assert.equal(leases.has(CAPABILITY), false)
      assert.equal(leases.get(ROTATED)?.expires, expiryBeforeExchange, 'rotation preserves original absolute expiry')
      assert.equal(reloadBrowser.sessionStorage.getItem('opencode.browser-resume'), ROTATED)
      assertNoTokenLeak(launchBrowser, TOKEN, CAPABILITY)
      assertNoTokenLeak(reloadBrowser, TOKEN, CAPABILITY, ROTATED)
      assert.ok(reloadBrowser.rawCalls.every(({ raw }) => raw.startsWith('/')))
    } finally {
      reloadBrowser.restore()
    }
  } finally {
    launchBrowser.restore()
    await close(server)
  }
})

test('missing or malformed launch credential fails before protected API effects', async () => {
  const { server, observed, state } = startAuthServer()
  const port = await listen(server, state)
  const malformed = 'b'.repeat(32)
  const browser = installBrowser({ hash: `#oc2-token=${malformed}`, port })
  try {
    const api = await import('./api.ts?case=malformed-fragment-corrected')
    await assert.rejects(
      () => api.listSessions(),
      (error) => {
        assert.equal(error.status, 401)
        assert.match(`${error.name} ${error.message}`, /re-?auth|credential|unauthor/i)
        assert.equal(error.message.includes(malformed), false)
        return true
      },
    )
    assert.equal(observed.length, 0)
    assert.equal(browser.rawCalls.length, 0)
    assert.equal(browser.location.href.includes(malformed), true, 'malformed fragment remains in the current address because it is not accepted or copied')
    assertNoTokenLeak(browser, malformed)

    const noFragment = installBrowser({ port })
    try {
      const noFragmentApi = await import('./api.ts?case=missing-capability-corrected')
      await assert.rejects(noFragmentApi.listSessions(), /re-?auth|credential|unauthor/i)
      assert.equal(observed.length, 0)
      assert.equal(noFragment.rawCalls.length, 0)
    } finally {
      noFragment.restore()
    }
  } finally {
    browser.restore()
    await close(server)
  }
})

test('public health stays unauthenticated while local API uses browser capability', async () => {
  const { server, observed, state } = startAuthServer()
  const port = await listen(server, state)
  const browser = installBrowser({ hash: `#oc2-token=${TOKEN}`, port })
  try {
    const api = await import('./api.ts?case=health-scope-corrected')
    const sessions = await api.listSessions()
    const health = await api.getHealth()
    assert.equal(sessions.length, 1)
    assert.equal(health.status, 'ok')
    const issue = observed.find((request) => request.url === '/auth/browser-resume')
    assert.ok(issue)
    assert.equal(issue.authorization, `Bearer ${TOKEN}`)
    const sessionsRequest = observed.find((request) => request.url === '/api/sessions')
    assert.ok(sessionsRequest)
    assert.equal(sessionsRequest.authorization, `BrowserResume ${CAPABILITY}`)
    assert.equal(sessionsRequest.origin, undefined, 'same-origin GET omits Origin')
    const healthRequest = observed.find((request) => request.url === '/health')
    assert.ok(healthRequest)
    assert.equal(healthRequest.authorization, undefined)
    assert.equal(healthRequest.origin, undefined, 'same-origin GET omits Origin')
    assert.ok(browser.rawCalls.every(({ raw }) => raw.startsWith('/')), 'API calls use relative local paths')
    assertNoTokenLeak(browser, TOKEN, CAPABILITY)
    assert.ok(observed.every(({ url }) => !url.includes(TOKEN) && !url.includes(CAPABILITY)))
  } finally {
    browser.restore()
    await close(server)
  }
})
