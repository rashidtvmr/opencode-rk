import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import test from 'node:test'

const NATIVE = 'a'.repeat(64)
const CAPABILITY = 'B'.repeat(43)
const ROTATED = 'C'.repeat(43)
const LIMIT_MS = 15 * 60 * 1000
const nativeFetch = globalThis.fetch

function fixture() {
  const state = { origin: '', leases: new Map(), generation: 1, sessions: [{ id: 's1', title: 'preserved selection/history' }], calls: [] }
  const server = createServer((req, res) => {
    const url = req.url ?? ''
    const origin = state.origin
    const auth = req.headers.authorization
    const requestOrigin = req.headers.origin
    state.calls.push({ url, auth, origin: requestOrigin, method: req.method })
    const send = (status, body) => { res.writeHead(status, { 'content-type': 'application/json', 'access-control-allow-origin': origin, 'access-control-allow-credentials': 'true' }).end(JSON.stringify(body)) }
    // Browsers normally omit Origin on same-origin GETs. Mutating and
    // capability-control requests still require the explicit bound origin.
    const originOK = requestOrigin === origin
    const readOriginOK = requestOrigin === undefined || originOK
    const token = auth?.startsWith('BrowserResume ') ? auth.slice('BrowserResume '.length) : undefined
    if (url === '/auth/browser-resume' && req.method === 'POST') {
      if (!originOK) return send(403, { message: 'origin denied' })
      if (auth !== `Bearer ${NATIVE}`) return send(auth ? 403 : 401, { message: 'trusted launch required' })
      state.leases.set(CAPABILITY, { expires: Date.now() + LIMIT_MS, generation: state.generation })
      return send(201, { browser_capability: CAPABILITY, expires_at_unix_ms: Date.now() + LIMIT_MS })
    }
    if (url === '/auth/browser-resume/exchange' && req.method === 'POST') {
      if (!originOK) return send(403, { message: 'origin denied' })
      const lease = state.leases.get(token)
      if (!lease || lease.expires <= Date.now() || lease.generation !== state.generation) return send(403, { message: 'resume denied' })
      state.leases.delete(token)
      state.leases.set(ROTATED, lease)
      return send(200, { browser_capability: ROTATED, expires_at_unix_ms: lease.expires })
    }
    if (url === '/auth/browser-resume' && req.method === 'DELETE') {
      const lease = state.leases.get(token)
      if (!originOK || !lease || lease.expires <= Date.now() || lease.generation !== state.generation) return send(403, { message: 'resume denied' })
      state.leases.delete(token)
      res.writeHead(204, { 'access-control-allow-origin': origin }).end()
      return
    }
    if (url === '/api/sessions') {
      const lease = state.leases.get(token)
      // The legacy daemon currently accepts native auth on this endpoint. Keep
      // that baseline behavior in the fixture so the contract test can expose
      // the client leak instead of failing because the fixture rejects it.
      const authorized = readOriginOK && (auth === `Bearer ${NATIVE}` || (!!lease && lease.expires > Date.now() && lease.generation === state.generation))
      return authorized ? send(200, { sessions: state.sessions }) : send(auth ? 403 : 401, { message: 'authentication required' })
    }
    return send(404, { message: 'not found' })
  })
  return { server, state }
}

async function listen(server, state) {
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')
  state.origin = `http://127.0.0.1:${address.port}`
  return address.port
}

function capabilityFrom(value) {
  if (typeof value !== 'string') return undefined
  try {
    const parsed = JSON.parse(value)
    if (parsed && typeof parsed === 'object') return parsed.browser_capability ?? parsed.capability ?? parsed.token
  } catch {}
  return value
}

const GLOBAL_KEYS = ['fetch', 'window', 'location', 'history', 'localStorage', 'sessionStorage', 'console']
function saveGlobals() {
  return Object.fromEntries(GLOBAL_KEYS.map((key) => [key, globalThis[key]]))
}
function restoreGlobals(snapshot) {
  for (const key of GLOBAL_KEYS) {
    if (snapshot[key] === undefined) delete globalThis[key]
    else globalThis[key] = snapshot[key]
  }
}

