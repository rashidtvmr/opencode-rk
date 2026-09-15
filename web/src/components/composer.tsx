import { useMemo, useState, type ChangeEvent, type KeyboardEvent } from 'react'
import {
  ArrowUp,
  ImagePlus,
  Mic,
  Paperclip,
  Plus,
  SlidersHorizontal,
} from 'lucide-react'

import { Button } from '@/components/ui/button'
import {
  DropdownMenu,
  DropdownMenuItem,
  DropdownMenuLabel,
  DropdownMenuSeparator,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu'
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select'
import { Textarea } from '@/components/ui/textarea'
import { modelKey, modelLabel, type ModelSummary } from '@/lib/api'

const effortLevels = ['Low', 'Medium', 'High', 'Extra High'] as const
type EffortLevel = (typeof effortLevels)[number]

interface ComposerProps {
  disabled?: boolean
  models: ModelSummary[]
  selectedModel: string
  onModelChange: (model: string) => void
  onSubmit?: (value: string) => boolean | void | Promise<boolean | void>
}

export function Composer({
  disabled = false,
  models,
  selectedModel,
  onModelChange,
  onSubmit,
}: ComposerProps) {
  const [value, setValue] = useState('')
  const [effort, setEffort] = useState<EffortLevel>('High')
  const [submitting, setSubmitting] = useState(false)

  const modelItems = useMemo(
    () =>
      models.map((model, index) => ({
        id: modelKey(model, index),
        label: modelLabel(model, index),
      })),
    [models],
  )

  const submit = async () => {
    const message = value.trim()
    if (!message || disabled || submitting) return

    setSubmitting(true)
    try {
      const accepted = await onSubmit?.(message)
      if (accepted !== false) setValue('')
    } finally {
      setSubmitting(false)
    }
  }

  const onKeyDown = (event: KeyboardEvent<HTMLTextAreaElement>) => {
    if (event.key === 'Enter' && !event.shiftKey && !event.nativeEvent.isComposing) {
      event.preventDefault()
      void submit()
    }
  }

  return (
    <div className="codex-composer" data-testid="codex-composer">
      <label className="sr-only" htmlFor="message-composer">
        Message
      </label>
      <Textarea
        id="message-composer"
        className="codex-composer-input"
        value={value}
        onChange={(event: ChangeEvent<HTMLTextAreaElement>) => setValue(event.target.value)}
        onKeyDown={onKeyDown}
        rows={3}
        disabled={disabled}
        aria-busy={submitting || undefined}
        placeholder={disabled ? 'Select or create a chat to start' : 'Ask anything'}
        aria-describedby="composer-help"
      />

      <div className="codex-composer-toolbar">
        <div className="codex-composer-tools">
          <DropdownMenuTrigger>
            <Button
              size="icon"
              variant="ghost"
              className="codex-composer-icon-button"
              aria-label="Add attachment"
            >
              <Plus aria-hidden="true" />
            </Button>
            <DropdownMenu placement="top start" className="w-56">
              <DropdownMenuLabel>Add to your message</DropdownMenuLabel>
              <DropdownMenuItem isDisabled>
                <Paperclip aria-hidden="true" />
                Add photos &amp; files
              </DropdownMenuItem>
              <DropdownMenuItem isDisabled>
                <ImagePlus aria-hidden="true" />
                Take screenshot
              </DropdownMenuItem>
            </DropdownMenu>
          </DropdownMenuTrigger>

          <Select
            selectedKey={selectedModel || null}
            onSelectionChange={(key) => {
              if (key != null) onModelChange(String(key))
            }}
            isDisabled={modelItems.length === 0}
            aria-label="Model"
          >
            <SelectTrigger className="codex-composer-select" size="sm">
              <SelectValue>
                {modelItems.find((item) => item.id === selectedModel)?.label ?? 'No models'}
              </SelectValue>
            </SelectTrigger>
            <SelectContent placement="top start" className="min-w-64">
              {modelItems.map((item) => (
                <SelectItem key={item.id} id={item.id}>
                  {item.label}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>

          <DropdownMenuTrigger>
            <Button
              size="sm"
              variant="ghost"
              className="codex-composer-effort"
              aria-label={`Reasoning effort: ${effort}`}
            >
              <SlidersHorizontal aria-hidden="true" className="size-3.5" />
              {effort}
            </Button>
            <DropdownMenu placement="top start" selectionMode="single" selectedKeys={[effort]}>
              <DropdownMenuLabel>Reasoning effort</DropdownMenuLabel>
              <DropdownMenuSeparator />
              {effortLevels.map((level) => (
                <DropdownMenuItem
                  key={level}
                  id={level}
                  onAction={() => setEffort(level)}
                >
                  {level}
                </DropdownMenuItem>
              ))}
            </DropdownMenu>
          </DropdownMenuTrigger>
        </div>

        <div className="codex-composer-actions">
          <p id="composer-help" className="sr-only">
            Enter to send. Shift plus Enter inserts a new line.
          </p>

          <Button
            size="icon"
            variant="ghost"
            className="codex-composer-icon-button"
            aria-label="Voice input unavailable"
            isDisabled
          >
            <Mic aria-hidden="true" />
          </Button>

          <Button
            size="icon"
            className="codex-send-button"
            aria-label="Send message"
            onPress={() => void submit()}
            isDisabled={disabled || submitting || value.trim().length === 0}
          >
            <ArrowUp aria-hidden="true" />
          </Button>
        </div>
      </div>
    </div>
  )
}
