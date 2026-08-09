// tests/input/selection.test.ts
import { describe, it, expect } from 'vitest'
import {
  createTextSelection,
  isCollapsed,
  getSelectionRange,
  expandToWord,
  expandToParagraph
} from '@input/selection'

describe('TextSelection', () => {
  it('creates a collapsed selection', () => {
    const sel = createTextSelection('thread-1', 5, 5)
    expect(isCollapsed(sel)).toBe(true)
  })

  it('reports non-collapsed for range selection', () => {
    const sel = createTextSelection('thread-1', 5, 10)
    expect(isCollapsed(sel)).toBe(false)
  })

  it('normalizes range (start <= end)', () => {
    const sel = createTextSelection('thread-1', 10, 5)
    const range = getSelectionRange(sel)
    expect(range.start).toBe(5)
    expect(range.end).toBe(10)
  })
})

describe('expandToWord', () => {
  it('selects the word at offset', () => {
    const text = 'Hello world test'
    const result = expandToWord(text, 7)
    expect(result).toEqual({ start: 6, end: 11 }) // "world"
  })
})

describe('expandToParagraph', () => {
  it('selects the paragraph at offset', () => {
    const text = 'First paragraph.\nSecond paragraph.\nThird.'
    const result = expandToParagraph(text, 20)
    expect(result).toEqual({ start: 17, end: 34 }) // "Second paragraph."
  })
})
