#!/usr/bin/env node
// G6 is a real browser test. No API-created sessions, API-submitted turns, or
// API permission replies: every user action in this driver is a browser action.
// Usage: node tests/e2e/web_browser_journey.mjs --metadata <fixture-metadata.json>

import fs from 'node:fs/promises'
import path from 'node:path'
import process from 'node:process'

const DEADLINE_MS = 30_000
const POLL_MS = 100
const MARKER_CONTENT = 'G6 browser fixture marker\n'
const FIRST_PROMPT = 'write the browser marker'
const SECOND_PROMPT = 'confirm the browser marker'
const RESUMED_PROMPT = 'resume after restart'

function argValue(name) {
  const index = process.argv.indexOf(name)
  return index >= 0 ? process.argv[index + 1] : undefined
}

function fail(stage, message) {
  throw new Error(`G6 ${stage}: ${message}`)
}

async function waitFor(check, stage, timeoutMs = DEADLINE_MS) {
  const deadline = Date.now() + timeoutMs
  while (Date.now() < deadline) {
    const result = await check()
    if (result) return result
    await new Promise((resolve) => setTimeout(resolve, POLL_MS))
  }
  fail(stage, `bounded ${timeoutMs}ms wait expired`)
}

async function readJson(file) {
  return JSON.parse(await fs.readFile(file, 'utf8'))
}

async function assertMarkerAbsent(meta) {
  try {
    await fs.access(meta.marker)
  } catch (error) {
    if (error?.code === 'ENOENT') return
    throw error
  }
  fail('marker-safety', `marker unexpectedly exists: ${meta.marker}`)
}

async function assertMarkerPresent(meta) {
  const bytes = await fs.readFile(meta.marker, 'utf8')
  if (bytes !== MARKER_CONTENT) fail('tool-result', 'marker bytes differ from exact expected content')
}

async function textCount(page, text) {
  return page.getByText(text, { exact: true }).count()
}

async function assertOneEach(page, userText, assistantText, stage) {
  await waitFor(async () => {
    return (await textCount(page, userText)) === 1 && (await textCount(page, assistantText)) === 1
  }, stage)
}

async function sendTurn(page, text, expectedAssistant, stage) {
  await submitTurn(page, text)
  await waitFor(async () => (await textCount(page, expectedAssistant)) === 1, stage)
}

async function submitTurn(page, text) {
  const editor = page.getByRole('textbox', { name: 'Message' })
  await editor.waitFor({ state: 'visible', timeout: DEADLINE_MS })
  await editor.fill(text)
  await page.getByRole('button', { name: 'Send message' }).click()
}

