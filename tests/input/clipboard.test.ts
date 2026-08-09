// tests/input/clipboard.test.ts
import { describe, it, expect } from 'vitest'
import { parseHtmlToRuns, runsToHtml, runsToPlainText } from '@input/clipboard'
import type { StyledRun, TextStyle } from '@model/types'

describe('parseHtmlToRuns', () => {
  it('parses plain text', () => {
    const runs = parseHtmlToRuns('Hello world')
    expect(runs).toHaveLength(1)
    expect(runs[0].text).toBe('Hello world')
  })

  it('parses bold text', () => {
    const runs = parseHtmlToRuns('Hello <b>bold</b> world')
    expect(runs.length).toBeGreaterThanOrEqual(2)
    const boldRun = runs.find(r => r.text.trim() === 'bold')
    expect(boldRun?.style.bold).toBe(true)
  })

  it('parses italic text', () => {
    const runs = parseHtmlToRuns('Hello <i>italic</i> world')
    const italicRun = runs.find(r => r.text.trim() === 'italic')
    expect(italicRun?.style.italic).toBe(true)
  })
})

describe('runsToPlainText', () => {
  it('concatenates run text', () => {
    const runs: StyledRun[] = [
      { text: 'Hello ', style: {} },
      { text: 'world', style: { bold: true } }
    ]
    expect(runsToPlainText(runs)).toBe('Hello world')
  })
})

describe('runsToHtml', () => {
  it('wraps bold runs in <b> tags', () => {
    const runs: StyledRun[] = [
      { text: 'Hello ', style: {} },
      { text: 'bold', style: { bold: true } }
    ]
    const html = runsToHtml(runs)
    expect(html).toContain('<b>bold</b>')
  })
})
