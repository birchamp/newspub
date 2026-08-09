// src/renderer/input/clipboard.ts
import type { StyledRun, TextStyle } from '@model/types'

// Lightweight HTML → runs parser. Handles <b>/<strong>, <i>/<em>, line
// breaks, and basic entities without relying on DOMParser so it behaves
// identically in the renderer and in tests.
export function parseHtmlToRuns(html: string): StyledRun[] {
  const runs: StyledRun[] = []
  const styleStack: TextStyle[] = [{}]
  let boldDepth = 0
  let italicDepth = 0

  // Drop non-content blocks entirely
  const cleaned = html
    .replace(/<!--[\s\S]*?-->/g, '')
    .replace(/<(style|script|head)[^>]*>[\s\S]*?<\/\1>/gi, '')

  const tokenRe = /<\/?[a-zA-Z][^>]*>/g
  let lastIndex = 0

  const pushText = (raw: string) => {
    if (!raw) return
    const text = decodeEntities(raw)
    if (text.length === 0) return
    const style: TextStyle = {}
    if (boldDepth > 0) style.bold = true
    if (italicDepth > 0) style.italic = true
    const prev = runs[runs.length - 1]
    if (prev && !!prev.style.bold === !!style.bold && !!prev.style.italic === !!style.italic) {
      runs[runs.length - 1] = { text: prev.text + text, style: prev.style }
    } else {
      runs.push({ text, style })
    }
  }

  let match: RegExpExecArray | null
  while ((match = tokenRe.exec(cleaned)) !== null) {
    pushText(cleaned.slice(lastIndex, match.index))
    lastIndex = tokenRe.lastIndex

    const token = match[0]
    const isClosing = token.startsWith('</')
    const tag = token.replace(/^<\/?\s*/, '').replace(/[\s/>].*$/s, '').toLowerCase()

    if (tag === 'b' || tag === 'strong') {
      boldDepth = Math.max(0, boldDepth + (isClosing ? -1 : 1))
    } else if (tag === 'i' || tag === 'em') {
      italicDepth = Math.max(0, italicDepth + (isClosing ? -1 : 1))
    } else if (tag === 'br' && !isClosing) {
      pushText('\n')
    } else if ((tag === 'p' || tag === 'div') && isClosing) {
      pushText('\n')
    }
  }
  pushText(cleaned.slice(lastIndex))

  // keep styleStack referenced out of the loop for future nested-style support
  void styleStack

  if (runs.length === 0) {
    const stripped = decodeEntities(cleaned.replace(/<[^>]*>/g, ''))
    return [{ text: stripped, style: {} }]
  }
  return runs
}

function decodeEntities(text: string): string {
  return text
    .replace(/&nbsp;/g, ' ')
    .replace(/&lt;/g, '<')
    .replace(/&gt;/g, '>')
    .replace(/&quot;/g, '"')
    .replace(/&#0?39;/g, "'")
    .replace(/&amp;/g, '&')
}

export function runsToPlainText(runs: StyledRun[]): string {
  return runs.map(r => r.text).join('')
}

export function runsToHtml(runs: StyledRun[]): string {
  return runs.map(run => {
    let text = escapeHtml(run.text)
    if (run.style.bold) text = `<b>${text}</b>`
    if (run.style.italic) text = `<i>${text}</i>`
    return text
  }).join('')
}

function escapeHtml(text: string): string {
  return text
    .replace(/&/g, '&amp;')
    .replace(/</g, '&lt;')
    .replace(/>/g, '&gt;')
}
