import { useEffect, useMemo, useRef, useState } from 'react'
import { Search, X } from 'lucide-react'
import { searchConversations, searchSessionFiles, type AgentMention, type GlobalSearchResult } from '@/lib/api'

const EMPTY: GlobalSearchResult = { sessions: [], messages: [], limitations: '' }

export function GlobalSearch({ sessionId, agents, onClose, onOpenChat }: {
  sessionId: string | null
  agents: AgentMention[]
  onClose: () => void
  onOpenChat: (id: string) => void
}) {
  const [query, setQuery] = useState('')
  const [results, setResults] = useState<GlobalSearchResult>(EMPTY)
  const [files, setFiles] = useState<string[]>([])
  const [loading, setLoading] = useState(false)
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const inputRef = useRef<HTMLInputElement>(null)
  const dialogRef = useRef<HTMLElement>(null)
  useEffect(() => {
    const previous = document.activeElement instanceof HTMLElement ? document.activeElement : null
    inputRef.current?.focus()
    const keydown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') { event.preventDefault(); onClose() }
      if (event.key !== 'Tab' || !dialogRef.current) return
      const controls = Array.from(dialogRef.current.querySelectorAll<HTMLElement>('button:not([disabled]),input:not([disabled])'))
      if (!controls.length) return
      if (event.shiftKey && document.activeElement === controls[0]) {
        event.preventDefault(); controls[controls.length - 1].focus()
      } else if (!event.shiftKey && document.activeElement === controls[controls.length - 1]) {
        event.preventDefault(); controls[0].focus()
      }
    }
    document.addEventListener('keydown', keydown)
    return () => { document.removeEventListener('keydown', keydown); previous?.focus() }
  }, [onClose])
  useEffect(() => {
    const abort = new AbortController()
    const q = query.trim()
    if (q.length < 2) { setResults(EMPTY); setFiles([]); setLoading(false); return () => abort.abort() }
    setLoading(true); setError('')
    const timeout = window.setTimeout(() => {
      void Promise.all([searchConversations(q, abort.signal),
        sessionId ? searchSessionFiles(sessionId, q, abort.signal).catch(() => []) : Promise.resolve([])])
        .then(([matches, names]) => {
          if (abort.signal.aborted) return
          setResults(matches); setFiles(names)
        }).catch((cause) => {
          if (!abort.signal.aborted) setError(cause instanceof Error ? cause.message : 'Search unavailable')
        }).finally(() => { if (!abort.signal.aborted) setLoading(false) })
    }, 180)
    return () => { abort.abort(); clearTimeout(timeout) }
  }, [query, sessionId])
  const matchingAgents = useMemo(() => agents.filter((a) => query.trim().length >= 2
    && (a.name.toLowerCase().includes(query.toLowerCase()) || a.description.toLowerCase().includes(query.toLowerCase())))
    .slice(0, 12), [query, agents])
  const open = (id: string) => { onOpenChat(id); onClose() }
  const copy = (text: string) => {
    void navigator.clipboard?.writeText(text).then(() => setNotice(`Copied ${text}`))
      .catch(() => setError('Clipboard permission denied'))
  }
  const count = results.sessions.length + results.messages.length + files.length + matchingAgents.length
  return (
    <div className="codex-global-search-backdrop" onMouseDown={(e) => { if (e.target === e.currentTarget) onClose() }}>
      <section ref={dialogRef} role="dialog" aria-modal="true" aria-label="Global search" className="codex-global-search">
        <div className="codex-global-search-input">
          <Search size={18} aria-hidden />
          <label htmlFor="global-search-query" className="sr-only">Search conversations, agents, and current project files</label>
          <input id="global-search-query" ref={inputRef} type="search" value={query} maxLength={120}
            placeholder="Search conversations, agents, project files"
            onChange={(event) => setQuery(event.target.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter') {
                const first = results.sessions[0]?.id ?? results.messages[0]?.session_id
                if (first) { event.preventDefault(); open(first) }
              }
            }} />
          <button type="button" onClick={onClose} aria-label="Close global search"><X size={16} /></button>
        </div>
        <div className="codex-global-search-results" aria-busy={loading}>
          {loading ? <p role="status">Searching native index…</p> : null}
          {error ? <p role="alert">{error}</p> : null}
          {notice ? <p role="status">{notice}</p> : null}
          {query.length < 2 ? <p>Type at least two characters. Esc closes search.</p> : null}
          {query.length >= 2 && !loading && !count && !error ? <p>No matches found.</p> : null}
          {results.sessions.length ? <h3>Conversations</h3> : null}
          {results.sessions.map((hit) => <button type="button" key={hit.id} onClick={() => open(hit.id)}>
            <strong>{hit.title}</strong><small>Conversation</small>
          </button>)}
          {results.messages.length ? <h3>Messages</h3> : null}
          {results.messages.map((hit) => <button type="button" key={hit.id} onClick={() => open(hit.session_id)}>
            <strong>{hit.session_title}</strong><small>{hit.role}: {hit.snippet}</small>
          </button>)}
          {matchingAgents.length ? <h3>Agents</h3> : null}
          {matchingAgents.map((agent) => <button type="button" key={agent.name}
            onClick={() => copy(`@${agent.name} `)}>
            <strong>@{agent.name}</strong><small>{agent.description} · Copy mention</small>
          </button>)}
          {files.length ? <h3>Current session files</h3> : null}
          {files.map((path) => <button type="button" key={path} onClick={() => copy(`@${path} `)}>
            <strong>{path}</strong><small>Copy mention · not a file attachment</small>
          </button>)}
          <p className="codex-global-search-footnote">Indexed: inline local chat messages and titles; current-session file paths are searched separately.</p>
        </div>
      </section>
    </div>
  )
}
