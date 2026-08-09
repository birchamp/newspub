import { describe, it, expect } from 'vitest'
import {
  createThread,
  insertText,
  deleteText,
  applyStyle,
  getPlainText,
  splitRunAtOffset,
  getRunAtOffset
} from '@model/thread'
import type { Thread, TextStyle } from '@model/types'

describe('createThread', () => {
  it('creates an empty thread with default style', () => {
    const thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    expect(thread.runs).toEqual([])
    expect(thread.defaultStyle.fontFamily).toBe('Inter')
  })
})

describe('insertText', () => {
  it('inserts text into an empty thread', () => {
    const thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    const updated = insertText(thread, 0, 'Hello world')
    expect(getPlainText(updated)).toBe('Hello world')
    expect(updated.runs).toHaveLength(1)
  })

  it('inserts text in the middle of a run', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Helloworld')
    thread = insertText(thread, 5, ' ')
    expect(getPlainText(thread)).toBe('Hello world')
  })

  it('inserts styled runs from paste', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello ')
    thread = insertText(thread, 6, 'bold', { bold: true })
    expect(getPlainText(thread)).toBe('Hello bold')
    expect(thread.runs[1].style.bold).toBe(true)
  })
})

describe('deleteText', () => {
  it('deletes a range of text within a single run', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello world')
    thread = deleteText(thread, 5, 11)
    expect(getPlainText(thread)).toBe('Hello')
  })

  it('deletes across run boundaries', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello ')
    thread = insertText(thread, 6, 'bold', { bold: true })
    thread = insertText(thread, 10, ' end')
    // "Hello bold end" → delete "lo bold e" → "Helnd"
    thread = deleteText(thread, 3, 12)
    expect(getPlainText(thread)).toBe('Helnd')
  })
})

describe('applyStyle', () => {
  it('applies bold to a range', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello world')
    thread = applyStyle(thread, 6, 11, { bold: true })
    expect(thread.runs.length).toBeGreaterThanOrEqual(2)
    // "Hello " is not bold, "world" is bold
    const worldRun = thread.runs.find(r => r.text === 'world')
    expect(worldRun?.style.bold).toBe(true)
  })
})

describe('getRunAtOffset', () => {
  it('returns the run and local offset for a global offset', () => {
    let thread = createThread({ fontFamily: 'Inter', fontSize: 12 })
    thread = insertText(thread, 0, 'Hello ')
    thread = insertText(thread, 6, 'world')
    const result = getRunAtOffset(thread, 8)
    expect(result.runIndex).toBe(1)
    expect(result.localOffset).toBe(2) // "wo|rld"
  })
})
