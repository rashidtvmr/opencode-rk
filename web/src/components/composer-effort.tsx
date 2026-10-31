import type { ModelSummary } from '@/lib/api'

/// Client mirror of the server turn contract (`lib.rs:766-771,895-900` and
/// `chat_composer.rs:24-25`): six efforts `none|minimal|low|medium|high|xhigh`.
/// The composer menu (`composer.tsx:47-52`) only lists four; this module adds
/// `none`/`minimal` plus the non-openai provider hint (`lib.rs:761-765`).
export const ALL_EFFORTS = ['none', 'minimal', 'low', 'medium', 'high', 'xhigh'] as const

export type ComposerEffortValue = (typeof ALL_EFFORTS)[number]

export const EFFORT_LEVELS: ReadonlyArray<{ value: ComposerEffortValue; label: string }> = [
  { value: 'none', label: 'None' },
  { value: 'minimal', label: 'Minimal' },
  { value: 'low', label: 'Low' },
  { value: 'medium', label: 'Medium' },
  { value: 'high', label: 'High' },
  { value: 'xhigh', label: 'Extra High' },
]

export const COMPOSER_EFFORT_EMPTY_LABEL = 'No models reported for effort selection'

/// The only provider with a native turn adapter (`lib.rs:761-765,890-894`).
export const SUPPORTED_TURN_PROVIDER = 'openai'

/// Hint for the selected model. Null when the turn contract accepts it;
/// otherwise the server's rejection reason, verbatim, so the UI never invents
/// a contract. Unknown providers and malformed selectors warn; nothing sends.
export function providerHint(selectedModel: string): string | null {
  const trimmed = selectedModel.trim()
  const [provider, model] = trimmed.split('/')
  if (!trimmed || provider === undefined || model === undefined || !provider || !model) {
    return 'model must use provider/model format'
  }
  if (provider === SUPPORTED_TURN_PROVIDER) return null
  // ponytail: allowlist fixed at openai; extend when the server turn adapter lands.
  return `provider '${provider}' does not have a native turn adapter yet`
}

interface ComposerEffortProps {
  models: ModelSummary[]
  selectedModel: string
  effort: string
  onEffortChange: (effort: ComposerEffortValue) => void
}

export function ComposerEffort({
  models,
  selectedModel,
  effort,
  onEffortChange,
}: ComposerEffortProps) {
  if (models.length === 0) {
    return <p className="codex-composer-effort-empty">{COMPOSER_EFFORT_EMPTY_LABEL}</p>
  }

  const hint = providerHint(selectedModel)

  const select = (value: ComposerEffortValue) => {
    onEffortChange(value)
  }

  return (
    <div className="codex-composer-effort-hint">
      <ul role="listbox" aria-label="Reasoning effort" className="codex-composer-effort-list">
        {EFFORT_LEVELS.map((level) => (
          <li
            key={level.value}
            role="option"
            aria-selected={effort === level.value}
            tabIndex={0}
            onClick={() => select(level.value)}
            onKeyDown={(event) => {
              if (event.key === 'Enter' || event.key === ' ') {
                event.preventDefault()
                select(level.value)
              }
            }}
          >
            {level.label}
          </li>
        ))}
      </ul>
      {hint ? <p role="status">{hint}</p> : null}
    </div>
  )
}
