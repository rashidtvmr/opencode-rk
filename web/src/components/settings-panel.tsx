import { useEffect, useRef, useState } from 'react'
import { MonitorCog, Palette, Settings2, SlidersHorizontal, X } from 'lucide-react'
import type { HealthResponse, ModelSummary, WebCapabilities } from '@/lib/api'
import type { WebPreferences, WebTheme, WebDensity, WebTextSize } from '@/lib/web-preferences'

type Tab = 'general' | 'appearance' | 'models' | 'capabilities'
const themeChoices: { value: WebTheme; label: string; summary: string }[] = [
  { value: 'system', label: 'System', summary: 'Match your device' },
  { value: 'light', label: 'Light', summary: 'Bright, high-contrast' },
  { value: 'dark', label: 'Dark', summary: 'Chat-style dark' },
  { value: 'midnight', label: 'Midnight', summary: 'Deep black' },
  { value: 'ocean', label: 'Ocean', summary: 'Blue graphite' },
  { value: 'forest', label: 'Forest', summary: 'Muted green' },
  { value: 'contrast', label: 'High contrast', summary: 'Maximum text and border contrast' },
]

export function SettingsPanel({ onClose, preferences, setPreferences, models, capabilities, health }: {
  onClose: () => void
  preferences: WebPreferences
  setPreferences: (patch: Partial<WebPreferences>) => void
  models: ModelSummary[]
  capabilities: WebCapabilities | null
  health: HealthResponse | null
}) {
  const [tab, setTab] = useState<Tab>('appearance')
  const closeButton = useRef<HTMLButtonElement>(null)
  const dialogRef = useRef<HTMLElement>(null)
  useEffect(() => {
    const previousFocus = document.activeElement instanceof HTMLElement ? document.activeElement : null
    closeButton.current?.focus()
    const onKeydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); onClose(); return }
      if (event.key !== 'Tab' || !dialogRef.current) return
      const controls = [...dialogRef.current.querySelectorAll<HTMLElement>(
        'button:not([disabled]), input:not([disabled]), select:not([disabled]), textarea:not([disabled]), a[href]',
      )].filter((element) => element.getClientRects().length > 0 || element === closeButton.current)
      if (!controls.length) return
      const first = controls[0]
      const last = controls[controls.length - 1]
      const active = document.activeElement
      if (event.shiftKey && active === first) { event.preventDefault(); last.focus() }
      else if (!event.shiftKey && active === last) { event.preventDefault(); first.focus() }
      else if (!dialogRef.current.contains(active)) { event.preventDefault(); first.focus() }
    }
    document.addEventListener('keydown', onKeydown)
    return () => {
      document.removeEventListener('keydown', onKeydown)
      previousFocus?.focus()
    }
  }, [onClose])

  return (
    <div className="codex-settings-overlay" onMouseDown={(event) => { if (event.target === event.currentTarget) onClose() }}>
      <section ref={dialogRef} role="dialog" aria-modal="true" aria-label="Settings" className="codex-settings-dialog">
        <div className="codex-panel-title">
          <div><h2>Settings</h2><p>Personalize OpenCode RK on this device</p></div>
          <button ref={closeButton} className="codex-panel-icon" aria-label="Close settings" onClick={onClose}><X size={18} /></button>
        </div>
        <div className="codex-settings-body">
          <nav className="codex-settings-tabs" aria-label="Settings sections">
            {([
              ['general', Settings2, 'General'],
              ['appearance', Palette, 'Appearance'],
              ['models', SlidersHorizontal, 'Models'],
              ['capabilities', MonitorCog, 'Capabilities'],
            ] as const).map(([key, Icon, label]) => (
              <button key={key} className={tab === key ? 'selected' : ''} aria-current={tab === key ? 'page' : undefined} onClick={() => setTab(key)}>
                <Icon size={17} />{label}
              </button>
            ))}
          </nav>
          <div className="codex-settings-content">
            {tab === 'appearance' ? (
              <>
                <h3>Appearance</h3>
                <p>Theme and reading preferences are saved locally in your browser.</p>
                <fieldset className="codex-settings-theme-grid">
                  <legend>Theme</legend>
                  {themeChoices.map((choice) => (
                    <label className={preferences.theme === choice.value ? 'selected' : ''} key={choice.value}>
                      <input type="radio" name="web-theme" value={choice.value} checked={preferences.theme === choice.value}
                        onChange={() => setPreferences({ theme: choice.value })} />
                      <span className={`codex-theme-preview codex-theme-preview-${choice.value}`} aria-hidden="true" />
                      <strong>{choice.label}</strong><small>{choice.summary}</small>
                    </label>
                  ))}
                </fieldset>
                <div className="codex-settings-field">
                  <label htmlFor="settings-density">Display density</label>
                  <select id="settings-density" value={preferences.density} onChange={(event) => setPreferences({ density: event.target.value as WebDensity })}>
                    <option value="comfortable">Comfortable</option><option value="compact">Compact</option>
                  </select>
                </div>
                <div className="codex-settings-field">
                  <label htmlFor="settings-text">Message text size</label>
                  <select id="settings-text" value={preferences.textSize} onChange={(event) => setPreferences({ textSize: event.target.value as WebTextSize })}>
                    <option value="small">Small</option><option value="normal">Normal</option><option value="large">Large</option>
                  </select>
                </div>
                <label className="codex-settings-checkbox">
                  <input type="checkbox" checked={preferences.reducedMotion} onChange={(event) => setPreferences({ reducedMotion: event.target.checked })} />
                  Reduce animations
                </label>
                <div className="codex-settings-field">
                  <label htmlFor="settings-sidebar-width">Desktop sidebar width: {preferences.sidebarWidth}px</label>
                  <input id="settings-sidebar-width" type="range" min="220" max="420" step="10"
                    value={preferences.sidebarWidth} onChange={(event) => setPreferences({ sidebarWidth: Number(event.target.value) })} />
                </div>
                <label className="codex-settings-checkbox">
                  <input type="checkbox" checked={preferences.expandToolCards}
                    onChange={(event) => setPreferences({ expandToolCards: event.target.checked })} />
                  Expand tool execution cards by default
                </label>
              </>
            ) : null}
            {tab === 'general' ? (
              <>
                <h3>General</h3>
                <p>Current session: {health?.status === 'ok' ? 'Connected to the local Rust daemon' : 'Daemon unavailable'}</p>
                <p>Enter sends a message, Shift+Enter adds a line, and the composer accepts configured <code>/commands</code> and <code>@agents</code>.</p>
                <p>All commands, tools and agent permissions execute through the native backend; the browser does not bypass approvals.</p>
              </>
            ) : null}
            {tab === 'models' ? (
              <>
                <h3>Available models ({models.length})</h3>
                <p>Models are discovered from the daemon. Select the model in the conversation composer.</p>
                <ul className="codex-settings-model-list">{models.map((model, i) => (
                  <li key={`${model.provider ?? model.provider_id ?? ''}-${model.id ?? model.model_id ?? i}`}>
                    <strong>{model.name ?? model.model_id ?? model.id ?? 'Unnamed model'}</strong>
                    <span>{model.provider ?? model.provider_id ?? 'Unknown provider'}</span>
                  </li>
                ))}</ul>
              </>
            ) : null}
            {tab === 'capabilities' ? (
              <>
                <h3>Native capabilities</h3>
                <p>Reported directly by the authenticated Rust daemon. Unavailable capabilities remain disabled.</p>
                <ul className="codex-settings-model-list">{capabilities?.tools.map((tool) => (
                  <li key={tool.id}><strong>{tool.name}</strong><span>{tool.available_for_web_turn ? 'Available' : tool.reason}</span></li>
                )) ?? <li>No capabilities reported</li>}</ul>
              </>
            ) : null}
          </div>
        </div>
      </section>
    </div>
  )
}
