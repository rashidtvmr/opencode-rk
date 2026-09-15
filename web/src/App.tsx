import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type ChangeEvent,
  type FormEvent,
} from 'react'
import {
  Archive,
  CircleHelp,
  Command,
  Copy,
  Ellipsis,
  GitFork,
  Menu,
  MessageSquarePlus,
  PanelLeftClose,
  Pencil,
  RefreshCw,
  Search,
  Settings,
  Sparkles,
} from 'lucide-react'

import { Composer, type ReasoningEffort } from '@/components/composer'
import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuItem,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import { Input } from '@/components/ui/input'
import { Textarea } from '@/components/ui/textarea'
import {
  archiveSession,
  branchSessionFromMessage,
  createSession,
  getForkProvenance,
  getHealth,
  listAssistantActivity,
  listMessages,
  listModels,
  listSessions,
  modelKey,
  prepareRetryBranch,
  renameSession,
  runTurnStream,
  type AssistantActivity,
  type ForkProvenance,
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

function MessageActions({
  message,
  onCopy,
  onBranch,
  onEdit,
  onRetry,
}: {
  message: MessageRecord
  onCopy: (message: MessageRecord) => void
  onBranch: (message: MessageRecord) => void
  onEdit: (message: MessageRecord) => void
  onRetry: (message: MessageRecord) => void
}) {
  if (message.role !== 'user' && message.role !== 'assistant') return null
  const role = message.role

  return (
    <div className="codex-message-actions" role="group" aria-label={`Actions for ${role} message`}>
      <Button
        size="icon-xs"
        variant="ghost"
        className="codex-message-action"
        aria-label={`Copy ${role} message`}
        onPress={() => onCopy(message)}
      >
        <Copy aria-hidden="true" />
      </Button>
      {role === 'user' ? (
        <Button
          size="icon-xs"
          variant="ghost"
          className="codex-message-action"
          aria-label="Edit user message"
          onPress={() => onEdit(message)}
        >
          <Pencil aria-hidden="true" />
        </Button>
      ) : null}
      <Button
        size="icon-xs"
        variant="ghost"
        className="codex-message-action"
        aria-label={role === 'user' ? 'Retry user message' : 'Regenerate assistant response'}
        onPress={() => onRetry(message)}
      >
        <RefreshCw aria-hidden="true" />
      </Button>
      <DropdownMenuTrigger>
        <Button
          size="icon-xs"
          variant="ghost"
          className="codex-message-action"
          aria-label={`Fork ${role} message`}
        >
          <GitFork aria-hidden="true" />
        </Button>
        <DropdownMenu placement={role === 'user' ? 'bottom end' : 'bottom start'}>
          <DropdownMenuItem onAction={() => onBranch(message)} aria-label="Branch in new chat">
            <GitFork aria-hidden="true" />
            Branch in new chat
          </DropdownMenuItem>
        </DropdownMenu>
      </DropdownMenuTrigger>
    </div>
  )
}

function App() {
  const [sessions, setSessions] = useState<SessionSummary[]>([])
  const [models, setModels] = useState<ModelSummary[]>([])
  const [health, setHealth] = useState<HealthResponse | null>(null)
  const [loadState, setLoadState] = useState<LoadState>('loading')
  const [error, setError] = useState('')
  const [selectedSessionId, setSelectedSessionId] = useState<string | null>(null)
  const [selectedModel, setSelectedModel] = useState('')
  const [selectedReasoningEffort, setSelectedReasoningEffort] = useState<ReasoningEffort>('high')
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
  const [assistantActivity, setAssistantActivity] = useState<Record<string, AssistantActivity>>({})
  const [forkProvenance, setForkProvenance] = useState<ForkProvenance | null>(null)
  const [editingMessageId, setEditingMessageId] = useState<string | null>(null)
  const [editingMessageText, setEditingMessageText] = useState('')
  const [editingMessageBusy, setEditingMessageBusy] = useState(false)
  const [editingSessionId, setEditingSessionId] = useState<string | null>(null)
  const [editingTitle, setEditingTitle] = useState('')
  const [streamingAssistantText, setStreamingAssistantText] = useState('')
  const [streamingReasoningSummary, setStreamingReasoningSummary] = useState('')
  const [activeTurnSessionId, setActiveTurnSessionId] = useState<string | null>(null)
  const activeTurnRef = useRef<{ sessionId: string; controller: AbortController } | null>(null)

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
      setAssistantActivity({})
      setForkProvenance(null)
      setMessageLoadState('idle')
      setMessageError('')
      return
    }

    if (activeTurnRef.current?.sessionId === selectedSessionId) {
      return
    }

    const controller = new AbortController()
    setMessages([])
    setAssistantActivity({})
    setForkProvenance(null)
    setMessageLoadState('loading')
    setMessageError('')

    Promise.all([
      listMessages(selectedSessionId, 200, controller.signal),
      listAssistantActivity(selectedSessionId, 200, controller.signal),
      getForkProvenance(selectedSessionId, controller.signal),
    ])
      .then(([result, activity, provenance]) => {
        if (controller.signal.aborted) return
        setMessages(result)
        setAssistantActivity(
          Object.fromEntries(activity.map((entry) => [entry.message_id, entry])) as Record<
            string,
            AssistantActivity
          >,
        )
        setForkProvenance(provenance)
        setMessageLoadState('ready')
      })
      .catch((cause: unknown) => {
        if (controller.signal.aborted) return
        setMessageError(cause instanceof Error ? cause.message : 'Could not load messages')
        setMessageLoadState('error')
      })

    return () => controller.abort()
  }, [selectedSessionId])

  useEffect(() => {
    const active = activeTurnRef.current
    if (active && active.sessionId !== selectedSessionId) {
      active.controller.abort()
      activeTurnRef.current = null
      setActiveTurnSessionId(null)
      setStreamingAssistantText('')
      setStreamingReasoningSummary('')
    }
  }, [selectedSessionId])

  useEffect(
    () => () => {
      activeTurnRef.current?.controller.abort()
      activeTurnRef.current = null
    },
    [],
  )

  const filteredSessions = useMemo(() => {
    const needle = query.trim().toLowerCase()
    if (!needle) return sessions
    return sessions.filter((session) => session.title.toLowerCase().includes(needle))
  }, [query, sessions])

  const selectedSession = sessions.find((session) => session.id === selectedSessionId) ?? null
  const forkParent = forkProvenance
    ? sessions.find((session) => session.id === forkProvenance.parent_session_id) ?? null
    : null

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

  const handleCopyMessage = (message: MessageRecord) => {
    const text = messageText(message)
    if (!navigator.clipboard?.writeText) {
      setNotice('Copy is unavailable in this browser context.')
      return
    }
    void navigator.clipboard
      .writeText(text)
      .then(() => setNotice(`Copied ${message.role} message.`))
      .catch(() => setNotice('Could not copy this message.'))
  }

  const handleBranchMessage = async (message: MessageRecord) => {
    if (!selectedSessionId || message.session_id !== selectedSessionId) return
    try {
      const result = await branchSessionFromMessage(selectedSessionId, message.id)
      setSessions((current) => [
        result.session,
        ...current.filter((session) => session.id !== result.session.id),
      ])
      setForkProvenance(result.fork)
      setSelectedSessionId(result.session.id)
      setNotice(`Branched chat from ${message.role} message.`)
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : 'Could not branch this chat')
    }
  }

  const executeTurnInSession = async (
    sessionId: string,
    text: string,
    reasoningEffort: ReasoningEffort,
    initialMessages?: MessageRecord[],
    initialActivity?: AssistantActivity[],
  ) => {
    if (!selectedModel) {
      setNotice('Choose a model before sending a message.')
      return false
    }

    activeTurnRef.current?.controller.abort()
    const controller = new AbortController()
    activeTurnRef.current = { sessionId, controller }
    setActiveTurnSessionId(sessionId)
    setStreamingAssistantText('')
    setStreamingReasoningSummary('')
    const baselineMessages = initialMessages ?? messages
    if (initialMessages) setMessages(initialMessages)
    if (initialActivity) {
      setAssistantActivity(
        Object.fromEntries(initialActivity.map((entry) => [entry.message_id, entry])) as Record<
          string,
          AssistantActivity
        >,
      )
    }
    const previousIds = new Set(baselineMessages.map((message) => message.id))

    try {
      const turn = await runTurnStream(
        sessionId,
        text,
        selectedModel,
        reasoningEffort,
        {
          onUserMessage: (message) => {
            if (activeTurnRef.current?.controller !== controller) return
            setMessages((current) =>
              current.some((item) => item.id === message.id) ? current : [...current, message],
            )
          },
          onReasoningSummaryDelta: (delta) => {
            if (activeTurnRef.current?.controller !== controller) return
            setStreamingReasoningSummary((current) => current + delta)
          },
          onAssistantDelta: (delta) => {
            if (activeTurnRef.current?.controller !== controller) return
            setStreamingAssistantText((current) => current + delta)
          },
          onAssistantActivity: (activity) => {
            if (activeTurnRef.current?.controller !== controller) return
            setAssistantActivity((current) => ({ ...current, [activity.message_id]: activity }))
            setStreamingReasoningSummary('')
          },
          onAssistantMessage: (message) => {
            if (activeTurnRef.current?.controller !== controller) return
            setStreamingAssistantText('')
            setStreamingReasoningSummary('')
            setMessages((current) =>
              current.some((item) => item.id === message.id) ? current : [...current, message],
            )
          },
        },
        controller.signal,
      )
      if (activeTurnRef.current?.controller !== controller) return false
      activeTurnRef.current = null
      setActiveTurnSessionId(null)
      setMessageLoadState('ready')
      setNotice(
        turn.executed
          ? 'Assistant reply received.'
          : 'Message saved. Upgrade the native server to enable turn execution.',
      )
      return true
    } catch (cause) {
      if (controller.signal.aborted) {
        if (activeTurnRef.current?.controller === controller) activeTurnRef.current = null
        setActiveTurnSessionId((current) => (current === sessionId ? null : current))
        setStreamingAssistantText('')
        setStreamingReasoningSummary('')
        setNotice('Turn stopped.')
        return false
      }
      if (activeTurnRef.current?.controller === controller) activeTurnRef.current = null
      setActiveTurnSessionId((current) => (current === sessionId ? null : current))
      setStreamingAssistantText('')
      setStreamingReasoningSummary('')
      let persisted = false
      try {
        const [refreshed, refreshedActivity] = await Promise.all([
          listMessages(sessionId, 200),
          listAssistantActivity(sessionId, 200),
        ])
        persisted = refreshed.some(
          (message) =>
            !previousIds.has(message.id) &&
            message.role === 'user' &&
            message.body.storage === 'inline' &&
            message.body.text === text,
        )
        setMessages(refreshed)
        setAssistantActivity(
          Object.fromEntries(
            refreshedActivity.map((entry) => [entry.message_id, entry]),
          ) as Record<string, AssistantActivity>,
        )
      } catch {
        // Keep the existing transcript when a follow-up refresh also fails.
      }
      setNotice(cause instanceof Error ? cause.message : 'Could not execute the turn')
      return persisted
    }
  }

  const handleComposerSubmit = async (text: string, reasoningEffort: ReasoningEffort) => {
    if (!selectedSessionId) return false
    setSelectedReasoningEffort(reasoningEffort)
    return executeTurnInSession(selectedSessionId, text, reasoningEffort)
  }

  const handleStopTurn = () => {
    const active = activeTurnRef.current
    if (!active || active.sessionId !== selectedSessionId) return
    active.controller.abort()
  }

  const handleRetryMessage = async (message: MessageRecord, editedText?: string) => {
    if (!selectedSessionId || message.session_id !== selectedSessionId) return false
    if (!selectedModel) {
      setNotice('Choose a model before retrying a message.')
      return false
    }
    const editing = editedText != null
    if (editing && editingMessageBusy) return false
    if (editing) setEditingMessageBusy(true)

    try {
      const result = await prepareRetryBranch(selectedSessionId, message.id)
      const [childMessages, childActivity] = await Promise.all([
        listMessages(result.session.id, 200),
        listAssistantActivity(result.session.id, 200),
      ])
      const requestText = editedText?.trim() || result.requestText

      setSessions((current) => [
        result.session,
        ...current.filter((session) => session.id !== result.session.id),
      ])
      setForkProvenance(result.fork)
      setMessages(childMessages)
      setAssistantActivity(
        Object.fromEntries(childActivity.map((entry) => [entry.message_id, entry])) as Record<
          string,
          AssistantActivity
        >,
      )
      setMessageLoadState('ready')
      setMessageError('')
      setEditingMessageId(null)
      setEditingMessageText('')
      setSelectedSessionId(result.session.id)

      const accepted = await executeTurnInSession(
        result.session.id,
        requestText,
        selectedReasoningEffort,
        childMessages,
        childActivity,
      )
      if (accepted) {
        setNotice(
          editedText == null
            ? message.role === 'assistant'
              ? 'Regenerated response in a new branch.'
              : 'Retried request in a new branch.'
            : 'Sent edited request in a new branch.',
        )
      }
      return accepted
    } catch (cause) {
      setNotice(cause instanceof Error ? cause.message : 'Could not prepare retry branch')
      return false
    } finally {
      if (editing) setEditingMessageBusy(false)
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
                  {forkProvenance ? (
                    <div className="codex-fork-lineage" aria-label="Branch lineage">
                      <GitFork aria-hidden="true" />
                      <span>Branched from</span>
                      {forkParent ? (
                        <Button
                          variant="link"
                          size="xs"
                          className="codex-fork-parent"
                          onPress={() => setSelectedSessionId(forkParent.id)}
                        >
                          {forkParent.title}
                        </Button>
                      ) : (
                        <span>parent chat</span>
                      )}
                      <span>at message {forkProvenance.fork_message_seq}</span>
                    </div>
                  ) : null}
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

                {messages.length > 0 || streamingAssistantText || streamingReasoningSummary ? (
                  <ol
                    className="codex-transcript"
                    aria-label="Conversation messages"
                    aria-busy={streamingAssistantText || streamingReasoningSummary ? true : undefined}
                  >
                    {messages.map((message) => (
                      <li
                        key={message.id}
                        className={`codex-message codex-message-row codex-message-${message.role}`}
                      >
                        <div className="codex-message-stack">
                          {editingMessageId === message.id && message.role === 'user' ? (
                            <form
                              className="codex-message-edit"
                              onSubmit={(event: FormEvent<HTMLFormElement>) => {
                                event.preventDefault()
                                void handleRetryMessage(message, editingMessageText)
                              }}
                            >
                              <label className="sr-only" htmlFor={`edit-message-${message.id}`}>
                                Edit user message
                              </label>
                              <Textarea
                                id={`edit-message-${message.id}`}
                                aria-label="Edit user message"
                                value={editingMessageText}
                                onChange={(event: ChangeEvent<HTMLTextAreaElement>) =>
                                  setEditingMessageText(event.target.value)
                                }
                                rows={4}
                                autoFocus
                                disabled={editingMessageBusy}
                              />
                              <div className="codex-message-edit-actions">
                                <Button
                                  size="sm"
                                  variant="ghost"
                                  isDisabled={editingMessageBusy}
                                  onPress={() => {
                                    setEditingMessageId(null)
                                    setEditingMessageText('')
                                  }}
                                >
                                  Cancel
                                </Button>
                                <Button
                                  size="sm"
                                  type="submit"
                                  isDisabled={editingMessageBusy || !editingMessageText.trim()}
                                >
                                  Send edit in new chat
                                </Button>
                              </div>
                            </form>
                          ) : (
                            <>
                              {message.role === 'assistant' && assistantActivity[message.id] ? (
                                <details className="codex-assistant-activity">
                                  <summary>Reasoning summary</summary>
                                  <p>{assistantActivity[message.id].reasoning_summary}</p>
                                </details>
                              ) : null}
                              <div className="codex-message-content">
                                <span className="sr-only">{message.role}: </span>
                                {messageText(message)}
                              </div>
                            </>
                          )}
                          <MessageActions
                            message={message}
                            onCopy={handleCopyMessage}
                            onBranch={(candidate) => void handleBranchMessage(candidate)}
                            onEdit={(candidate) => {
                              if (candidate.body.storage !== 'inline') {
                                setNotice('Only text messages can be edited right now.')
                                return
                              }
                              setEditingMessageId(candidate.id)
                              setEditingMessageText(candidate.body.text)
                            }}
                            onRetry={(candidate) => void handleRetryMessage(candidate)}
                          />
                        </div>
                      </li>
                    ))}
                    {streamingAssistantText || streamingReasoningSummary ? (
                      <li className="codex-message codex-message-row codex-message-assistant">
                        <div className="codex-message-stack">
                          {streamingReasoningSummary ? (
                            <details className="codex-assistant-activity" open>
                              <summary>Reasoning summary</summary>
                              <p>{streamingReasoningSummary}</p>
                            </details>
                          ) : null}
                          {streamingAssistantText ? (
                            <div className="codex-message-content">
                              <span className="sr-only">assistant: </span>
                              {streamingAssistantText}
                            </div>
                          ) : null}
                        </div>
                      </li>
                    ) : null}
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
              sessionId={selectedSession?.id ?? null}
              running={activeTurnSessionId === selectedSession?.id}
              models={models}
              selectedModel={selectedModel}
              onModelChange={setSelectedModel}
              onReasoningEffortChange={setSelectedReasoningEffort}
              onStop={handleStopTurn}
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
