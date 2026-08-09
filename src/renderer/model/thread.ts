import type { Thread, StyledRun, TextStyle } from './types'
import { generateId } from './document'

export function createThread(defaultStyle: TextStyle): Thread {
  return {
    id: generateId('thread'),
    runs: [],
    defaultStyle: { ...defaultStyle }
  }
}

export function getPlainText(thread: Thread): string {
  return thread.runs.map(r => r.text).join('')
}

export function getRunAtOffset(
  thread: Thread,
  offset: number
): { runIndex: number; localOffset: number } {
  let pos = 0
  for (let i = 0; i < thread.runs.length; i++) {
    const len = thread.runs[i].text.length
    if (offset <= pos + len) {
      return { runIndex: i, localOffset: offset - pos }
    }
    pos += len
  }
  return {
    runIndex: Math.max(0, thread.runs.length - 1),
    localOffset: thread.runs.length > 0
      ? thread.runs[thread.runs.length - 1].text.length
      : 0
  }
}

export function splitRunAtOffset(
  runs: StyledRun[],
  offset: number
): { before: StyledRun[]; after: StyledRun[] } {
  const before: StyledRun[] = []
  const after: StyledRun[] = []
  let pos = 0

  for (const run of runs) {
    const runEnd = pos + run.text.length
    if (runEnd <= offset) {
      before.push(run)
    } else if (pos >= offset) {
      after.push(run)
    } else {
      // Split this run
      const splitAt = offset - pos
      before.push({ text: run.text.slice(0, splitAt), style: { ...run.style } })
      after.push({ text: run.text.slice(splitAt), style: { ...run.style } })
    }
    pos = runEnd
  }

  return { before, after }
}

function mergeAdjacentRuns(runs: StyledRun[]): StyledRun[] {
  if (runs.length === 0) return []
  const result: StyledRun[] = [runs[0]]
  for (let i = 1; i < runs.length; i++) {
    const prev = result[result.length - 1]
    const curr = runs[i]
    if (stylesEqual(prev.style, curr.style)) {
      result[result.length - 1] = {
        text: prev.text + curr.text,
        style: prev.style
      }
    } else {
      result.push(curr)
    }
  }
  return result.filter(r => r.text.length > 0)
}

function stylesEqual(a: TextStyle, b: TextStyle): boolean {
  return JSON.stringify(a) === JSON.stringify(b)
}

export function insertText(
  thread: Thread,
  offset: number,
  text: string,
  style?: TextStyle
): Thread {
  const resolvedStyle = style
    ? { ...thread.defaultStyle, ...style }
    : { ...thread.defaultStyle }

  const { before, after } = splitRunAtOffset(thread.runs, offset)
  const newRun: StyledRun = { text, style: resolvedStyle }
  const runs = mergeAdjacentRuns([...before, newRun, ...after])

  return { ...thread, runs }
}

export function deleteText(
  thread: Thread,
  from: number,
  to: number
): Thread {
  const { before } = splitRunAtOffset(thread.runs, from)
  const { after } = splitRunAtOffset(thread.runs, to)
  const runs = mergeAdjacentRuns([...before, ...after])
  return { ...thread, runs }
}

export function applyStyle(
  thread: Thread,
  from: number,
  to: number,
  style: Partial<TextStyle>
): Thread {
  const { before, after: rest } = splitRunAtOffset(thread.runs, from)
  const { before: middle, after } = splitRunAtOffset(rest, to - from)

  const styled = middle.map(run => ({
    text: run.text,
    style: { ...run.style, ...style }
  }))

  const runs = mergeAdjacentRuns([...before, ...styled, ...after])
  return { ...thread, runs }
}
