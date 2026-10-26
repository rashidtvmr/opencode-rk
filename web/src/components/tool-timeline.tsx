import { AlertCircle, CheckCircle2, LoaderCircle, Terminal, Wrench } from 'lucide-react'
import type { MessageRecord } from '@/lib/api'

export interface ToolExecutionItem {
  id: string
  name: string
  arguments: string
  output?: string
  state: 'running' | 'completed' | 'failed'
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

export function ToolTimeline({ tools }: { tools: ToolExecutionItem[] }) {
  if (!tools.length) return null
  return (
    <section className="codex-tool-timeline" aria-label="Tool execution">
      {tools.map((tool) => {
        const shell = /(?:shell|bash|exec|terminal|command)/i.test(tool.name)
        return (
          <details key={tool.id} className="codex-tool-card">
            <summary>
              {tool.state === 'running'
                ? <LoaderCircle className="codex-spin" size={15} aria-hidden />
                : tool.state === 'failed'
                  ? <AlertCircle size={15} aria-hidden />
                  : <CheckCircle2 size={15} aria-hidden />}
              {shell ? <Terminal size={15} aria-hidden /> : <Wrench size={15} aria-hidden />}
              <strong>{tool.name}</strong>
              <small>{tool.state === 'running' ? 'Running' : tool.state === 'failed' ? 'Failed' : 'Completed'}</small>
            </summary>
            {tool.arguments ? (
              <div className="codex-tool-section"><h4>Arguments</h4><pre>{tool.arguments}</pre></div>
            ) : null}
            {tool.output !== undefined ? (
              <div className="codex-tool-section"><h4>{shell ? 'Terminal output' : 'Result'}</h4><pre>{tool.output || '(no output)'}</pre></div>
            ) : null}
          </details>
        )
      })}
    </section>
  )
}

export function PersistedToolMessage({ text }: { text: string }) {
  return (
    <details className="codex-tool-card codex-persisted-tool">
      <summary><Terminal size={15} aria-hidden /><strong>Tool output</strong><small>Saved in conversation</small></summary>
      <div className="codex-tool-section"><pre>{text}</pre></div>
    </details>
  )
}