function browser(port, initialHash = '', tabSessionState = new Map()) {
  const old = { fetch: globalThis.fetch, console: globalThis.console }
  const location = { href: `http://127.0.0.1:${port}/${initialHash}`, hash: initialHash }
  const writes = []
  const storage = (name) => {
    const entries = name === 'sessionStorage' ? tabSessionState : new Map()
    return { entries, getItem: (key) => entries.get(key) ?? null, setItem: (key, value) => { writes.push({ name, key, value: String(value) }); entries.set(key, String(value)) }, removeItem: (key) => entries.delete(key), clear: () => entries.clear(), get length() { return entries.size }, key: (index) => [...entries.keys()][index] ?? null }
  }
  const sessionStorage = storage('sessionStorage')
  const localStorage = storage('localStorage')
  const historyWrites = []
  const history = { state: null, replaceState(_state, _title, value) { historyWrites.push(String(value)); const parsed = new URL(value, location.href); location.href = parsed.href; location.hash = parsed.hash } }
  const logs = []
  const loggedConsole = Object.fromEntries(['debug', 'info', 'log', 'warn', 'error'].map((method) => [method, (...args) => { logs.push(args.map(String).join(' ')); old.console?.[method]?.(...args) }]))
  globalThis.location = location
  globalThis.history = history
  globalThis.window = { location, history, sessionStorage, localStorage }
  globalThis.sessionStorage = sessionStorage
  globalThis.localStorage = localStorage
  globalThis.console = { ...old.console, ...loggedConsole }
  globalThis.fetch = (input, init) => {
    const headers = new Headers(init?.headers)
    // Do not invent Origin on GET: real browsers do not send it for same-origin
    // fetches. They do send it for these CORS-capable state-changing requests.
    const method = (init?.method ?? 'GET').toUpperCase()
    if (!headers.has('origin') && method !== 'GET' && method !== 'HEAD') headers.set('origin', new URL(location.href).origin)
    return old.fetch(new URL(String(input), `http://127.0.0.1:${port}/`), { ...init, headers })
  }
  return { location, sessionStorage, localStorage, writes, historyWrites, logs }
}

async function imported(caseId) { return import(`./api.ts?browser-resume=${caseId}`) }
async function rawFetch(port, path, { method = 'GET', authorization, origin = `http://127.0.0.1:${port}` } = {}) {
  const headers = new Headers({ origin })
  if (authorization) headers.set('authorization', authorization)
  return nativeFetch(`http://127.0.0.1:${port}${path}`, { method, headers })
}
async function close(server) {
  if (!server.listening) return
  server.close()
  await once(server, 'close')
}

// This is intentionally an external-HTTP client contract fixture: the server
// below models the authorized wire protocol but cannot implement browser API
// behavior on behalf of api.ts.
test('authenticated trusted launch issues separate bounded tab capability; fresh module reload resumes before sessions', async (t) => {
  const globals = saveGlobals()
  t.after(() => restoreGlobals(globals))
  const { server, state } = fixture()
  t.after(() => close(server))
  const port = await listen(server, state)
  const firstTab = browser(port, `#oc2-token=${NATIVE}`)
  let reload
  const api = await imported('launch')
  const sessions = await api.listSessions()
  assert.equal(sessions[0]?.id, 's1')
  assert.equal(firstTab.location.hash, '')
  const stored = firstTab.sessionStorage.getItem('opencode.browser-resume')
  const sessionCapability = capabilityFrom(stored)
  assert.equal(firstTab.localStorage.length, 0, 'capability is not origin-persistent across tabs')
  assert.ok(firstTab.writes.every(({ value }) => !value.includes(NATIVE)), 'native bearer never enters browser storage')
  assert.ok(firstTab.historyWrites.every((value) => !value.includes(NATIVE) && !value.includes(sessionCapability ?? 'invalid')), 'credentials never enter browser history')
  const issue = state.calls.find((call) => call.url === '/auth/browser-resume')
  assert.equal(issue?.auth, `Bearer ${NATIVE}`, 'native bearer used only to mint separate lease')
  assert.ok(!state.calls.some((call) => call.url.includes(NATIVE)))
  assert.ok(sessionCapability, 'distinct browser capability stored for tab reload')
  assert.notEqual(sessionCapability, NATIVE)
  assert.equal(firstTab.writes.length, 1)
  assert.equal(firstTab.writes[0].name, 'sessionStorage')
  assert.equal(capabilityFrom(firstTab.writes[0].value), sessionCapability)
  assert.ok(firstTab.writes.every(({ value }) => !value.includes(NATIVE)), 'native bearer never enters browser storage')
  assert.ok(firstTab.historyWrites.every((value) => !value.includes(NATIVE) && !value.includes(sessionCapability)), 'credentials never enter browser history')

  reload = browser(port, '', firstTab.sessionStorage.entries)
  const reloadedApi = await imported('reload')
  const resumedSessions = await reloadedApi.listSessions()
  assert.equal(resumedSessions[0]?.id, 's1', 'selection/session history can be restored independently of bearer')
  assert.equal(state.calls.at(-2)?.url, '/auth/browser-resume/exchange')
  assert.equal(state.calls.at(-1)?.url, '/api/sessions', 'exchange precedes session request')
  assert.equal(state.calls.at(-1)?.auth, `BrowserResume ${ROTATED}`)
  assert.equal(capabilityFrom(reload.sessionStorage.getItem('opencode.browser-resume')), ROTATED, 'rotated tab capability retained after reload')
  assert.equal(reload.location.href.includes(NATIVE), false)
  assert.equal(reload.location.href.includes(sessionCapability), false)
  assert.ok(state.calls.every((call) => !call.url.includes(NATIVE) && !call.url.includes(sessionCapability)))
  assert.ok([...firstTab.logs, ...reload.logs].every((line) => !line.includes(NATIVE) && !line.includes(sessionCapability) && !line.includes(ROTATED)), 'credentials are not written to console')
})

