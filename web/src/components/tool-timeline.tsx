import { CheckCircle2, LoaderCircle, Terminal, Wrench } from 'lucide-react'

export interface ToolExecutionItem {
  id: string
  name: string
  arguments: string
  output?: string
  state: 'running' | 'completed'
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
              {tool.state === 'running' ? <LoaderCircle className="codex-spin" size={15} aria-hidden /> : <CheckCircle2 size={15} aria-hidden />}
              {shell ? <Terminal size={15} aria-hidden /> : <Wrench size={15} aria-hidden />}
              <strong>{tool.name}</strong>
              <small>{tool.state === 'running' ? 'Running' : 'Completed'}</small>
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
