import {
  useEffect,
  useMemo,
  useState,
  type ChangeEvent,
  type FormEvent,
} from 'react'
import {
  Archive,
  CircleHelp,
  Command,
  Ellipsis,
  Menu,
  MessageSquarePlus,
  PanelLeftClose,
  Pencil,
  Search,
  Settings,
  Sparkles,
} from 'lucide-react'

import { Composer } from '@/components/composer'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import {
  appendMessage,
  archiveSession,
  createSession,
  getHealth,
  listMessages,
  listModels,
  listSessions,
  modelKey,
  renameSession,
  type HealthResponse,
  type MessageRecord,
  type ModelSummary,
  type SessionSummary,
} from '@/lib/api'

type LoadState = 'loading' | 'ready' | 'error'
type MessageLoadState = 'idle' | LoadState

function messageText(message: MessageRecord) {
  return message.body.storage === 'inline'
    ? message.body.text
    : `Attachment · ${message.body.bytes} bytes`
}

function App() {
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const [models, setModels] = useState<ModelSummary[]>([])
  const [health, setHealth] = useState<HealthResponse | null>(null)
  const [loadState, setLoadState] = useState<LoadState>('loading')
  const [error, setError] = useState('')
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null)
  const [selectedModel, setSelectedModel] = useState('')
  const [query, setQuery] = useState('')
  const [sidebarOpen, setSidebarOpen] = useState(() =>
    typeof window.matchMedia === 'function'
      ? !window.matchMedia('(max-width: 720px)').matches
      : true,
  )
  const [creating, setCreating] = useState(false)
  const [newTitle, setNewTitle] = useState('')
  const [notice, setNotice] = useState('')
  const [messages, setMessages] = useState<MessageRecord[]>([])
  const [messageLoadState, setMessageLoadState] = useState<MessageLoadState>('idle')
  const [messageError, setMessageError] = useState('')
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null)
  const [editingTitle, setEditingTitle] = useState('')

  useEffect(() => {
    const controller = new AbortController()

    Promise.all([getHealth(), listSessions(), listModels()])
      .then(([healthResult, sessionResult, modelResult]) => {
        if (controller.signal.aborted) return
        setHealth(healthResult)
        setSessions(sessionResult)
        setModels(modelResult)
        setSelectedSessionId((current) => current ?? sessionResult[0]?.id ?? null)
        setSelectedModel((current) => current || (modelResult[0] ? modelKey(modelResult[0], 0) : ''))
        setLoadState('ready')
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return
        setError(cause instanceof Error ? cause.message : 'Could not connect to the native server')
        setLoadState('error')
      })

    return () => controller.abort()
  }, [])

  useEffect(() => {
    if (!selectedSessionId) {
      setMessages([])
      setMessageLoadState('idle')
      setMessageError('')
      return
    }

    const controller = new AbortController()
    setMessages([])
    setMessageLoadState('loading')
    setMessageError('')

    listMessages(selectedSessionId, 200, controller.signal)
      .then((result) => {
        if (controller.signal.aborted) return
        setMessages(result)
        setMessageLoadState('ready')
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return
        setMessageError(cause instanceof Error ? cause.message : 'Could not load messages')
        setMessageLoadState('error')
      })

    return () => controller.abort()
  }, [selectedSessionId])

  const filteredSessions = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return sessions
    return sessions.filter((session) => session.title.toLowerCase().includes(needle))
  }, [query, sessions])

  const selectedSession = sessions.find((session) => session.id === selectedSessionId) ?? null

  const handleCreateSession = async () => {
    const title = newTitle.trim()
    if (!title) return

    try {
      const created = await createSession(title)
      setSessions((current) => [created, ...current])
      setSelectedSessionId(created.id)
      setNewTitle('')
      setCreating(false)
      setNotice(`Created ${created.title}`)
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : 'Could not create the chat')
    }
  }

  const handleRenameSession = async () => {
    if (!editingSessionId) return
    const title = editingTitle.trim()
    if (!title) return

    try {
      const renamed = await renameSession(editingSessionId, title)
      setSessions((current) =>
        current.map((session) => (session.id === renamed.id ? renamed : session)),
      )
      setEditingSessionId(null)
      setEditingTitle('')
      setNotice(`Renamed chat to ${renamed.title}`)
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : 'Could not rename the chat')
    }
  }

  const handleArchiveSession = async (id: string) => {
    try {
      const archived = await archiveSession(id)
      const remaining = sessions.filter((session) => session.id !== id)
      setSessions(remaining)
      if (selectedSessionId === id) setSelectedSessionId(remaining[0]?.id ?? null)
      if (editingSessionId === id) {
        setEditingSessionId(null)
        setEditingTitle('')
      }
      setNotice(`Archived ${archived.title}`)
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : 'Could not archive the chat')
    }
  }

  const handleComposerSubmit = async (text: string) => {
    if (!selectedSessionId) return false

    try {
      const message = await appendMessage(selectedSessionId, text)
      setMessages((current) => [...current, message])
      setMessageLoadState('ready')
      setNotice('Message saved. Agent turn execution is not connected yet.')
      return true
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : 'Could not save the message')
      return false
    }
  }

  return (
    <div className={`codex-app ${sidebarOpen ? '' : 'codex-sidebar-collapsed'}`}>
      <a className="skip-link" href="#main-content">
        Skip to main content
      </a>

      <aside id="sessions" className="codex-sidebar" aria-label="Sessions" hidden={!sidebarOpen}>
        <div className="codex-sidebar-header">
          <div className="codex-brand" aria-label="OpenCode RK">
            <span className="codex-brand-mark" aria-hidden="true">
              <Command />
            </span>
            <span>OpenCode RK</span>
          </div>
          <Button
            size="icon"
            variant="ghost"
            className="codex-icon-button"
            aria-label="Collapse sidebar"
            onPress={() => setSidebarOpen(false)}
          >
            <PanelLeftClose aria-hidden="true" />
          </Button>
        </div>

        <nav aria-label="Primary" className="codex-primary-nav">
          <Button
            variant="ghost"
            className="codex-new-chat"
            onPress={() => setCreating(true)}
          >
            <MessageSquarePlus aria-hidden="true" />
            New chat
          </Button>
          <a href="#models" className="codex-nav-link">
            <Sparkles aria-hidden="true" />
            Models
          </a>
          <a href="#settings" className="codex-nav-link">
            <Settings aria-hidden="true" />
            Settings
          </a>
        </nav>

        {creating ? (
          <form
            className="codex-new-chat-form"
            onSubmit={(event: FormEvent<HTMLFormElement>) => {
              event.preventDefault()
              void handleCreateSession()
            }}
          >
            <label htmlFor="new-session-title">Chat title</label>
            <Input
              id="new-session-title"
              value={newTitle}
              onChange={(event: ChangeEvent<HTMLInputElement>) => setNewTitle(event.target.value)}
              autoFocus
              maxLength={160}
              placeholder="What are you working on?"
            />
            <div className="flex justify-end gap-1.5">
              <Button size="sm" variant="ghost" onPress={() => setCreating(false)}>
                Cancel
              </Button>
              <Button size="sm" type="submit" isDisabled={!newTitle.trim()}>
                Create
              </Button>
            </div>
          </form>
        ) : null}

        <div className="codex-sidebar-search">
          <Search aria-hidden="true" />
          <label className="sr-only" htmlFor="session-search">
            Search chats
          </label>
          <Input
            id="session-search"
            type="search"
            value={query}
            onChange={(event: ChangeEvent<HTMLInputElement>) => setQuery(event.target.value)}
            placeholder="Search chats"
            className="border-0 bg-transparent shadow-none focus-visible:ring-0 dark:bg-transparent"
          />
        </div>

        <div className="codex-history" aria-label="Chat history">
          <p className="codex-history-label">Chats</p>

          {loadState === 'loading' ? (
            <p className="codex-sidebar-message" role="status">
              Loading chats…
            </p>
          ) : null}

          {loadState === 'error' ? (
            <p className="codex-sidebar-message text-destructive" role="alert">
              {error}
            </p>
          ) : null}

          {loadState === 'ready' && filteredSessions.length === 0 ? (
            <p className="codex-sidebar-message">No chats found.</p>
          ) : null}

          <ul className="m-0 list-none p-0" aria-label="Session history">
            {filteredSessions.map((session) => (
              <li key={session.id} className="codex-history-item">
                <div className="codex-history-row-wrap">
                  <button
                    type="button"
                    className={`codex-history-row ${session.id === selectedSessionId ? 'codex-history-row-active' : ''}`}
                    onClick={() => {
                      setSelectedSessionId(session.id)
                      if (window.matchMedia('(max-width: 720px)').matches) setSidebarOpen(false)
                    }}
                    aria-current={session.id === selectedSessionId ? 'page' : undefined}
                  >
                    <span className="truncate">{session.title}</span>
                  </button>

                  <DropdownMenuTrigger>
                    <Button
                      size="icon-sm"
                      variant="ghost"
                      className="codex-history-action"
                      aria-label={`Chat actions for ${session.title}`}
                    >
                      <Ellipsis aria-hidden="true" />
                    </Button>
                    <DropdownMenu placement="bottom end">
                      <DropdownMenuItem
                        onAction={() => {
                          setEditingSessionId(session.id)
                          setEditingTitle(session.title)
                        }}
                      >
                        <Pencil aria-hidden="true" />
                        Rename chat
                      </DropdownMenuItem>
                      <DropdownMenuSeparator />
                      <DropdownMenuItem
                        variant="destructive"
                        onAction={() => void handleArchiveSession(session.id)}
                      >
                        <Archive aria-hidden="true" />
                        Archive chat
                      </DropdownMenuItem>
                    </DropdownMenu>
                  </DropdownMenuTrigger>
                </div>

                {editingSessionId === session.id ? (
                  <form
                    className="codex-history-rename"
                    onSubmit={(event: FormEvent<HTMLFormElement>) => {
                      event.preventDefault()
                      void handleRenameSession()
                    }}
                  >
                    <label className="sr-only" htmlFor={`rename-${session.id}`}>
                      Rename chat
                    </label>
                    <Input
                      id={`rename-${session.id}`}
                      aria-label="Rename chat"
                      value={editingTitle}
                      onChange={(event: ChangeEvent<HTMLInputElement>) =>
                        setEditingTitle(event.target.value)
                      }
                      autoFocus
                      maxLength={160}
                    />
                    <div className="flex justify-end gap-1">
                      <Button
                        size="xs"
                        variant="ghost"
                        aria-label="Cancel rename"
                        onPress={() => {
                          setEditingSessionId(null)
                          setEditingTitle('')
                        }}
                      >
                        Cancel
                      </Button>
                      <Button
                        size="xs"
                        type="submit"
                        aria-label="Save rename"
                        isDisabled={!editingTitle.trim()}
                      >
                        Save
                      </Button>
                    </div>
                  </form>
                ) : null}
              </li>
            ))}
          </ul>
        </div>

        <div className="codex-sidebar-footer">
          <span
            className={`codex-status-dot ${health?.status === 'ok' ? 'codex-status-dot-online' : ''}`}
            aria-hidden="true"
          />
          <span>
            {loadState === 'loading' ? 'Connecting' : health?.status === 'ok' ? 'Local' : 'Offline'}
          </span>
          <span className="ml-auto text-muted-foreground">Rust</span>
        </div>
      </aside>

      <main id="main-content" className="codex-main" tabIndex={-1}>
        <header className="codex-header">
          <div className="codex-header-left">
            {!sidebarOpen ? (
              <Button
                size="icon"
                variant="ghost"
                className="codex-icon-button"
                aria-label="Open sidebar"
                onPress={() => setSidebarOpen(true)}
              >
                <Menu aria-hidden="true" />
              </Button>
            ) : null}
            <span className="codex-header-title">{selectedSession?.title ?? 'New chat'}</span>
          </div>

          <div className="codex-header-actions" id="settings">
            <div className="codex-connection" title={health?.runtime ?? 'Native server'}>
              <span
                className={`codex-status-dot ${health?.status === 'ok' ? 'codex-status-dot-online' : ''}`}
                aria-hidden="true"
              />
              <span className="hidden sm:inline">{health?.status === 'ok' ? 'Local' : 'Offline'}</span>
            </div>
            <Button
              size="icon"
              variant="ghost"
              className="codex-icon-button"
              aria-label="Help"
            >
              <CircleHelp aria-hidden="true" />
            </Button>
            <Button
              size="icon"
              variant="ghost"
              className="codex-icon-button"
              aria-label="Settings"
            >
              <Settings aria-hidden="true" />
            </Button>
          </div>
        </header>

        <section className="codex-conversation" aria-labelledby="conversation-title">
          <div className="codex-conversation-inner">
            {selectedSession ? (
              <>
                <div className="codex-session-heading">
                  <h1 id="conversation-title">{selectedSession.title}</h1>
                  {messages.length === 0 && messageLoadState === 'ready' ? (
                    <p>Send a message to begin. Messages are stored locally in the native session database.</p>
                  ) : null}
                </div>

                {messageLoadState === 'loading' ? (
                  <p className="codex-transcript-status" role="status">
                    Loading messages…
                  </p>
                ) : null}

                {messageLoadState === 'error' ? (
                  <p className="codex-transcript-status text-destructive" role="alert">
                    {messageError}
                  </p>
                ) : null}

                {messages.length > 0 ? (
                  <ol className="codex-transcript" aria-label="Conversation messages">
                    {messages.map((message) => (
                      <li
                        key={message.id}
                        className={`codex-message codex-message-${message.role}`}
                      >
                        <div className="codex-message-content">
                          <span className="sr-only">{message.role}: </span>
                          {messageText(message)}
                        </div>
                      </li>
                    ))}
                  </ol>
                ) : null}
              </>
            ) : (
              <div className="codex-empty-state">
                <div className="codex-empty-mark" aria-hidden="true">
                  <Sparkles />
                </div>
                <h1 id="conversation-title">What can I help you build?</h1>
                <p>Create a chat, choose a model, and start working with your local coding agent.</p>
              </div>
            )}
          </div>
        </section>

        <footer className="codex-composer-dock" id="models">
          <div className="codex-composer-wrap">
            <Composer
              disabled={!selectedSession}
              models={models}
              selectedModel={selectedModel}
              onModelChange={setSelectedModel}
              onSubmit={handleComposerSubmit}
            />
            <p className="codex-composer-caption">
              OpenCode RK can make mistakes. Review code and commands before applying them.
            </p>
          </div>
        </footer>
      </main>

      <div className="sr-only" aria-live="polite" aria-atomic="true">
        {notice}
      </div>
    </div>
  )
}

export default App
