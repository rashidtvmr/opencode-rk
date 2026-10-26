import { useEffect, useRef, useState, type FormEvent, type ReactNode } from 'react'
import {
  Bot, Check, ChevronLeft, CircleAlert, Code2, FileText, Folder, FolderGit2,
  KeyRound, ListTodo, LoaderCircle, PlugZap, RefreshCcw, Server, ShieldCheck,
  Square, Terminal, X,
} from 'lucide-react'
import {
  addDisabledMcp, browseSessionFiles, listCustomProviders, listMcpStatuses,
  listPermissionRules, listSessionTodos, modelKey, modelLabel, getSessionGit, getSessionGitDiff, runNativeShellStream,
  saveCustomProvider, savePermissionRule, saveSessionTodos, setMcpConnection,
  type BrowserFileResult, type CommandInventory, type ModelSummary,
  type NativeMcpStatus, type NativePermissionRule, type NativeProvider, type SessionGitInfo,
  type NativeTodo, type WorkspaceCatalogState,
} from '@/lib/api'

export type HubPage =
  | 'projects' | 'files' | 'jobs' | 'agents' | 'models'
  | 'providers' | 'mcp' | 'permissions' | 'tasks'

const PAGES: { id: HubPage; label: string; icon: typeof Folder }[] = [
  { id: 'projects', label: 'Projects', icon: FolderGit2 },
  { id: 'files', label: 'Files', icon: Folder },
  { id: 'jobs', label: 'Shell & jobs', icon: Terminal },
  { id: 'agents', label: 'Agents', icon: Bot },
  { id: 'models', label: 'Models', icon: Code2 },
  { id: 'providers', label: 'Providers', icon: Server },
  { id: 'mcp', label: 'MCP servers', icon: PlugZap },
  { id: 'permissions', label: 'Permissions', icon: ShieldCheck },
  { id: 'tasks', label: 'Tasks', icon: ListTodo },
]

interface WorkspaceHubProps {
  page: HubPage
  onPageChange: (page: HubPage) => void
  onClose: () => void
  onOpenSubagents: () => void
  sessionId: string | null
  workspaces: WorkspaceCatalogState
  models: ModelSummary[]
  inventory: CommandInventory
  onChooseModel: (model: string) => void
}

function PanelTitle({ title, description, extra }: { title: string; description: string; extra?: ReactNode }) {
  return (
    <div className="codex-hub-heading">
      <div><h2>{title}</h2><p>{description}</p></div>
      {extra}
    </div>
  )
}

function AsyncNotice({ error, notice }: { error: string; notice: string }) {
  return (
    <>
      {error ? <p role="alert" className="codex-hub-error"><CircleAlert size={15} aria-hidden />{error}</p> : null}
      {notice ? <p role="status" className="codex-hub-notice"><Check size={15} aria-hidden />{notice}</p> : null}
    </>
  )
}

function Empty({ children }: { children: ReactNode }) {
  return <p className="codex-hub-empty">{children}</p>
}

