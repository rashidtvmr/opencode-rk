import assert from 'node:assert/strict'
import { createServer } from 'node:http'
import { once } from 'node:events'
import test from 'node:test'

import { runTurnStream } from './api.ts'

// B1 RED: the native daemon emits tool_call/tool_output NDJSON events on
// POST /api/sessions/:id/turns/stream (crates/server/src/lib.rs, ndjson
// json!({"type": "tool_call", ...}) and json!({"type": "tool_output", ...})),
// but web/src/lib/api.ts parseTurnStreamEvent has no such cases and throws
// 'Server returned an unknown turn stream event'. These tests pin the
// observable accepted-tool-stream contract; they fail cleanly until the
// client learns the server's real tool events. Component RED only: the
// fake opener is not browser proof, real-browser acceptance still pending.

const token = 'c'.repeat(64)

function userMessage(sessionId) {
  return {
    id: 'msg_user_1',
    session_id: sessionId,
    role: 'user',
    body: { storage: 'inline', text: 'run echo g6-tool-ok' },
  }
}

function assistantMessage(sessionId, text) {
  return {
    id: 'msg_asst_1',
    session_id: sessionId,
    role: 'assistant',
    body: { storage: 'inline', text },
  }
}

// Full tool round in server wire order: user, tool_call, tool_output,
// assistant deltas, final assistant_message whose text matches the deltas.
function toolRoundLines(sessionId) {
  return [
    JSON.stringify({ type: 'user_message', message: userMessage(sessionId) }),
    JSON.stringify({
      type: 'tool_call',
      call_id: 'call_fixture_1',
      name: 'bash',
      arguments: '{"command":"echo g6-tool-ok"}',
    }),
    JSON.stringify({
      type: 'tool_output',
      call_id: 'call_fixture_1',
      name: 'bash',
      output: 'g6-tool-ok',
    }),
    JSON.stringify({ type: 'assistant_delta', delta: 'Loop ' }),
    JSON.stringify({ type: 'assistant_delta', delta: 'done' }),
    JSON.stringify({
      type: 'assistant_message',
      message: assistantMessage(sessionId, 'Loop done'),
      reasoning_summary: null,
      stop_reason: 'completed',
    }),
  ]
}

function startHarness(respond) {
  const observed = []
  const server = createServer((request, response) => {
    let body = ''
    request.on('data', (chunk) => {
      body += chunk
    })
    request.on('end', () => {
      observed.push({ authorization: request.headers.authorization, url: request.url, body })
      respond(request, response, body)
    })
  })
  return { server, observed }
}

async function withBrowser(port, fn) {
  const originalFetch = globalThis.fetch
  const originalWindow = globalThis.window
  const originalLocation = globalThis.location
  const originalHistory = globalThis.history
  const location = {
    href: `http://127.0.0.1:${port}/#oc2-token=${token}`,
    hash: `#oc2-token=${token}`,
  }
  globalThis.location = location
  globalThis.history = {
    replaceState(_state, _title, url) {
      if (typeof url === 'string') {
        const parsed = new URL(url, location.href)
        location.href = parsed.href
        location.hash = parsed.hash
      }
    },
  }
  globalThis.window = { location, history: globalThis.history }
  globalThis.fetch = (input, init) => {
    const target = new URL(String(input), `http://127.0.0.1:${port}/`)
    return originalFetch(target, init)
  }
  try {
    await fn()
  } finally {
    globalThis.fetch = originalFetch
    if (originalWindow === undefined) delete globalThis.window
    else globalThis.window = originalWindow
    if (originalLocation === undefined) delete globalThis.location
    else globalThis.location = originalLocation
    if (originalHistory === undefined) delete globalThis.history
    else globalThis.history = originalHistory
  }
}

// Fragmented writes: split every line into small mid-line chunks so the
// test proves NDJSON reassembly, not just whole-line parsing.
function sendFragmented(response, lines) {
  response.writeHead(201, { 'content-type': 'application/x-ndjson', 'cache-control': 'no-store' })
  const payload = lines.join('\n') + '\n'
  for (let i = 0; i < payload.length; i += 13) {
    response.write(payload.slice(i, i + 13))
  }
  response.end()
}