test('a fresh document exchanges the tab capability before issuing its first session request', async (t) => {
  const globals = saveGlobals()
  t.after(() => restoreGlobals(globals))
  const { server, state } = fixture()
  state.leases.set(CAPABILITY, { expires: Date.now() + LIMIT_MS, generation: state.generation })
  t.after(() => close(server))
  const port = await listen(server, state)
  const page = browser(port)
  page.sessionStorage.setItem('opencode.browser-resume', CAPABILITY)
  const api = await imported('reload-contract')
  const sessions = await api.listSessions()
  assert.equal(sessions[0]?.id, 's1')
  assert.equal(state.calls[0]?.url, '/auth/browser-resume/exchange')
  assert.equal(state.calls[0]?.auth, `BrowserResume ${CAPABILITY}`)
  assert.equal(state.calls[1]?.url, '/api/sessions')
  assert.equal(state.calls[1]?.auth, `BrowserResume ${ROTATED}`)
  assert.equal(capabilityFrom(page.sessionStorage.getItem('opencode.browser-resume')), ROTATED)
  assert.ok(state.calls.every((call) => !call.url.includes(NATIVE) && !call.url.includes(CAPABILITY) && !call.url.includes(ROTATED)))
})

test('missing, malformed, expired, revoked, wrong-origin, and restarted leases fail before session API effects', async (t) => {
  const globals = saveGlobals()
  t.after(() => restoreGlobals(globals))
  const { server, state } = fixture()
  t.after(() => close(server))
  const port = await listen(server, state)
  const page = browser(port)
  let wrongOrigin
  const api = await imported('missing')
  await assert.rejects(api.listSessions(), /re-?auth|credential|unauthor/i)
  assert.equal(state.calls.length, 0, 'missing capability makes no API request')
  for (const bad of ['bad', 'not-a-capability']) {
    page.sessionStorage.setItem('opencode.browser-resume', bad)
    const before = state.calls.length
    const badApi = await imported(`bad-${bad.length}-${bad[0]}`)
    await assert.rejects(badApi.listSessions())
    assert.ok(!state.calls.slice(before).some((call) => call.url === '/api/sessions'))
  }

  page.sessionStorage.setItem('opencode.browser-resume', CAPABILITY)
  state.leases.set(CAPABILITY, { expires: Date.now() - 1, generation: state.generation })
  const expiredApi = await imported('expired')
  const beforeExpired = state.calls.length
  await assert.rejects(expiredApi.listSessions())
  assert.ok(!state.calls.slice(beforeExpired).some((call) => call.url === '/api/sessions'))

  state.leases.set(CAPABILITY, { expires: Date.now() + LIMIT_MS, generation: state.generation })
  state.leases.delete(CAPABILITY)
  const revokedApi = await imported('revoked')
  const beforeRevoked = state.calls.length
  await assert.rejects(revokedApi.listSessions())
  assert.ok(!state.calls.slice(beforeRevoked).some((call) => call.url === '/api/sessions'))

  state.leases.set(CAPABILITY, { expires: Date.now() + LIMIT_MS, generation: state.generation })
  state.generation++
  const restartedApi = await imported('restart')
  const beforeRestart = state.calls.length
  await assert.rejects(restartedApi.listSessions())
  assert.ok(!state.calls.slice(beforeRestart).some((call) => call.url === '/api/sessions'))

  state.generation--
  wrongOrigin = browser(port)
  wrongOrigin.location.href = 'http://attacker.invalid/'
  wrongOrigin.sessionStorage.setItem('opencode.browser-resume', CAPABILITY)
  state.leases.set(CAPABILITY, { expires: Date.now() + LIMIT_MS, generation: state.generation })
  const wrongOriginApi = await imported('wrong-origin')
  const beforeOrigin = state.calls.length
  await assert.rejects(wrongOriginApi.listSessions())
  assert.ok(!state.calls.slice(beforeOrigin).some((call) => call.url === '/api/sessions'))

  const wrongOriginServerResponse = await rawFetch(port, '/auth/browser-resume/exchange', {
    method: 'POST', authorization: `BrowserResume ${CAPABILITY}`, origin: 'http://attacker.invalid',
  })
  assert.equal(wrongOriginServerResponse.status, 403, 'server fixture binds exchange to its original origin')
  state.leases.set(CAPABILITY, { expires: Date.now() - 1, generation: state.generation })
  const expiredServerResponse = await rawFetch(port, '/api/sessions', { authorization: `BrowserResume ${CAPABILITY}` })
  assert.equal(expiredServerResponse.status, 403, 'server fixture rejects expired or unknown leases')
  state.leases.set(CAPABILITY, { expires: Date.now() + LIMIT_MS, generation: state.generation - 1 })
  const restartedServerResponse = await rawFetch(port, '/api/sessions', { authorization: `BrowserResume ${CAPABILITY}` })
  assert.equal(restartedServerResponse.status, 403, 'server fixture rejects prior-generation leases')
})
