import { useEffect, useState } from 'react'

export type WebTheme = 'system' | 'light' | 'dark' | 'midnight' | 'ocean' | 'forest'
export type WebDensity = 'comfortable' | 'compact'
export type WebTextSize = 'small' | 'normal' | 'large'

export interface WebPreferences {
  theme: WebTheme
  density: WebDensity
  textSize: WebTextSize
  reducedMotion: boolean
}

export const DEFAULT_WEB_PREFERENCES: WebPreferences = {
  theme: 'dark',
  density: 'comfortable',
  textSize: 'normal',
  reducedMotion: false,
}

const STORAGE_KEY = 'opencode-rk:web-preferences:v1'
const themes: WebTheme[] = ['system', 'light', 'dark', 'midnight', 'ocean', 'forest']

function storedPreferences(): WebPreferences {
  try {
    const stored = JSON.parse(localStorage.getItem(STORAGE_KEY) ?? '{}') as Record<string, unknown>
    return {
      theme: themes.includes(stored.theme as WebTheme) ? stored.theme as WebTheme : 'dark',
      density: stored.density === 'compact' ? 'compact' : 'comfortable',
      textSize: stored.textSize === 'small' || stored.textSize === 'large' ? stored.textSize : 'normal',
      reducedMotion: stored.reducedMotion === true,
    }
  } catch {
    return DEFAULT_WEB_PREFERENCES
  }
}

export function useWebPreferences() {
  const [preferences, setPreferences] = useState<WebPreferences>(storedPreferences)

  useEffect(() => {
    const root = document.documentElement
    const systemDark = window.matchMedia?.('(prefers-color-scheme: dark)')
    const apply = () => {
      const effective = preferences.theme === 'system' ? (systemDark?.matches ? 'dark' : 'light') : preferences.theme
      root.classList.toggle('dark', effective !== 'light')
      root.dataset.theme = effective
      root.dataset.density = preferences.density
      root.dataset.textSize = preferences.textSize
      root.dataset.reduceMotion = String(preferences.reducedMotion)
    }
    apply()
    systemDark?.addEventListener?.('change', apply)
    try { localStorage.setItem(STORAGE_KEY, JSON.stringify(preferences)) } catch { /* preference persistence is optional */ }
    return () => {
      systemDark?.removeEventListener?.('change', apply)
    }
  }, [preferences])

  return {
    preferences,
    setPreferences: (patch: Partial<WebPreferences>) =>
      setPreferences((current) => ({ ...current, ...patch })),
  }
}
