import { AlertCircle, Bot, CheckCircle2, Download, FileDiff, LoaderCircle, Plug, Search, Terminal, Wrench } from 'lucide-react'
import type { MessageRecord } from '@/lib/api'

export interface ToolExecutionItem {
  id: string
  name: string
  arguments: string
  output?: string
  state: 'running' | 'completed' | 'failed'
}

function kind(name: string) {
  if (/^(?:mcp[_:.])|(?:^mcp$)/i.test(name)) return 'mcp'
  if (/(?:shell|bash|exec|terminal|command)/i.test(name)) return 'shell'
  if (/(?:patch|edit|write|diff|git)/i.test(name)) return 'patch'
  if (/(?:grep|glob|search|find)/i.test(name)) return 'search'
  if (/(?:task|subagent|delegate)/i.test(name)) return 'agent'
  return 'tool'
}

function DiffOutput({ text }: { text: string }) {
  if (text.length > 64_000) return <pre>{text}</pre>
  const lines = text.split('\n')
  return <pre>{lines.map((line, index) => <span key={index} className={
    line.startsWith('+') && !line.startsWith('+++') ? 'codex-diff-add'
      : line.startsWith('-') && !line.startsWith('---') ? 'codex-diff-remove'
        : line.startsWith('@@') ? 'codex-diff-hunk' : undefined
  }>{line}{index < lines.length - 1 ? '\n' : ''}</span>)}</pre>
}

function downloadToolLog(name: string, contents: string) {
  if (typeof URL.createObjectURL !== 'function') return
  const url = URL.createObjectURL(new Blob([contents], { type: 'text/plain;charset=utf-8' }))
  const link = document.createElement('a')
  link.href = url
  link.download = `${name.replace(/[^a-z0-9_-]/gi, '_').slice(0, 64) || 'tool'}.log`
  link.click()
  window.setTimeout(() => URL.revokeObjectURL(url), 1000)
}

/** Combine durable typed records into the same cards used by the live stream. */
export function deriveStoredToolTimeline(messages: MessageRecord[]): Map<string, ToolExecutionItem> {
  const items = new Map<string, ToolExecutionItem>()
  for (const message of messages) {
    for (const part of message.parts ?? []) {
      const previous = items.get(part.call_id)
      if (part.type === 'tool_call') {
        items.set(part.call_id, {
          id: part.call_id,
          name: part.name,
          arguments: part.arguments ?? '',
          output: previous?.output,
          state: previous?.output !== undefined
            ? previous.state : part.status === 'completed' ? 'completed' : 'running',
        })
      } else {
        items.set(part.call_id, {
          id: part.call_id,
          name: previous?.name ?? part.name,
          arguments: previous?.arguments ?? '',
          output: part.output ?? '',
          state: part.success === false ? 'failed' : 'completed',
        })
      }
    }
  }
  return items
}

interface ToolCollapseProps {
  expandedByDefault?: boolean
  onExpandedChange?: (expanded: boolean) => void
}

export function ToolTimeline({ tools, expandedByDefault = false, onExpandedChange }: {
  tools: ToolExecutionItem[]
} & ToolCollapseProps) {
  if (!tools.length) return null
  return (
    <section className="codex-tool-timeline" aria-label="Tool execution">
      {tools.map((tool) => {
        const category = kind(tool.name)
        const shell = category === 'shell'
        const Symbol = shell ? Terminal : category === 'patch' ? FileDiff
          : category === 'search' ? Search : category === 'agent' ? Bot
            : category === 'mcp' ? Plug : Wrench
        return (
          <details key={`${tool.id}:${expandedByDefault}`} className="codex-tool-card"
            open={expandedByDefault}
            onToggle={(event) => {
              const open = event.currentTarget.open
              if (open !== expandedByDefault) onExpandedChange?.(open)
            }}>
            <summary>
              {tool.state === 'running'
                ? <LoaderCircle className="codex-spin" size={15} aria-hidden />
                : tool.state === 'failed'
                  ? <AlertCircle size={15} aria-hidden />
                  : <CheckCircle2 size={15} aria-hidden />}
              <Symbol size={15} aria-hidden />
              <strong>{tool.name}</strong>
              <small>{category === 'shell' ? 'Shell' : category === 'patch' ? 'Files' : category === 'search' ? 'Search'
                : category === 'agent' ? 'Agent' : category === 'mcp' ? 'MCP' : 'Tool'}</small>
              <small>{tool.state === 'running' ? 'Running' : tool.state === 'failed' ? 'Failed' : 'Completed'}</small>
            </summary>
            {tool.arguments ? (
              <div className="codex-tool-section"><h4>Arguments</h4><pre>{tool.arguments}</pre></div>
            ) : null}
            {tool.output !== undefined ? (
              <div className="codex-tool-section">
                <div className="codex-tool-section-heading">
                  <h4>{shell ? 'Terminal output' : category === 'patch' ? 'File change' : 'Result'}</h4>
                  <button type="button" onClick={() => downloadToolLog(tool.name, tool.output ?? '')}
                    aria-label={`Download ${tool.name} output`}><Download size={13} aria-hidden /> Download log</button>
                </div>
                {category === 'patch' || /^diff --git/m.test(tool.output)
                  ? <DiffOutput text={tool.output || '(no output)'} />
                  : <pre>{tool.output || '(no output)'}</pre>}
              </div>
            ) : null}
          </details>
        )
      })}
    </section>
  )
}

export function PersistedToolMessage({ text, expandedByDefault = false, onExpandedChange }: {
  text: string
} & ToolCollapseProps) {
  return (
    <details key={String(expandedByDefault)} className="codex-tool-card codex-persisted-tool"
      open={expandedByDefault}
      onToggle={(event) => {
        const open = event.currentTarget.open
        if (open !== expandedByDefault) onExpandedChange?.(open)
      }}>
      <summary><Terminal size={15} aria-hidden /><strong>Tool output</strong><small>Saved in conversation</small></summary>
      <div className="codex-tool-section"><pre>{text}</pre></div>
    </details>
  )
}
