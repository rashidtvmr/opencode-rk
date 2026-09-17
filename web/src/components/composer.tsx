import {
  useEffect,
  useMemo,
  useRef,
  useState,
  type ClipboardEvent,
  type KeyboardEvent,
} from 'react'
import {
  ArrowUp,
  Code2,
  Command,
  ImagePlus,
  Mic,
  Paperclip,
  Plus,
  SlidersHorizontal,
  Square,
  Search,
  X,
  Wrench,
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
import {
  modelKey,
  modelLabel,
  type DraftAttachment,
  type ModelSummary,
  type WebCapabilities,
} from '@/lib/api'

const effortLevels = [
  { value: 'low', label: 'Low' },
  { value: 'medium', label: 'Medium' },
  { value: 'high', label: 'High' },
  { value: 'xhigh', label: 'Extra High' },
] as const
export type ReasoningEffort = (typeof effortLevels)[number]['value']

export const MAX_COMPOSER_DRAFT_BYTES = 32_768
const DRAFT_STORAGE_PREFIX = 'opencode-rk:web-draft:v1:'

type ComposerBlock =
  | { type: 'paragraph'; text: string }
  | { type: 'code'; text: string }

interface StoredDraft {
  version: 1
  blocks: ComposerBlock[]
}

interface ComposerProps {
  disabled?: boolean
  sessionId?: string | null
  running?: boolean
  attachments?: DraftAttachment[]
  attachmentsAvailable?: boolean
  attachmentUnavailableReason?: string
  capabilities?: WebCapabilities | null
  models: ModelSummary[]
  selectedModel: string
  onModelChange: (model: string) => void
  onReasoningEffortChange?: (effort: ReasoningEffort) => void
  onFilesSelected?: (files: File[]) => void
  onRemoveAttachment?: (attachmentId: string) => void
  onStop?: () => void
  onSubmit?: (
    value: string,
    reasoningEffort: ReasoningEffort,
  ) => boolean | void | Promise<boolean | void>
}

function escapeHtml(value: string) {
  return value
    .replaceAll('&', '&amp;')
    .replaceAll('<', '&lt;')
    .replaceAll('>', '&gt;')
    .replaceAll('"', '&quot;')
}

function blocksToHtml(blocks: ComposerBlock[]) {
  return blocks
    .map((block) => {
      const text = escapeHtml(block.text).replaceAll('\n', '<br>') || '<br>'
      return block.type === 'code' ? `<pre>${text}</pre>` : `<p>${text}</p>`
    })
    .join('')
}

function editorBlocks(element: HTMLElement): ComposerBlock[] {
  const blocks: ComposerBlock[] = []
  for (const node of Array.from(element.childNodes)) {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent ?? ''
      if (text) blocks.push({ type: 'paragraph', text })
      continue
    }
    if (!(node instanceof HTMLElement)) continue
    // innerText requires layout; detached nodes (jsdom) do not have it.
    const text = (node.innerText ?? node.textContent ?? '').replace(/\u200b/g, '')
    blocks.push({ type: node.tagName === 'PRE' ? 'code' : 'paragraph', text })
  }
  if (blocks.length === 0 && element.innerText) {
    blocks.push({ type: 'paragraph', text: element.innerText })
  }
  return blocks
}

export function lowerComposerDocument(blocks: ComposerBlock[]) {
  return blocks
    .map((block) =>
      block.type === 'code' ? `\`\`\`\n${block.text}\n\`\`\`` : block.text,
    )
    .join('\n\n')
    .trim()
}

function draftBytes(value: string) {
  return new TextEncoder().encode(value).byteLength
}

function loadDraft(sessionId: string): ComposerBlock[] {
  try {
    const raw = localStorage.getItem(`${DRAFT_STORAGE_PREFIX}${sessionId}`)
    if (!raw) return []
    const parsed = JSON.parse(raw) as Partial<StoredDraft>
    if (parsed.version !== 1 || !Array.isArray(parsed.blocks)) return []
    const blocks = parsed.blocks.filter(
      (block): block is ComposerBlock =>
        Boolean(block) &&
        (block.type === 'paragraph' || block.type === 'code') &&
        typeof block.text === 'string',
    )
    const lowered = lowerComposerDocument(blocks)
    return draftBytes(lowered) <= MAX_COMPOSER_DRAFT_BYTES ? blocks : []
  } catch {
    return []
  }
}

