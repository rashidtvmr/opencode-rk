import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import test from 'node:test'

// AUTHWEB-REFRESH-RED: browser credential lifecycle after the fragment is consumed.
//
// These tests drive the REAL module at web/src/lib/api.ts against a REAL bounded
// loopback HTTP server that binds 127.0.0.1 on an ephemeral port and enforces the
// same bearer contract the native daemon enforces for /api/* (crates/server daemon
// auth middleware). The server is the authority: it records every request's URL and
// Authorization header and returns 401 for /api/sessions without a bearer token.
//
// No response is synthesized, no credential is injected by the test harness, and no
// external network or real secret is used. The fetch wrapper only resolves the
// client's relative path to the local server so the real request can be observed.

const TOKEN = 'a'.repeat(64)

function startAuthServer() {
  const observed = []
  const server = createServer((request, response) => {
    const url = request.url ?? ''
    const record = { url, authorization: request.headers.authorization, status: 0 }
    observed.push(record)

    if (url === '/api/sessions') {
      if (record.authorization !== `Bearer ${TOKEN}`) {
        record.status = 401
        response.writeHead(401, { 'content-type': 'application/json' })
          .end(JSON.stringify({ message: 'missing bearer credential' }))
        return
      }
      record.status = 200
      response.writeHead(200, { 'content-type': 'application/json' })
        .end(JSON.stringify({ sessions: [{ id: 's1', title: 'Real session' }] }))
      return
    }

    if (url === '/health') {
      record.status = 200
      response.writeHead(200, { 'content-type': 'application/json' })
        .end(JSON.stringify({ schema_version: 1, status: 'ok', runtime: 'native' }))
      return
    }

    record.status = 404
    response.writeHead(404, { 'content-type': 'application/json' })
      .end(JSON.stringify({ message: 'not found' }))
  })
  return { server, observed }
}

async function listen(server) {
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')
  return address.port
}

// Minimal browser surface: location/history/storage plus console capture. Nothing
// here fabricates an API result or injects a credential.
function installBrowser({ hash, port }) {
  const original = {
    fetch: globalThis.fetch,
    window: globalThis.window,
    location: globalThis.location,
    history: globalThis.history,
    localStorage: globalThis.localStorage,
    sessionStorage: globalThis.sessionStorage,
    console: { log: console.log, warn: console.warn, error: console.error, info: console.info },
  }

  const location = { href: `http://127.0.0.1:${port}/${hash}`, hash }
  const replaced = []
  const history = {
    state: null,
    replaceState(state, _title, url) {
      replaced.push(url)
      if (typeof url === 'string') {
        const parsed = new URL(url, location.href)
        location.href = parsed.href
        location.hash = parsed.hash
      }
    },
  }

  const storageWrites = []
  const makeStorage = (name) => ({
    name,
    getItem() { return null },
    setItem(key, value) { storageWrites.push({ name, key, value: String(value) }) },
    removeItem() {},
    clear() {},
    key() { return null },
    get length() { return 0 },
  })

  const consoleCalls = []
  const recordConsole = (level) => (...args) => {
    consoleCalls.push({ level, text: args.map((value) => String(value)).join(' ') })
  }

  const rawCalls = []
  globalThis.location = location
  globalThis.history = history
  const localStorage = makeStorage('localStorage')
  const sessionStorage = makeStorage('sessionStorage')
  globalThis.window = { location, history, localStorage, sessionStorage }
  globalThis.localStorage = localStorage
  globalThis.sessionStorage = sessionStorage
  console.log = recordConsole('log')
  console.warn = recordConsole('warn')
  console.error = recordConsole('error')
  console.info = recordConsole('info')
  globalThis.fetch = (input, init) => {
    const raw = String(input)
    const headers = init?.headers ? new Headers(init.headers) : null
    rawCalls.push({ raw, authorization: headers?.get('authorization') ?? null })
    const target = new URL(raw, `http://127.0.0.1:${port}/`)
    return original.fetch(target, init)
  }

  const restore = () => {
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
  }

  return { location, replaced, storageWrites, consoleCalls, rawCalls, restore }
}

function assertNoTokenLeak(browser, token) {
  // The Authorization request header is the sanctioned carrier for the bearer
  // credential, so it is deliberately excluded here. Everything else (history
  // entries, storage writes, console output, and the raw fetch input/URL) must
  // never contain the credential.
  const evidence = JSON.stringify({
    replaced: browser.replaced,
    storageWrites: browser.storageWrites,
    consoleCalls: browser.consoleCalls,
    rawFetchInputs: browser.rawCalls.map((call) => call.raw),
  })
  assert.equal(
    evidence.includes(token),
    false,
    'the credential must not leak into history, storage, console output or the raw fetch input/URL',
  )
}

