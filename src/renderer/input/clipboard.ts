// src/renderer/input/clipboard.ts
import type { StyledRun, TextStyle } from '@model/types'

export function parseHtmlToRuns(html: string): StyledRun[] {
  // Simple parser — handles <b>, <strong>, <i>, <em>, <span style="...">
  // Uses DOMParser when available (Electron renderer has it)
  if (typeof DOMParser === 'undefined') {
    return [{ text: html.replace(/<[^>]*>/g, ''), style: {} }]
  }

  const parser = new DOMParser()
  const doc = parser.parseFromString(html, 'text/html')
  const runs: StyledRun[] = []

  function walk(node: Node, inheritedStyle: TextStyle): void {
    if (node.nodeType === Node.TEXT_NODE) {
      const text = node.textContent ?? ''
      if (text.length > 0) {
        runs.push({ text, style: { ...inheritedStyle } })
      }
      return
    }

    if (node.nodeType !== Node.ELEMENT_NODE) return
    const el = node as Element
    const style = { ...inheritedStyle }

    const tag = el.tagName.toLowerCase()
    if (tag === 'b' || tag === 'strong') style.bold = true
    if (tag === 'i' || tag === 'em') style.italic = true

    for (const child of Array.from(node.childNodes)) {
      walk(child, style)
    }
  }

  walk(doc.body, {})
  return runs.length > 0 ? runs : [{ text: html, style: {} }]
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