export function WorkspaceHub({
  page, onPageChange, onClose, onOpenSubagents, sessionId, workspaces,
  models, inventory, onChooseModel,
}: WorkspaceHubProps) {
  const [error, setError] = useState('')
  const [notice, setNotice] = useState('')
  const [busy, setBusy] = useState(false)
  const [loading, setLoading] = useState(false)
  const [version, setVersion] = useState(0)
  const [filePath, setFilePath] = useState('.')
  const [file, setFile] = useState<BrowserFileResult | null>(null)
  const [git, setGit] = useState<SessionGitInfo | null>(null)
  const [gitDiff, setGitDiff] = useState<string | null>(null)
  const [loadingDiff, setLoadingDiff] = useState(false)
  const [providers, setProviders] = useState<NativeProvider[]>([])
  const [mcp, setMcp] = useState<Record<string, NativeMcpStatus>>({})
  const [rules, setRules] = useState<NativePermissionRule[]>([])
  const [todos, setTodos] = useState<NativeTodo[]>([])
  const [providerId, setProviderId] = useState('')
  const [providerName, setProviderName] = useState('')
  const [providerUrl, setProviderUrl] = useState('https://')
  const [providerKey, setProviderKey] = useState('')
  const [mcpName, setMcpName] = useState('')
  const [mcpArgs, setMcpArgs] = useState('["node", "server.js"]')
  const [confirmConnect, setConfirmConnect] = useState<string | null>(null)
  const [permissionTool, setPermissionTool] = useState('bash')
  const [permissionDecision, setPermissionDecision] = useState<'ask' | 'allow' | 'deny'>('ask')
  const [permissionScope, setPermissionScope] = useState<'session' | 'always'>('session')
  const [acknowledgeAllow, setAcknowledgeAllow] = useState(false)
  const [newTodo, setNewTodo] = useState('')
  const [shellCommand, setShellCommand] = useState('')
  const [shellLog, setShellLog] = useState('')
  const [shellRunning, setShellRunning] = useState(false)
  const shellAbort = useRef<AbortController | null>(null)
  const hubRef = useRef<HTMLElement | null>(null)

  useEffect(() => { hubRef.current?.focus() }, [page])

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === 'Escape') {
        if (shellRunning) return
        onClose()
      }
    }
    document.addEventListener('keydown', onKeyDown)
    return () => document.removeEventListener('keydown', onKeyDown)
  }, [onClose, shellRunning])

  useEffect(() => {
    const controller = new AbortController()
    setError('')
    setNotice('')
    setLoading(true)
    const load = async () => {
      try {
        switch (page) {
          case 'projects':
            setGit(sessionId ? await getSessionGit(sessionId, controller.signal) : null)
            break
          case 'files':
            setFile(sessionId ? await browseSessionFiles(sessionId, filePath, controller.signal) : null)
            break
          case 'providers':
            setProviders(await listCustomProviders(controller.signal))
            break
          case 'mcp':
            setMcp(await listMcpStatuses(controller.signal))
            break
          case 'permissions':
            setRules(await listPermissionRules(sessionId ?? undefined, controller.signal))
            break
          case 'tasks':
            setTodos(sessionId ? await listSessionTodos(sessionId, controller.signal) : [])
            break
        }
      } catch (cause) {
        if (!controller.signal.aborted) setError(cause instanceof Error ? cause.message : 'Unable to load this page')
      } finally {
        if (!controller.signal.aborted) setLoading(false)
      }
    }
    void load()
    return () => controller.abort()
  }, [page, sessionId, filePath, version])

  useEffect(() => {
    setFilePath('.')
  }, [sessionId])

  // Credentials are never persisted in browser state beyond the provider form.
  useEffect(() => {
    if (page !== 'providers') setProviderKey('')
  }, [page])

  useEffect(() => {
    setGitDiff(null)
  }, [filePath, sessionId])

  useEffect(() => () => shellAbort.current?.abort(), [])

  const reload = () => setVersion((v) => v + 1)
  const mutate = async (action: () => Promise<void>, success: string) => {
    if (busy) return
    setBusy(true)
    setNotice('')
    setError('')
    try {
      await action()
      setNotice(success)
      reload()
    } catch (cause) {
      setError(cause instanceof Error ? cause.message : 'Action failed')
    } finally {
      setBusy(false)
    }
  }

  const changeTodos = (next: NativeTodo[]) => {
    if (!sessionId) return
    void mutate(async () => { setTodos(await saveSessionTodos(sessionId, next)) }, 'Tasks saved in session ledger')
  }

  const submitProvider = (event: FormEvent) => {
    event.preventDefault()
    void mutate(async () => {
      await saveCustomProvider({
        id: providerId.trim(), name: providerName.trim(),
        base_url: providerUrl.trim(), ...(providerKey ? { api_key: providerKey } : {}),
      })
      setProviderKey('')
    }, 'Provider configuration saved by the native daemon')
  }

  const submitMcp = (event: FormEvent) => {
    event.preventDefault()
    let command: unknown
    try { command = JSON.parse(mcpArgs) } catch { setError('Command must be a JSON string array'); return }
    if (!Array.isArray(command) || !command.every((arg) => typeof arg === 'string')) {
      setError('Command must be a JSON string array')
      return
    }
    void mutate(() => addDisabledMcp(mcpName.trim(), command as string[]), 'MCP server registered disabled; it will not execute until connected')
  }

  const submitPermission = (event: FormEvent) => {
    event.preventDefault()
    if (permissionDecision === 'allow' && !acknowledgeAllow) {
      setError('Confirm the warning before creating an allow rule')
      return
    }
    void mutate(() => savePermissionRule({
      tool: permissionTool, decision: permissionDecision,
      scope: permissionScope, session_id: permissionScope === 'session' ? sessionId : null,
    }), permissionDecision === 'ask' ? 'Matching rule reset to ask' : 'Rule saved on native daemon')
  }

  const runShell = async (event: FormEvent) => {
    event.preventDefault()
    if (!sessionId || !shellCommand.trim() || shellRunning) return
    const controller = new AbortController()
    shellAbort.current = controller
    setShellRunning(true)
    setShellLog(`$ ${shellCommand}\n`)
    setError('')
    setNotice('')
    try {
      await runNativeShellStream(sessionId, shellCommand, {
        onToolCall: (call) => setShellLog((log) => log + `\n▶ ${call.name}\n`),
        onToolOutput: (result) => setShellLog((log) => log + `${result.output}\n`),
      }, controller.signal)
      setNotice('Native shell stream completed; output is stored in the session transcript')
    } catch (cause) {
      if (!controller.signal.aborted) setError(cause instanceof Error ? cause.message : 'Shell execution failed')
      else setNotice('Shell request cancelled')
    } finally {
      shellAbort.current = null
      setShellRunning(false)
    }
  }

  return (
    <section ref={hubRef} className="codex-hub" aria-label="Workspace and developer tools" tabIndex={-1}>
      <div className="codex-hub-top">
        <strong>Workspace</strong>
        <button type="button" className="codex-panel-icon" onClick={onClose} aria-label="Close workspace"><X size={18} /></button>
      </div>
      <div className="codex-hub-layout">
        <nav className="codex-hub-navigation" aria-label="Workspace pages">
          {PAGES.map(({ id, label, icon: Icon }) => (
            <button type="button" key={id} className={page === id ? 'selected' : ''}
              aria-current={page === id ? 'page' : undefined} onClick={() => onPageChange(id)}>
              <Icon size={17} aria-hidden />{label}
            </button>
          ))}
        </nav>
        <div className="codex-hub-content">
          <AsyncNotice error={error} notice={notice} />
          {loading ? <p className="codex-hub-loading"><LoaderCircle className="codex-spin" size={16} /> Loading native data…</p> : null}

          {page === 'projects' ? (
            <>
              <PanelTitle title="Projects & workspaces" description="Native workspace registry and live read-only Git worktree inventory for the selected session."
                extra={<button type="button" className="codex-hub-button" onClick={reload}><RefreshCcw size={14} /> Refresh</button>} />
              {!workspaces.available ? <Empty>{workspaces.reason || 'No workspace registry is available.'}</Empty> : null}
              <div className="codex-hub-cards">
                {workspaces.workspaces.map((workspace) => (
                  <article className="codex-hub-card" key={workspace.id}>
                    <div className="codex-hub-card-head"><FolderGit2 size={18} /><strong>{workspace.label}</strong></div>
                    <code>{workspace.project_root ?? 'No project root configured'}</code>
                    <span>Workspace state: {workspace.status === 1 ? 'ready' : `status ${workspace.status}`}</span>
                  </article>
                ))}
              </div>
              {workspaces.available && !workspaces.workspaces.length ? <Empty>No registered workspaces found.</Empty> : null}
              {git?.available ? (
                <>
                  <h3>Git worktrees ({git.worktrees.length})</h3>
                  <p className="codex-hub-footnote">Repository: {git.root}</p>
                  <div className="codex-hub-rows">
                    {git.worktrees.map((tree, index) => (
                      <div key={`${tree.path}-${index}`} className="codex-hub-row">
                        <div><strong>{tree.branch || '(unknown branch)'}</strong><code>{tree.path}</code></div>
                        <span>{tree.head.slice(0, 12)}{tree.locked ? ' · Locked' : ''}{tree.prunable ? ' · Prunable' : ''}</span>
                      </div>
                    ))}
                  </div>
                  <h3>Working-tree changes (tracked files)</h3>
                  <div className="codex-hub-source"><pre>{git.status?.join('\n') || 'Clean'}</pre></div>
                </>
              ) : (
                <p className="codex-hub-footnote">{git?.reason ?? 'Open a conversation linked to a Git repository to inspect its worktrees.'}</p>
              )}
              <p className="codex-hub-footnote">Git results are read-only and session-rooted. Cross-workspace session binding is still unavailable.</p>
            </>
          ) : null}

          {page === 'files' ? (
            <>
              <PanelTitle title="Session files" description="Read-only explorer rooted at the current session directory. Secret files, symbolic links, binary files and oversized previews are refused."
                extra={<button className="codex-hub-button" type="button" onClick={reload}><RefreshCcw size={14} /> Refresh</button>} />
              {!sessionId ? <Empty>Select a conversation to inspect its working directory.</Empty> : null}
              {sessionId && file ? (
                <>
                  <div className="codex-hub-file-path">
                    <button type="button" disabled={filePath === '.'} onClick={() => {
                      const parent = filePath.includes('/') ? filePath.slice(0, filePath.lastIndexOf('/')) : '.'
                      setFilePath(parent || '.')
                    }} aria-label="Go to parent folder"><ChevronLeft size={15} /></button>
                    <code>{file.path === '.' ? 'Session root' : file.path}</code>
                    <span>Read-only</span>
                  </div>
                  {file.type === 'directory' ? (
                    <div className="codex-hub-files" aria-label="Directory files">
                      {file.entries.map((entry) => (
                        <button type="button" key={entry.path} onClick={() => setFilePath(entry.path)}>
                          {entry.is_dir ? <Folder size={16} aria-hidden /> : <FileText size={16} aria-hidden />}
                          <span>{entry.name}</span>
                          <small>{entry.is_dir ? 'Folder' : `${entry.bytes} B`}</small>
                        </button>
                      ))}
                      {file.truncated ? <p className="codex-hub-footnote">Directory listing truncated to native safety limit.</p> : null}
                      {!file.entries.length ? <Empty>Folder is empty or contains only protected files.</Empty> : null}
                    </div>
                  ) : (
                    <>
                      <div className="codex-hub-actions">
                        <button type="button" className="codex-hub-button" disabled={loadingDiff}
                          onClick={() => {
                            if (!sessionId) return
                            setLoadingDiff(true)
                            setError('')
                            void getSessionGitDiff(sessionId, file.path).then((result) => setGitDiff(result))
                              .catch((cause) => setError(cause instanceof Error ? cause.message : 'Git diff unavailable'))
                              .finally(() => setLoadingDiff(false))
                          }}>
                          <Code2 size={14} /> {loadingDiff ? 'Loading diff…' : 'Show Git diff'}
                        </button>
                      </div>
                      <div className="codex-hub-source"><pre>{file.text}</pre></div>
                      {gitDiff !== null ? (
                        <>
                          <h3>Unstaged Git diff</h3>
                          <div className="codex-hub-source" role="region" aria-label="Read-only Git diff">
                            <pre>{gitDiff || 'No tracked uncommitted changes for this file.'}</pre>
                          </div>
                        </>
                      ) : null}
                    </>
                  )}
                </>
              ) : null}
            </>
          ) : null}

          {page === 'jobs' ? (
            <>
              <PanelTitle title="Native shell" description="Run a command explicitly through the authenticated Rust permission broker. This is a foreground shell stream, not a durable background-job scheduler." />
              <form className="codex-hub-form" onSubmit={(event) => void runShell(event)}>
                <label htmlFor="hub-command">Command</label>
                <textarea id="hub-command" spellCheck={false} value={shellCommand} maxLength={32768}
                  onChange={(event) => setShellCommand(event.target.value)} rows={3}
                  placeholder="git status --short" disabled={!sessionId || shellRunning} />
                <div className="codex-hub-actions">
                  <button type="submit" disabled={!sessionId || !shellCommand.trim() || shellRunning} className="codex-hub-button primary">
                    <Terminal size={15} /> Run with native permissions
                  </button>
                  {shellRunning ? <button className="codex-hub-button" type="button" onClick={() => shellAbort.current?.abort()}><Square size={13} /> Stop</button> : null}
                </div>
              </form>
              {!sessionId ? <Empty>Open a conversation before starting a shell command.</Empty> : null}
              {shellLog ? <div className="codex-hub-source" aria-label="Live terminal output" role="log"><pre>{shellLog}</pre></div> : null}
              <p className="codex-hub-footnote">Tool approvals and destructive-command protections remain server-owned. Closing or stopping this stream cancels its current native run.</p>
            </>
          ) : null}

          {page === 'agents' ? (
            <>
              <PanelTitle title="Agents & delegation" description="Built-in and user-configured roles discovered from the daemon, with permissions enforced by the native orchestrator."
                extra={<button type="button" className="codex-hub-button" disabled={!sessionId} onClick={onOpenSubagents}><Bot size={15} /> Live subagents</button>} />
              {!inventory.agents.length ? <Empty>No agents advertised by this daemon. Open Settings to check capabilities.</Empty> : null}
              <div className="codex-hub-cards">
                {inventory.agents.map((agent) => (
                  <article key={agent.name} className="codex-hub-card">
                    <div className="codex-hub-card-head"><Bot size={18} /><strong>@{agent.name}</strong><span>{agent.read_only ? 'Read only' : 'Tool-enabled'}</span></div>
                    <p>{agent.description || 'No description provided'}</p>
                    <button className="codex-hub-button" type="button" onClick={() => {
                      void navigator.clipboard?.writeText(`@${agent.name} `).then(() => setNotice(`Copied @${agent.name} mention`))
                        .catch(() => setError('Clipboard write denied by browser'))
                    }}><Code2 size={14} /> Copy mention</button>
                  </article>
                ))}
              </div>
              <h3>Configured slash commands</h3>
              <div className="codex-hub-rows">
                {inventory.commands.map((command) => <div key={command.name} className="codex-hub-row"><code>/{command.name}</code><span>{command.description}</span></div>)}
                {!inventory.commands.length ? <Empty>No custom commands configured.</Empty> : null}
              </div>
            </>
          ) : null}

          {page === 'models' ? (
            <>
              <PanelTitle title="Available models" description="Native provider/model catalogue; selection changes the conversation composer." />
              <div className="codex-hub-rows">
                {models.map((model, index) => (
                  <div className="codex-hub-row" key={`${modelKey(model, index)}-${index}`}>
                    <strong>{modelLabel(model, index)}</strong><code>{modelKey(model, index)}</code>
                    <button type="button" className="codex-hub-button" onClick={() => { onChooseModel(modelKey(model, index)); onClose() }}>Use</button>
                  </div>
                ))}
                {!models.length ? <Empty>No models loaded. Check native provider configuration and sync the catalogue.</Empty> : null}
              </div>
            </>
          ) : null}

          {page === 'providers' ? (
            <>
              <PanelTitle title="Custom providers" description="Provider configuration persists natively. API keys are write-only and never returned to the browser." extra={
                <button type="button" className="codex-hub-button" onClick={reload}><RefreshCcw size={14} /> Refresh</button>
              } />
              <div className="codex-hub-rows">
                {providers.map((provider) => (
                  <div className="codex-hub-row" key={provider.id}>
                    <div><strong>{provider.name}</strong><code>{provider.id}</code></div>
                    <span>{provider.base_url}</span>
                    <button type="button" className="codex-hub-button" onClick={() => {
                      setProviderId(provider.id)
                      setProviderName(provider.name)
                      setProviderUrl(provider.base_url)
                      setProviderKey('')
                    }}>Edit</button>
                  </div>
                ))}
                {!providers.length ? <Empty>No custom providers yet; OpenAI may still be available through native environment credentials.</Empty> : null}
              </div>
              <h3>Add or update a provider</h3>
              <form className="codex-hub-form" onSubmit={submitProvider}>
                <label htmlFor="hub-provider-id">Provider ID</label>
                <input id="hub-provider-id" value={providerId} onChange={(event) => setProviderId(event.target.value)}
                  placeholder="my-provider" maxLength={63} required />
                <label htmlFor="hub-provider-name">Display name</label>
                <input id="hub-provider-name" value={providerName} onChange={(event) => setProviderName(event.target.value)}
                  placeholder="My OpenAI-compatible provider" required maxLength={120} />
                <label htmlFor="hub-provider-url">API base URL</label>
                <input id="hub-provider-url" type="url" value={providerUrl} onChange={(event) => setProviderUrl(event.target.value)} required />
                <label htmlFor="hub-provider-key">API key (optional; never shown again)</label>
                <input id="hub-provider-key" type="password" autoComplete="off" value={providerKey} onChange={(event) => setProviderKey(event.target.value)} maxLength={16384} />
                <button className="codex-hub-button primary" disabled={busy} type="submit"><KeyRound size={15} /> Save provider natively</button>
              </form>
            </>
          ) : null}

          {page === 'mcp' ? (
            <>
              <PanelTitle title="MCP servers" description="Native daemon lifecycle and discovered tools. Connecting starts the configured local MCP process."
                extra={<button type="button" className="codex-hub-button" onClick={reload}><RefreshCcw size={14} /> Refresh</button>} />
              <div className="codex-hub-cards">
                {Object.entries(mcp).map(([name, state]) => (
                  <article className="codex-hub-card" key={name}>
                    <div className="codex-hub-card-head"><PlugZap size={18} /><strong>{name}</strong><span>{state.status}</span></div>
                    {state.error ? <p>{state.error}</p> : null}
                    <p>{state.tools?.length ?? 0} tools available</p>
                    <div className="codex-hub-actions">
                      {state.status === 'connected' ? (
                        <button type="button" className="codex-hub-button" disabled={busy}
                          onClick={() => void mutate(() => setMcpConnection(name, false), `Disconnected ${name}`)}>Disconnect</button>
                      ) : confirmConnect === name ? (
                        <>
                          <button type="button" className="codex-hub-button danger" disabled={busy}
                            onClick={() => { setConfirmConnect(null); void mutate(() => setMcpConnection(name, true), `Connected ${name}`) }}>Confirm launch</button>
                          <button type="button" className="codex-hub-button" onClick={() => setConfirmConnect(null)}>Cancel</button>
                        </>
                      ) : (
                        <button type="button" className="codex-hub-button" onClick={() => setConfirmConnect(name)}>Connect</button>
                      )}
                    </div>
                    {confirmConnect === name ? <p className="codex-hub-warning">This runs a native local MCP process. Connect only to a configuration you trust.</p> : null}
                  </article>
                ))}
              </div>
              {!Object.keys(mcp).length ? <Empty>No MCP servers registered in this daemon.</Empty> : null}
              <h3>Add a disabled local server</h3>
              <form className="codex-hub-form" onSubmit={submitMcp}>
                <label htmlFor="hub-mcp-name">Server ID</label>
                <input id="hub-mcp-name" value={mcpName} maxLength={64} required onChange={(event) => setMcpName(event.target.value)} placeholder="filesystem" />
                <label htmlFor="hub-mcp-args">Command argv (JSON string array)</label>
                <textarea id="hub-mcp-args" value={mcpArgs} onChange={(event) => setMcpArgs(event.target.value)} rows={3} spellCheck={false} />
                <button type="submit" className="codex-hub-button primary" disabled={busy}>Register disabled</button>
              </form>
              <p className="codex-hub-footnote">Remote MCP transport, credential management and guaranteed restart persistence depend on native backend support.</p>
            </>
          ) : null}

          {page === 'permissions' ? (
            <>
              <PanelTitle title="Permission rules" description="Native human approval preferences; deny rules take precedence. A saved rule does not bypass the Rust execution broker."
                extra={<button type="button" className="codex-hub-button" onClick={reload}><RefreshCcw size={14} /> Refresh</button>} />
              <div className="codex-hub-rows">
                {rules.map((rule, i) => (
                  <div key={`${rule.tool}-${rule.scope}-${i}`} className="codex-hub-row">
                    <strong>{rule.tool}</strong><span>{rule.scope} · {rule.decision}</span>
                  </div>
                ))}
                {!rules.length ? <Empty>No custom rules. Native approvals remain in effect.</Empty> : null}
              </div>
              <h3>Set a rule</h3>
              <form className="codex-hub-form" onSubmit={submitPermission}>
                <label htmlFor="hub-permission-tool">Tool</label>
                <select id="hub-permission-tool" value={permissionTool} onChange={(event) => setPermissionTool(event.target.value)}>
                  {['bash', 'shell', 'read', 'write', 'edit', 'glob', 'grep', 'webfetch', 'websearch', 'task', 'todowrite'].map((tool) => <option key={tool}>{tool}</option>)}
                </select>
                <label htmlFor="hub-permission-decision">Decision</label>
                <select id="hub-permission-decision" value={permissionDecision} onChange={(event) => {
                  setPermissionDecision(event.target.value as 'ask' | 'allow' | 'deny')
                  setAcknowledgeAllow(false)
                }}>
                  <option value="ask">Ask each time (reset)</option><option value="deny">Deny</option><option value="allow">Allow</option>
                </select>
                <label htmlFor="hub-permission-scope">Scope</label>
                <select id="hub-permission-scope" value={permissionScope} onChange={(event) => setPermissionScope(event.target.value as 'session' | 'always')}>
                  <option value="session" disabled={!sessionId}>This session</option>
                  <option value="always">Across sessions</option>
                </select>
                {permissionDecision === 'allow' ? (
                  <label className="codex-hub-acknowledge">
                    <input type="checkbox" checked={acknowledgeAllow} onChange={(event) => setAcknowledgeAllow(event.target.checked)} />
                    I understand this allows the selected native tool within the selected scope.
                  </label>
                ) : null}
                <button type="submit" disabled={busy || (permissionScope === 'session' && !sessionId)}
                  className="codex-hub-button primary"><ShieldCheck size={14} /> Save rule</button>
              </form>
            </>
          ) : null}

          {page === 'tasks' ? (
            <>
              <PanelTitle title="Conversation tasks" description="Durable native TODOs for the selected session, including status and priority."
                extra={<button type="button" className="codex-hub-button" onClick={reload}><RefreshCcw size={14} /> Refresh</button>} />
              {!sessionId ? <Empty>Open a conversation to manage its tasks.</Empty> : (
                <>
                  <form className="codex-hub-inline-form" onSubmit={(event) => {
                    event.preventDefault()
                    if (!newTodo.trim()) return
                    const id = typeof crypto.randomUUID === 'function' ? crypto.randomUUID() : `task-${Date.now()}`
                    changeTodos([...todos, { id, content: newTodo.trim(), status: 'pending', priority: 'medium' }])
                    setNewTodo('')
                  }}>
                    <label htmlFor="hub-new-todo" className="sr-only">New task</label>
                    <input id="hub-new-todo" value={newTodo} maxLength={4096}
                      onChange={(event) => setNewTodo(event.target.value)} placeholder="Add a task..." />
                    <button type="submit" disabled={busy || !newTodo.trim()} className="codex-hub-button primary">Add task</button>
                  </form>
                  <div className="codex-hub-todos">
                    {todos.map((todo) => (
                      <div className="codex-hub-todo" key={todo.id}>
                        <span>{todo.content}</span>
                        <select aria-label={`Status for ${todo.content}`} value={todo.status} disabled={busy}
                          onChange={(event) => changeTodos(todos.map((item) => item.id === todo.id
                            ? { ...item, status: event.target.value as NativeTodo['status'] } : item))}>
                          <option value="pending">Pending</option><option value="in_progress">In progress</option>
                          <option value="completed">Completed</option><option value="cancelled">Cancelled</option>
                        </select>
                        <select aria-label={`Priority for ${todo.content}`} value={todo.priority} disabled={busy}
                          onChange={(event) => changeTodos(todos.map((item) => item.id === todo.id
                            ? { ...item, priority: event.target.value as NativeTodo['priority'] } : item))}>
                          <option value="low">Low</option><option value="medium">Medium</option><option value="high">High</option>
                        </select>
                        <button type="button" aria-label={`Delete ${todo.content}`} className="codex-panel-icon"
                          disabled={busy} onClick={() => changeTodos(todos.filter((item) => item.id !== todo.id))}><X size={15} /></button>
                      </div>
                    ))}
                    {!todos.length ? <Empty>No tasks recorded for this session.</Empty> : null}
                  </div>
                </>
              )}
            </>
          ) : null}
        </div>
      </div>
    </section>
  )
}
