import { memo, useMemo, useState, type ReactNode } from 'react'
import { Check, Copy } from 'lucide-react'

/**
 * Safe, dependency-free conversation markdown: only React text nodes and
 * allowlisted elements are rendered. Raw HTML is never interpreted, and
 * arbitrary URL schemes are not turned into links.
 */
function inline(text: string): ReactNode[] {
  const nodes: ReactNode[] = []
  const matcher = /(`[^`\n]+`|\*\*[^*\n]+\*\*|\*[^*\n]+\*|\[[^\]]+\]\(https?:\/\/[^\s)]+\))/g
  let cursor = 0
  let index = 0
  for (const match of text.matchAll(matcher)) {
    const start = match.index ?? 0
    if (start > cursor) nodes.push(text.slice(cursor, start))
    const value = match[0]
    if (value.startsWith('`')) {
      nodes.push(<code key={index++}>{value.slice(1, -1)}</code>)
    } else if (value.startsWith('**')) {
      nodes.push(<strong key={index++}>{value.slice(2, -2)}</strong>)
    } else if (value.startsWith('*')) {
      nodes.push(<em key={index++}>{value.slice(1, -1)}</em>)
    } else {
      const split = value.indexOf('](')
      const url = value.slice(split + 2, -1)
      nodes.push(<a href={url} target="_blank" rel="noopener noreferrer" key={index++}>{value.slice(1, split)}</a>)
    }
    cursor = start + value.length
  }
  if (cursor < text.length) nodes.push(text.slice(cursor))
  return nodes
}

function CodeFence({ code, language }: { code: string; language: string }) {
  const [copied, setCopied] = useState(false)
  return (
    <div className="codex-markdown-code">
      <div className="codex-markdown-codebar">
        <span>{language || 'text'}</span>
        <button type="button" onClick={() => {
          void navigator.clipboard?.writeText(code).then(() => {
            setCopied(true)
          }).catch(() => setCopied(false))
        }} aria-label={`Copy ${language || 'text'} code`}>
          {copied ? <Check size={13} aria-hidden /> : <Copy size={13} aria-hidden />}
          {copied ? 'Copied' : 'Copy'}
        </button>
      </div>
      <pre><code className={language && /^[a-z0-9_+-]{1,35}$/i.test(language) ? `language-${language}` : undefined}>{code}</code></pre>
    </div>
  )
}

function isTableSeparator(row: string) {
  return /^\s*\|?\s*:?-{3,}:?\s*(?:\|\s*:?-{3,}:?\s*)+\|?\s*$/.test(row)
}

function tableCells(row: string) {
  return row.trim().replace(/^\|/, '').replace(/\|$/, '').split('|').map((cell) => cell.trim())
}

function parseMarkdown(text: string): ReactNode[] {
  const lines = text.replace(/\r\n?/g, '\n').split('\n')
  const blocks: ReactNode[] = []
  let i = 0
  let id = 0
  const isSpecial = (line: string) =>
    /^\s*```|^#{1,6} |^[-*+] |^\d+[.)] |^> |^---+$|^\|/.test(line)
  while (i < lines.length) {
    const line = lines[i]
    if (!line.trim()) { i++; continue }

    const fence = /^\s*```([^\s`]*)\s*$/.exec(line)
    if (fence) {
      i++
      const code: string[] = []
      while (i < lines.length && !/^\s*```\s*$/.test(lines[i])) code.push(lines[i++])
      if (i < lines.length) i++
      blocks.push(<CodeFence key={id++} code={code.join('\n')} language={fence[1].slice(0, 35)} />)
      continue
    }

    const heading = /^(#{1,6})\s+(.+)$/.exec(line)
    if (heading) {
      const level = heading[1].length
      const content = inline(heading[2])
      blocks.push(level <= 2 ? <h2 key={id++}>{content}</h2>
        : level === 3 ? <h3 key={id++}>{content}</h3>
          : <h4 key={id++}>{content}</h4>)
      i++
      continue
    }

    if (/^---+$/.test(line.trim())) { blocks.push(<hr key={id++} />); i++; continue }
    if (i + 1 < lines.length && line.includes('|') && isTableSeparator(lines[i + 1])) {
      const headers = tableCells(line)
      i += 2
      const rows: string[][] = []
      while (i < lines.length && lines[i].trim() && lines[i].includes('|')) rows.push(tableCells(lines[i++]))
      blocks.push(
        <div className="codex-markdown-table-scroll" key={id++}>
          <table>
            <thead><tr>{headers.map((cell, j) => <th key={j}>{inline(cell)}</th>)}</tr></thead>
            <tbody>{rows.map((row, j) => <tr key={j}>{headers.map((_, k) => <td key={k}>{inline(row[k] ?? '')}</td>)}</tr>)}</tbody>
          </table>
        </div>,
      )
      continue
    }
    if (/^> /.test(line)) {
      const quote: string[] = []
      while (i < lines.length && /^> /.test(lines[i])) quote.push(lines[i++].slice(2))
      blocks.push(<blockquote key={id++}>{inline(quote.join(' '))}</blockquote>)
      continue
    }
    if (/^[-*+] /.test(line)) {
      const items: string[] = []
      while (i < lines.length && /^[-*+] /.test(lines[i])) items.push(lines[i++].slice(2))
      blocks.push(<ul key={id++}>{items.map((item, j) => <li key={j}>{inline(item)}</li>)}</ul>)
      continue
    }
    if (/^\d+[.)] /.test(line)) {
      const items: string[] = []
      const start = Number(/^\d+/.exec(line)?.[0]) || 1
      while (i < lines.length && /^\d+[.)] /.test(lines[i])) items.push(lines[i++].replace(/^\d+[.)] /, ''))
      blocks.push(<ol key={id++} start={start}>{items.map((item, j) => <li key={j}>{inline(item)}</li>)}</ol>)
      continue
    }
    const paragraph = [line]
    i++
    while (i < lines.length && lines[i].trim() && !isSpecial(lines[i])) paragraph.push(lines[i++])
    blocks.push(<p key={id++}>{inline(paragraph.join(' '))}</p>)
  }
  return blocks
}

export const MessageMarkdown = memo(function MessageMarkdown({ text }: { text: string }) {
  const blocks = useMemo(() => parseMarkdown(text), [text])
  return <div className="codex-markdown">{blocks}</div>
})