function persistDraft(sessionId: string, blocks: ComposerBlock[]) {
  try {
    if (blocks.length === 0 || !lowerComposerDocument(blocks)) {
      localStorage.removeItem(`${DRAFT_STORAGE_PREFIX}${sessionId}`)
      return
    }
    const draft: StoredDraft = { version: 1, blocks }
    localStorage.setItem(`${DRAFT_STORAGE_PREFIX}${sessionId}`, JSON.stringify(draft))
  } catch {
    // Browser storage is an optional draft convenience, never the message authority.
  }
}

function selectionInsideCode(editor: HTMLElement) {
  const selection = window.getSelection()
  const node = selection?.anchorNode
  if (!node || !editor.contains(node)) return false
  const element = node.nodeType === Node.ELEMENT_NODE ? (node as Element) : node.parentElement
  return Boolean(element?.closest('pre'))
}

export function Composer({
  disabled = false,
  sessionId = null,
  running = false,
  attachments = [],
  attachmentsAvailable = true,
  attachmentUnavailableReason,
  capabilities = null,
  models,
  selectedModel,
  onModelChange,
  onReasoningEffortChange,
  onFilesSelected,
  onRemoveAttachment,
  onStop,
  onSubmit,
}: ComposerProps) {
  const editorRef = useRef<HTMLDivElement>(null)
  const fileInputRef = useRef<HTMLInputElement>(null)
  const lastAcceptedHtmlRef = useRef('')
  const [draftText, setDraftText] = useState('')
  const [draftError, setDraftError] = useState('')
  const [effort, setEffort] = useState<ReasoningEffort>('high')
  const [submitting, setSubmitting] = useState(false)

  const modelItems = useMemo(
    () =>
      models.map((model, index) => ({
        id: modelKey(model, index),
        label: modelLabel(model, index),
      })),
    [models],
  )

  useEffect(() => {
    const editor = editorRef.current
    if (!editor) return
    const blocks = sessionId ? loadDraft(sessionId) : []
    editor.innerHTML = blocksToHtml(blocks)
    lastAcceptedHtmlRef.current = editor.innerHTML
    setDraftText(lowerComposerDocument(blocks))
    setDraftError('')
  }, [sessionId])

  const syncDraft = () => {
    const editor = editorRef.current
    if (!editor) return
    const blocks = editorBlocks(editor)
    const lowered = lowerComposerDocument(blocks)
    const bytes = draftBytes(lowered)
    if (bytes > MAX_COMPOSER_DRAFT_BYTES) {
      editor.innerHTML = lastAcceptedHtmlRef.current
      setDraftError(`Draft exceeds the ${MAX_COMPOSER_DRAFT_BYTES.toLocaleString()} byte limit.`)
      return
    }
    lastAcceptedHtmlRef.current = editor.innerHTML
    setDraftText(lowered)
    setDraftError('')
    if (sessionId) persistDraft(sessionId, blocks)
  }

  const clearDraft = () => {
    if (editorRef.current) editorRef.current.innerHTML = ''
    lastAcceptedHtmlRef.current = ''
    setDraftText('')
    setDraftError('')
    if (sessionId) localStorage.removeItem(`${DRAFT_STORAGE_PREFIX}${sessionId}`)
  }

  const submit = async () => {
    const editor = editorRef.current
    if (
      !editor ||
      disabled ||
      running ||
      submitting ||
      draftError ||
      attachments.length > 0
    ) return
    const blocks = editorBlocks(editor)
    const message = lowerComposerDocument(blocks)
    if (!message) return

    setSubmitting(true)
    try {
      const accepted = await onSubmit?.(message, effort)
      if (accepted !== false) clearDraft()
    } finally {
      setSubmitting(false)
    }
  }

  const onKeyDown = (event: KeyboardEvent<HTMLDivElement>) => {
    if (
      event.key === 'Enter' &&
      !event.shiftKey &&
      !event.nativeEvent.isComposing &&
      editorRef.current &&
      !selectionInsideCode(editorRef.current)
    ) {
      event.preventDefault()
      void submit()
    }
  }

  const onPaste = (event: ClipboardEvent<HTMLDivElement>) => {
    const editor = editorRef.current
    if (!editor) return
    event.preventDefault()
    const text = event.clipboardData.getData('text/plain')
    if (draftBytes(`${draftText}${text}`) > MAX_COMPOSER_DRAFT_BYTES) {
      setDraftError(`Paste would exceed the ${MAX_COMPOSER_DRAFT_BYTES.toLocaleString()} byte limit.`)
      return
    }
    const selection = window.getSelection()
    if (selection && selection.rangeCount > 0 && editor.contains(selection.anchorNode)) {
      const range = selection.getRangeAt(0)
      range.deleteContents()
      const node = document.createTextNode(text)
      range.insertNode(node)
      range.setStartAfter(node)
      range.collapse(true)
      selection.removeAllRanges()
      selection.addRange(range)
    } else {
      editor.append(document.createTextNode(text))
    }
    syncDraft()
  }

  const insertCodeBlock = () => {
    const editor = editorRef.current
    if (!editor || disabled || running) return
    const pre = document.createElement('pre')
    pre.append(document.createElement('br'))
    editor.append(pre)
    editor.append(document.createElement('p'))
    editor.focus()
    const range = document.createRange()
    range.selectNodeContents(pre)
    range.collapse(true)
    const selection = window.getSelection()
    selection?.removeAllRanges()
    selection?.addRange(range)
    syncDraft()
  }

  return (
    <div className="codex-composer" data-testid="codex-composer">
      <input
        ref={fileInputRef}
        className="sr-only"
        type="file"
        multiple
        tabIndex={-1}
        aria-hidden="true"
        onChange={(event) => {
          const files = Array.from(event.currentTarget.files ?? [])
          event.currentTarget.value = ''
          if (files.length > 0) onFilesSelected?.(files)
        }}
      />
      <label className="sr-only" id="message-composer-label">
        Message
      </label>
      <div
        ref={editorRef}
        id="message-composer"
        className="codex-composer-input codex-structured-editor"
        role="textbox"
        aria-labelledby="message-composer-label"
        aria-multiline="true"
        aria-busy={submitting || running || undefined}
        aria-describedby="composer-help composer-draft-status"
        contentEditable={!disabled && !running}
        suppressContentEditableWarning
        data-placeholder={disabled ? 'Select or create a chat to start' : 'Ask anything'}
        onInput={syncDraft}
        onKeyDown={onKeyDown}
        onPaste={onPaste}
      />

      {attachments.length > 0 ? (
        <div className="codex-attachment-area">
          <ul className="codex-attachment-list" aria-label="Draft attachments">
            {attachments.map((attachment) => (
              <li key={attachment.id} className="codex-attachment-chip">
                <Paperclip aria-hidden="true" />
                <span className="codex-attachment-name">{attachment.name}</span>
                <span className="codex-attachment-size">
                  {Math.max(1, Math.ceil(attachment.bytes / 1024))} KiB
                </span>
                <Button
                  size="icon-xs"
                  variant="ghost"
                  aria-label={`Remove ${attachment.name}`}
                  isDisabled={running}
                  onPress={() => onRemoveAttachment?.(attachment.id)}
                >
                  <X aria-hidden="true" />
                </Button>
              </li>
            ))}
          </ul>
          <p className="codex-attachment-warning" role="status">
            Files are stored with this draft. Remove them before sending; the current provider adapter cannot transmit attachments yet.
          </p>
        </div>
      ) : null}

      <div className="codex-composer-toolbar">
        <div className="codex-composer-tools">
          <DropdownMenuTrigger>
            <Button
              size="icon"
              variant="ghost"
              className="codex-composer-icon-button"
              aria-label="Add to message"
              isDisabled={disabled || running}
            >
              <Plus aria-hidden="true" />
            </Button>
            <DropdownMenu placement="top start" className="w-64" isNonModal>
              <DropdownMenuLabel>Add to your message</DropdownMenuLabel>
              <DropdownMenuItem onAction={insertCodeBlock}>
                <Code2 aria-hidden="true" />
                Code block
              </DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem
                isDisabled={!attachmentsAvailable || running}
                onAction={() => fileInputRef.current?.click()}
              >
                <Paperclip aria-hidden="true" />
                {attachmentsAvailable ? 'Add files' : 'Files unavailable for this chat'}
              </DropdownMenuItem>
              <DropdownMenuItem isDisabled>
                <ImagePlus aria-hidden="true" />
                Screenshot unavailable
              </DropdownMenuItem>
            </DropdownMenu>
          </DropdownMenuTrigger>

          <DropdownMenuTrigger>
            <Button
              size="icon"
              variant="ghost"
              className="codex-composer-icon-button"
              aria-label="Tools and apps"
              isDisabled={disabled || running}
            >
              <Wrench aria-hidden="true" />
            </Button>
            <DropdownMenu placement="top start" className="w-80" isNonModal>
              <DropdownMenuLabel>Native tools</DropdownMenuLabel>
              {capabilities?.tools.length ? (
                capabilities.tools.map((tool) => (
                  <DropdownMenuItem key={tool.id} id={`tool-${tool.id}`} isDisabled>
                    <Wrench aria-hidden="true" />
                    {tool.name} · {tool.available_for_web_turn ? 'Available' : tool.reason}
                  </DropdownMenuItem>
                ))
              ) : (
                <DropdownMenuItem isDisabled>No daemon tool capabilities reported</DropdownMenuItem>
              )}
              <DropdownMenuSeparator />
              <DropdownMenuItem isDisabled>
                Plugins/apps · {capabilities?.plugins.reason ?? 'capability discovery unavailable'}
              </DropdownMenuItem>
              <DropdownMenuItem isDisabled>
                Approvals · {capabilities?.approvals.reason ?? 'capability discovery unavailable'}
              </DropdownMenuItem>
            </DropdownMenu>
          </DropdownMenuTrigger>

          <DropdownMenuTrigger>
            <Button
              size="icon"
              variant="ghost"
              className="codex-composer-icon-button"
              aria-label="Commands and mentions"
              isDisabled={disabled || running}
            >
              <Command aria-hidden="true" />
            </Button>
            <DropdownMenu placement="top start" className="w-72" isNonModal>
              <DropdownMenuLabel>Structured capabilities</DropdownMenuLabel>
              <DropdownMenuItem isDisabled>Slash commands · native endpoint unavailable</DropdownMenuItem>
              <DropdownMenuItem isDisabled>Mentions · native endpoint unavailable</DropdownMenuItem>
              <DropdownMenuItem isDisabled>Queue / steer · native endpoint unavailable</DropdownMenuItem>
              <DropdownMenuSeparator />
              <DropdownMenuItem isDisabled>
                <Search aria-hidden="true" />
                Search · {capabilities?.search.reason ?? 'capability discovery unavailable'}
              </DropdownMenuItem>
              <DropdownMenuItem isDisabled>
                <Search aria-hidden="true" />
                Deep research · {capabilities?.deep_research.reason ?? 'capability discovery unavailable'}
              </DropdownMenuItem>
            </DropdownMenu>
          </DropdownMenuTrigger>

          <Select
            selectedKey={selectedModel || null}
            onSelectionChange={(key) => {
              if (key != null) onModelChange(String(key))
            }}
            isDisabled={modelItems.length === 0 || running}
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
              aria-label={`Reasoning effort: ${effortLevels.find((level) => level.value === effort)?.label}`}
              isDisabled={running}
            >
              <SlidersHorizontal aria-hidden="true" className="size-3.5" />
              {effortLevels.find((level) => level.value === effort)?.label}
            </Button>
            <DropdownMenu placement="top start" selectionMode="single" selectedKeys={[effort]} isNonModal>
              <DropdownMenuLabel>Reasoning effort</DropdownMenuLabel>
              <DropdownMenuSeparator />
              {effortLevels.map((level) => (
                <DropdownMenuItem
                  key={level.value}
                  id={level.value}
                  onAction={() => {
                    setEffort(level.value)
                    onReasoningEffortChange?.(level.value)
                  }}
                >
                  {level.label}
                </DropdownMenuItem>
              ))}
            </DropdownMenu>
          </DropdownMenuTrigger>
        </div>

        <div className="codex-composer-actions">
          <p id="composer-help" className="sr-only">
            Enter sends from a text paragraph. Shift plus Enter inserts a new line. Enter inside a code block inserts a new line. Drafts are stored locally per chat.
            {attachmentUnavailableReason ? ` ${attachmentUnavailableReason}` : ''}
          </p>
          <p id="composer-draft-status" className="sr-only" aria-live="polite">
            {draftError}
          </p>

          <Button
            size="icon"
            variant="ghost"
            className="codex-composer-icon-button"
            aria-label={`Voice input unavailable: ${capabilities?.voice.reason ?? 'native audio adapter unavailable'}`}
            title={capabilities?.voice.reason ?? 'Native audio adapter unavailable'}
            isDisabled
          >
            <Mic aria-hidden="true" />
          </Button>

          {running ? (
            <Button
              size="icon"
              className="codex-send-button"
              aria-label="Stop generating"
              onPress={onStop}
            >
              <Square aria-hidden="true" />
            </Button>
          ) : (
            <Button
              size="icon"
              className="codex-send-button"
              aria-label="Send message"
              onPress={() => void submit()}
              isDisabled={
                disabled ||
                submitting ||
                Boolean(draftError) ||
                attachments.length > 0 ||
                draftText.trim().length === 0
              }
            >
              <ArrowUp aria-hidden="true" />
            </Button>
          )}
        </div>
      </div>
    </div>
  )
}