test('same-tab lifecycle: a consumed fragment keeps authenticating later API calls without leaking the token', async () => {
  const { server, observed } = startAuthServer()
  const port = await listen(server)
  const browser = installBrowser({ hash: `#oc2-token=${TOKEN}`, port })
  try {
    const api = await import('./api.ts?case=same-tab')
    const first = await api.listSessions()
    const second = await api.listSessions()
    const expected = [{ id: 's1', title: 'Real session' }]
    assert.deepEqual(first.map(({ id, title }) => ({ id, title })), expected)
    assert.deepEqual(second.map(({ id, title }) => ({ id, title })), expected)

    assert.equal(observed.length, 2, 'both same-tab calls must reach the live endpoint')
    for (const request of observed) {
      assert.equal(request.authorization, `Bearer ${TOKEN}`)
      assert.equal(request.url, '/api/sessions')
      assert.equal(new URL(request.url, 'http://127.0.0.1').searchParams.has('token'), false)
      assert.equal(request.url.includes('oc2-token'), false)
    }

    assert.equal(browser.location.hash, '')
    assert.equal(browser.location.href.includes(TOKEN), false)
    assert.ok(browser.replaced.length > 0, 'fragment token should be cleared with history.replaceState')
    for (const url of browser.replaced) {
      assert.equal(String(url).includes('oc2-token'), false)
      assert.equal(String(url).includes(TOKEN), false)
    }

    assertNoTokenLeak(browser, TOKEN)
    assert.ok(
      browser.rawCalls.every((call) => call.raw.startsWith('/')),
      'the client must only issue relative local paths',
    )
  } finally {
    browser.restore()
    server.close()
    await once(server, 'close')
  }
})

test('simulated full reload without fragment must use an explicit relaunch contract or fail with a clear re-auth state', async () => {
  const { server, observed } = startAuthServer()
  const port = await listen(server)
  const browser = installBrowser({ hash: `#oc2-token=${TOKEN}`, port })
  try {
    // Tab bootstrap: the fragment is consumed and cleared.
    const bootTab = await import('./api.ts?case=reload-boot')
    const booted = await bootTab.listSessions()
    assert.equal(booted.length, 1)
    assert.equal(browser.location.hash, '')

    // Simulate a full page reload: the fragment is gone and the in-memory
    // credential of the first tab no longer exists (fresh module instance).
    observed.length = 0
    const reloadedTab = await import('./api.ts?case=reload-fresh')
    let reloadError = null
    try {
      await reloadedTab.listSessions()
    } catch (cause) {
      reloadError = cause
    }

    const requests = observed.slice()
    const unauthenticatedApiHits = requests.filter(
      (request) => request.url.startsWith('/api/') && request.authorization === undefined,
    )
    const relaunchHits = requests.filter(
      (request) =>
        !request.url.startsWith('/api/sessions') &&
        /relaunch|re-?auth|credential|refresh/i.test(request.url),
    )
    const authenticatedSessionsOk = requests.some(
      (request) =>
        request.url === '/api/sessions' &&
        request.authorization === `Bearer ${TOKEN}` &&
        request.status === 200,
    )

    if (relaunchHits.length > 0 && authenticatedSessionsOk) {
      // Supported lifecycle: the client regained a usable credential through an
      // explicit secure relaunch contract and then authenticated normally.
      assert.ok(true)
    } else {
      assert.equal(
        unauthenticatedApiHits.length,
        0,
        'a full reload without a fragment must not silently send unauthenticated API requests',
      )
      assert.ok(reloadError, 'a full reload without a fragment must fail with a clear re-auth state')
      assert.match(
        `${reloadError.name} ${reloadError.message}`,
        /re-?auth|unauthor|credential|session expired|sign-?in|forbidden/i,
        'the failure must clearly indicate that re-authentication is required',
      )
    }
  } finally {
    browser.restore()
    server.close()
    await once(server, 'close')
  }
})

test('malformed fragment is ignored: no Authorization is sent and the live endpoint returns a real 401', async () => {
  const { server, observed } = startAuthServer()
  const port = await listen(server)
  const malformed = 'b'.repeat(32)
  const browser = installBrowser({ hash: `#oc2-token=${malformed}`, port })
  try {
    const api = await import('./api.ts?case=malformed')
    let failure = null
    await assert.rejects(
      () => api.listSessions(),
      (error) => {
        failure = error
        assert.equal(error.status, 401)
        return true
      },
    )

    assert.equal(observed.length, 1)
    assert.equal(observed[0].url, '/api/sessions')
    assert.equal(observed[0].authorization, undefined, 'a malformed fragment must not become a bearer credential')
    assert.equal(observed[0].url.includes(malformed), false)
    assert.equal(browser.rawCalls[0].authorization, null)
    assert.equal(String(failure.message).includes(malformed), false)
    assertNoTokenLeak(browser, malformed)
  } finally {
    browser.restore()
    server.close()
    await once(server, 'close')
  }
})

test('bearer credential is never attached to non-/api or remote paths', async () => {
  const { server, observed } = startAuthServer()
  const port = await listen(server)
  const browser = installBrowser({ hash: `#oc2-token=${TOKEN}`, port })
  try {
    const api = await import('./api.ts?case=non-api')
    await api.listSessions()
    const health = await api.getHealth()
    assert.equal(health.status, 'ok')

    const healthRequest = observed.find((request) => request.url === '/health')
    assert.ok(healthRequest, 'the health request must reach the local server')
    assert.equal(
      healthRequest.authorization,
      undefined,
      'a non-/api path must never receive the bearer credential',
    )

    const healthRaw = browser.rawCalls.find((call) => call.raw === '/health')
    assert.ok(healthRaw, 'the health fetch must use a relative non-/api path')
    assert.equal(healthRaw.authorization, null)

    const sessionsRequest = observed.find((request) => request.url === '/api/sessions')
    assert.ok(sessionsRequest, 'the /api path must still receive the credential in this tab')
    assert.equal(sessionsRequest.authorization, `Bearer ${TOKEN}`)

    assert.ok(
      browser.rawCalls.every((call) => call.raw.startsWith('/')),
      'the client must never fetch an absolute/remote URL',
    )
  } finally {
    browser.restore()
    server.close()
    await once(server, 'close')
  }
})