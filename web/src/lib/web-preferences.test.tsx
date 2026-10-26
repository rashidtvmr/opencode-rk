import { afterEach, expect, it } from 'vitest'
import { cleanup, renderHook, waitFor, act } from '@testing-library/react'
import { clampSidebarWidth, useWebPreferences } from './web-preferences'

afterEach(() => { cleanup(); localStorage.clear() })

it('validates persistent layout preferences and rejects oversized sidebar widths', async () => {
  expect(clampSidebarWidth(-200)).toBe(220)
  expect(clampSidebarWidth(2000)).toBe(420)
  expect(clampSidebarWidth(Number.NaN)).toBe(260)
  const { result } = renderHook(() => useWebPreferences())
  act(() => result.current.setPreferences({ sidebarWidth: 376, theme: 'contrast', expandToolCards: true }))
  await waitFor(() => expect(document.documentElement.dataset.theme).toBe('contrast'))
  expect(document.documentElement.style.getPropertyValue('--codex-sidebar-width')).toBe('376px')
  const stored = JSON.parse(localStorage.getItem('opencode-rk:web-preferences:v1') ?? '{}')
  expect(stored).toMatchObject({ sidebarWidth: 376, theme: 'contrast', expandToolCards: true })
})
