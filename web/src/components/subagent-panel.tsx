import { useEffect, useState } from 'react'
import { ArrowUpRight, CirclePause, RefreshCcw, Send, X } from 'lucide-react'
import { listSubagents, updateSubagent, type SubagentSnapshot } from '@/lib/api'

interface SubagentPanelProps {
  sessionId: string
  onClose: () => void
  onOpenSession: (child: SubagentSnapshot) => void
}

const activeStates = new Set(['queued', 'running'])

export function SubagentPanel({ sessionId, onClose, onOpenSession }: SubagentPanelProps) {
  const [children, setChildren] = useState<SubagentSnapshot[]>([])
  const [error, setError] = useState('')
  const [registryScope, setRegistryScope] = useState('')
  const [busyId, setBusyId] = useState('')
  const [followupId, setFollowupId] = useState<string | null>(null)
  const [followupText, setFollowupText] = useState('')

  useEffect(() => {
    const controller = new AbortController()
    let timer: ReturnType<typeof setTimeout> | null = null
    const refresh = async () => {
      try {
        const data = await listSubagents(sessionId, controller.signal)
        if (controller.signal.aborted) return
        setChildren(data.children)
        setRegistryScope(data.registryScope)
        setError('')
      } catch (cause) {
        if (controller.signal.aborted) return
        setError(cause instanceof Error ? cause.message : 'Could not load subagents')
      } finally {
        if (!controller.signal.aborted) timer = setTimeout(() => void refresh(), 2000)
      }
    }
    void refresh()
    return () => {
      controller.abort()
      if (timer) clearTimeout(timer)
    }
  }, [sessionId])

  const act = async (child: SubagentSnapshot, action: 'stop' | 'retry' | 'continue') => {
    if (busyId) return
    const text = followupText.trim()
    if (action === 'continue' && !text) return
    setBusyId(child.session_id)
    setError('')
    try {
      const updated = await updateSubagent(sessionId, child.session_id, action, text)
      setChildren((current) => current.map((item) => item.session_id === updated.session_id ? updated : item))
      if (action === 'continue') { setFollowupId(null); setFollowupText('') }
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Unable to update subagent')
    } finally {
      setBusyId('')
    }
  }

  return (
    <aside className="codex-subagents-panel" aria-label="Subagents">
      <div className="codex-panel-title">
        <div>
          <h2>Subagents</h2>
          <p>Live child tasks · {children.length} total</p>
        </div>
        <button aria-label="Close subagents" className="codex-panel-icon" onClick={onClose}><X size={18} /></button>
      </div>
      {error ? <p className="codex-panel-error" role="alert">{error}</p> : null}
      <div className="codex-subagents-scroll">
        {children.length === 0 && !error ? (
          <p className="codex-panel-empty">No subagents have been delegated in this daemon session.</p>
        ) : null}
        {children.map((child) => {
          const running = activeStates.has(child.state)
          const busy = busyId === child.session_id
          return (
            <section className="codex-subagent-card" key={child.session_id} aria-label={child.description || child.agent}>
              <div className="codex-subagent-header">
                <span className={`codex-subagent-state codex-subagent-state-${child.state}`} title={child.state}>{child.state}</span>
                <strong>{child.description || child.agent}</strong>
              </div>
              <p className="codex-subagent-meta">@{child.agent} · {child.model || 'Inherited model'} · Gen {child.generation}</p>
              {child.awaiting_permission ? <p className="codex-subagent-waiting">Awaiting approval</p> : null}
              {child.pending_followups > 0 ? <p className="codex-subagent-meta">{child.pending_followups} queued follow-up(s)</p> : null}
              {child.error ? <p className="codex-panel-error">{child.error}</p> : null}
              {child.output_tail ? (
                <details className="codex-subagent-output">
                  <summary>Output preview</summary>
                  <pre>{child.output_tail}</pre>
                </details>
              ) : null}
              <div className="codex-subagent-actions">
                <button disabled={busy} onClick={() => onOpenSession(child)} aria-label={`Open transcript for ${child.description}`}><ArrowUpRight size={14} /> Transcript</button>
                {running ? <button disabled={busy} onClick={() => void act(child, 'stop')}><CirclePause size={14} /> Stop</button> : null}
                {!running && child.state !== 'completed' && !child.has_tool_calls && child.manual_retries < 3 ? (
                  <button disabled={busy} onClick={() => void act(child, 'retry')}><RefreshCcw size={14} /> Retry</button>
                ) : null}
                <button disabled={busy} onClick={() => { setFollowupId(followupId === child.session_id ? null : child.session_id); setFollowupText('') }}>
                  <Send size={14} /> Follow up
                </button>
              </div>
              {followupId === child.session_id ? (
                <form className="codex-subagent-followup" onSubmit={(event) => { event.preventDefault(); void act(child, 'continue') }}>
                  <label htmlFor={`followup-${child.session_id}`}>Follow-up instruction</label>
                  <textarea id={`followup-${child.session_id}`} maxLength={32768} value={followupText} onChange={(event) => setFollowupText(event.target.value)} />
                  <button disabled={busy || !followupText.trim()} type="submit">{running ? 'Queue follow-up' : 'Continue task'}</button>
                </form>
              ) : null}
            </section>
          )
        })}
      </div>
      {registryScope ? <p className="codex-panel-footnote">{registryScope}</p> : null}
    </aside>
  )
}