async function main() {
  const metadataFile = argValue('--metadata')
  if (!metadataFile) fail('setup', 'pass --metadata <fixture-metadata.json>')

  let chromium
  try {
    // Resolve only the locally cached package. Never install or fetch anything.
    ;({ chromium } = await import('playwright'))
  } catch (error) {
    fail('prerequisite', `Playwright is not locally available; install nothing in this test lane (${error.message})`)
  }

  const metadataPath = path.resolve(metadataFile)
  const initial = await readJson(metadataPath)
  const root = path.resolve(initial.fixture_root)
  const marker = path.resolve(initial.marker)
  if (path.dirname(marker) !== path.join(root, 'outside')) fail('setup', 'marker is not in this fixture-owned outside sibling')
  let initialUrl
  try {
    initialUrl = new URL(initial.launch_fragment)
  } catch {
    fail('setup', 'fixture launch URL is malformed')
  }
  if (initialUrl.protocol !== 'http:' || initialUrl.hostname !== '127.0.0.1' || !initialUrl.hash.startsWith('#oc2-token=')) {
    fail('setup', 'fixture launch URL lacks the daemon-auth fragment')
  }
  const control = path.resolve(initial.control_dir)
  if (path.dirname(control) !== path.dirname(metadataPath)) fail('setup', 'control directory is not artifact-owned')
  await assertMarkerAbsent(initial)

  let browser
  let context
  try {
    browser = await chromium.launch({ headless: true })
    context = await browser.newContext()
    const page = await context.newPage()
    page.setDefaultTimeout(DEADLINE_MS)
    await page.goto(initial.launch_fragment, { waitUntil: 'domcontentloaded', timeout: DEADLINE_MS })

    // These selectors are read from the actual App/composer source, not guessed:
    // App.tsx exposes New chat / Create; composer.tsx exposes Message / Send message.
    await page.getByRole('button', { name: 'New chat', exact: true }).click()
    await page.getByLabel('Chat title').fill('G6 browser journey')
    await page.getByRole('button', { name: 'Create', exact: true }).click()
    await page.getByRole('heading', { name: 'G6 browser journey' }).waitFor({ state: 'visible' })
    await submitTurn(page, FIRST_PROMPT)

    // Permission MUST come from actual UI backed by daemon broker state. The
    // read-only provider-request artifact is diagnostic only; never reply by API.
    let onceButton
    const permissionDeadline = Date.now() + 10_000
    while (Date.now() < permissionDeadline && !onceButton) {
      const requests = await readJson(initial.request_evidence).catch(() => [])
      const markerExists = await fs.access(marker).then(() => true, () => false)
      if (requests.length > 1 && !markerExists) {
        fail('backend-pending-red', `provider advanced to request ${requests.length} without a user grant; daemon did not park the tool call in a pending approval state`)
      }
      onceButton = await (async () => {
        const button = page.getByRole('button', { name: 'Allow once', exact: true })
        return (await button.count()) === 1 && (await button.isVisible()) ? button : null
      })()
      if (!onceButton) await new Promise((resolve) => setTimeout(resolve, POLL_MS))
    }
    if (!onceButton) {
      const requests = await readJson(initial.request_evidence).catch(() => [])
      const markerExists = await fs.access(marker).then(() => true, () => false)
      if (requests.length === 1 && !markerExists) {
        fail('permission-ui-red', 'exactly one broker-bound provider tool request is waiting with no marker, but App has no real pending-permission UI/Once control')
      }
      fail('backend-pending-red', `no pending permission request/Once button; provider requests=${requests.length}, markerExists=${markerExists}`)
    }

    // This is the only approval action, and it is a human-facing browser click.
    await onceButton.click()
    await waitFor(async () => {
      try {
        await assertMarkerPresent(initial)
        return true
      } catch (error) {
        if (error?.code === 'ENOENT') return false
        throw error
      }
    }, 'tool-result')
    await waitFor(async () => (await textCount(page, 'G6 first turn settled')) === 1, 'first-turn-after-approval')
    await assertOneEach(page, FIRST_PROMPT, 'G6 first turn settled', 'first-history')

    await sendTurn(page, SECOND_PROMPT, 'G6 second turn settled', 'second-turn')
    await assertOneEach(page, FIRST_PROMPT, 'G6 first turn settled', 'pre-reload-first-history')
    await assertOneEach(page, SECOND_PROMPT, 'G6 second turn settled', 'pre-reload-second-history')

    await page.reload({ waitUntil: 'domcontentloaded', timeout: DEADLINE_MS })
    await page.getByRole('heading', { name: 'G6 browser journey' }).waitFor({ state: 'visible' })
    await assertOneEach(page, FIRST_PROMPT, 'G6 first turn settled', 'reload-first-history')
    await assertOneEach(page, SECOND_PROMPT, 'G6 second turn settled', 'reload-second-history')

    // A separately created browser page is a second client on the same daemon.
    const secondClient = await context.newPage()
    secondClient.setDefaultTimeout(DEADLINE_MS)
    await secondClient.goto(initial.launch_fragment, { waitUntil: 'domcontentloaded', timeout: DEADLINE_MS })
    await secondClient.getByRole('heading', { name: 'G6 browser journey' }).waitFor({ state: 'visible' })
    await assertOneEach(secondClient, FIRST_PROMPT, 'G6 first turn settled', 'second-client-first-history')
    await assertOneEach(secondClient, SECOND_PROMPT, 'G6 second turn settled', 'second-client-second-history')

    // The fixture atomically replaces origin+token after it observes this request.
    await fs.writeFile(path.join(control, 'restart.request'), 'restart\n', { flag: 'wx' })
    const updated = await waitFor(async () => {
      const next = await readJson(metadataPath)
      return next.restart_generation === 1 && next.pid !== initial.pid ? next : null
    }, 'daemon-restart')
    await page.goto(updated.launch_fragment, { waitUntil: 'domcontentloaded', timeout: DEADLINE_MS })
    await page.getByRole('heading', { name: 'G6 browser journey' }).waitFor({ state: 'visible' })
    await assertOneEach(page, FIRST_PROMPT, 'G6 first turn settled', 'resumed-first-history')
    await assertOneEach(page, SECOND_PROMPT, 'G6 second turn settled', 'resumed-second-history')
    await sendTurn(page, RESUMED_PROMPT, 'G6 resumed turn settled', 'resumed-turn')

    const requests = await readJson(initial.request_evidence)
    if (requests.length !== 4) fail('provider-contract', `expected exactly four fixture calls, got ${requests.length}`)
    await assertMarkerPresent(initial)
    await fs.writeFile(path.join(control, 'stop.request'), 'stop\n', { flag: 'wx' })
    console.log('G6 browser journey passed its observable browser/provider assertions (integration acceptance remains external).')
  } finally {
    const stopPath = path.join(path.resolve(initial.control_dir), 'stop.request')
    await fs.writeFile(stopPath, 'stop\n', { flag: 'wx' }).catch(() => {})
    if (context) await context.close().catch(() => {})
    if (browser) await browser.close().catch(() => {})
  }
}

main().catch((error) => {
  console.error(error?.stack ?? String(error))
  process.exitCode = 1
})