test('tool round streams to completion: user -> tool_call -> tool_output -> deltas -> assistant_message', async () => {
  const sessionId = 'ses_g6_tool_1'
  const { server, observed } = startHarness((request, response) => {
    if (request.headers.authorization !== `Bearer ${token}`) {
      response.writeHead(401, { 'content-type': 'application/json' }).end('{}')
      return
    }
    sendFragmented(response, toolRoundLines(sessionId))
  })
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')

  try {
    await withBrowser(address.port, async () => {
      const seen = []
      const turn = await runTurnStream(sessionId, 'run echo g6-tool-ok', 'openai/gpt-5.6', 'high', {
        onUserMessage: () => seen.push('user_message'),
        onAssistantDelta: () => seen.push('assistant_delta'),
        onAssistantMessage: () => seen.push('assistant_message'),
      })
      // Observable accepted tool stream: executed, final assistant text from
      // the post-tool round, reached completion after the tool round.
      assert.equal(turn.executed, true)
      assert.ok(turn.userMessage)
      assert.equal(turn.userMessage.role, 'user')
      assert.ok(turn.assistantMessage)
      assert.equal(turn.assistantMessage.role, 'assistant')
      assert.equal(turn.assistantMessage.body.text, 'Loop done')
      assert.deepEqual(seen, [
        'user_message',
        'assistant_delta',
        'assistant_delta',
        'assistant_message',
      ])
      assert.equal(observed.length, 1)
      assert.ok(observed[0].url.includes(`/api/sessions/${sessionId}/turns/stream`))
    })
  } finally {
    server.close()
    await once(server, 'close')
  }
})

test('two successive tool turns parse identically (second-turn parser stability)', async () => {
  const sessionId = 'ses_g6_tool_2'
  const { server } = startHarness((request, response) => {
    sendFragmented(response, toolRoundLines(sessionId))
  })
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')

  try {
    await withBrowser(address.port, async () => {
      for (let round = 0; round < 2; round++) {
        const turn = await runTurnStream(sessionId, `second turn probe ${round}`, 'openai/gpt-5.6', 'high')
        assert.equal(turn.executed, true)
        assert.equal(turn.assistantMessage?.body?.storage === 'inline' && turn.assistantMessage.body.text, 'Loop done')
      }
    })
  } finally {
    server.close()
    await once(server, 'close')
  }
})

test('malformed tool events are rejected, never silently accepted', async () => {
  const sessionId = 'ses_g6_tool_3'
  const cases = [
    // tool_call without a string call_id
    JSON.stringify({ type: 'tool_call', name: 'bash', arguments: '{}' }),
    // tool_call with non-string name
    JSON.stringify({ type: 'tool_call', call_id: 'call_1', name: 42, arguments: '{}' }),
    // tool_output without output text
    JSON.stringify({ type: 'tool_output', call_id: 'call_1', name: 'bash' }),
  ]
  for (const badLine of cases) {
    const { server } = startHarness((request, response) => {
      sendFragmented(response, [
        JSON.stringify({ type: 'user_message', message: userMessage(sessionId) }),
        badLine,
      ])
    })
    server.listen(0, '127.0.0.1')
    await once(server, 'listening')
    const address = server.address()
    assert.ok(address && typeof address === 'object')
    try {
      await withBrowser(address.port, async () => {
        await assert.rejects(
          runTurnStream(sessionId, 'run echo g6-tool-ok', 'openai/gpt-5.6', 'high'),
          (error) => error instanceof Error && /tool|invalid|unknown/i.test(error.message),
          `malformed line must be rejected: ${badLine}`,
        )
      })
    } finally {
      server.close()
      await once(server, 'close')
    }
  }
})

test('late server error after tool events propagates the server message', async () => {
  const sessionId = 'ses_g6_tool_4'
  const { server } = startHarness((request, response) => {
    sendFragmented(response, [
      JSON.stringify({ type: 'user_message', message: userMessage(sessionId) }),
      JSON.stringify({
        type: 'tool_call',
        call_id: 'call_fixture_1',
        name: 'bash',
        arguments: '{"command":"echo g6-tool-ok"}',
      }),
      JSON.stringify({ type: 'error', code: 'bad_gateway', message: 'provider stream failed' }),
    ])
  })
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')

  try {
    await withBrowser(address.port, async () => {
      await assert.rejects(
        runTurnStream(sessionId, 'run echo g6-tool-ok', 'openai/gpt-5.6', 'high'),
        /provider stream failed/,
      )
    })
  } finally {
    server.close()
    await once(server, 'close')
  }
})

test('turn stream byte bound retained: oversized line rejected', async () => {
  const sessionId = 'ses_g6_tool_5'
  const { server } = startHarness((request, response) => {
    // One line past MAX_TURN_STREAM_LINE_BYTES (256 KiB).
    const big = JSON.stringify({
      type: 'tool_output',
      call_id: 'call_fixture_1',
      name: 'bash',
      output: 'x'.repeat(300 * 1024),
    })
    sendFragmented(response, [
      JSON.stringify({ type: 'user_message', message: userMessage(sessionId) }),
      big,
    ])
  })
  server.listen(0, '127.0.0.1')
  await once(server, 'listening')
  const address = server.address()
  assert.ok(address && typeof address === 'object')

  try {
    await withBrowser(address.port, async () => {
      await assert.rejects(
        runTurnStream(sessionId, 'run echo g6-tool-ok', 'openai/gpt-5.6', 'high'),
        /safety limit/,
      )
    })
  } finally {
    server.close()
    await once(server, 'close')
  }
})
