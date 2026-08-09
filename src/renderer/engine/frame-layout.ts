// src/renderer/engine/frame-layout.ts
import { prepareWithSegments, layoutNextLine } from '@chenglou/pretext'
import type { StyledRun, Rect, TextStyle } from '@model/types'
import type { LayoutLine, ExclusionZone } from './layout-types'
import { getAvailableWidthForLine, getSkipRange } from './wrap-calculator'

interface FrameLayoutInput {
  runs: StyledRun[]
  frameRect: Rect
  lineHeight?: number  // omit to derive from the frame's starting font size
  exclusions: ExclusionZone[]
  startOffset: number // character offset into the thread to start from
  padding?: number
}

/** Style of the run containing the given offset (or the last run) */
function styleAtOffset(runs: StyledRun[], offset: number): TextStyle {
  let pos = 0
  for (const run of runs) {
    pos += run.text.length
    if (offset < pos) return run.style
  }
  return runs.length > 0 ? runs[runs.length - 1].style : {}
}

// Shared measuring context: real canvas metrics in the renderer, a rough
// estimate in DOM-free environments (unit tests).
let measureCanvasCtx: CanvasRenderingContext2D | null | undefined
function getMeasureCtx(): CanvasRenderingContext2D | null {
  if (measureCanvasCtx === undefined) {
    measureCanvasCtx =
      typeof document !== 'undefined'
        ? document.createElement('canvas').getContext('2d')
        : null
  }
  return measureCanvasCtx
}

function measureRun(text: string, style: TextStyle): number {
  const ctx = getMeasureCtx()
  if (ctx) {
    const weight = style.bold ? 'bold' : 'normal'
    const slant = style.italic ? 'italic' : 'normal'
    ctx.font = `${slant} ${weight} ${style.fontSize ?? 14}px ${style.fontFamily ?? 'sans-serif'}`
    return ctx.measureText(text).width
  }
  return text.length * ((style.fontSize ?? 14) * 0.6) // rough estimate
}

interface FrameLayoutResult {
  lines: LayoutLine[]
  endOffset: number    // character offset where we stopped
  overflow: boolean    // true if text remains but frame is full
}

export function layoutFrameText(input: FrameLayoutInput): FrameLayoutResult {
  const {
    runs,
    frameRect,
    exclusions,
    startOffset,
    padding = 8
  } = input

  const lines: LayoutLine[] = []
  const availableHeight = frameRect.height - padding * 2
  const baseWidth = frameRect.width - padding * 2

  // Build the full text from runs starting at startOffset
  const fullText = runs.map(r => r.text).join('')
  const textToLayout = fullText.slice(startOffset)

  if (textToLayout.length === 0) {
    return { lines: [], endOffset: startOffset, overflow: false }
  }

  // Font and line height follow the style where this frame's text starts, so
  // a headline frame with 28pt text wraps and spaces correctly. (Mixed sizes
  // within one frame still approximate using the starting style.)
  const startStyle = styleAtOffset(runs, startOffset)
  const weight = startStyle.bold ? 'bold ' : ''
  const font = `${weight}${startStyle.fontSize ?? 14}px ${startStyle.fontFamily ?? 'sans-serif'}`
  const lineHeight = input.lineHeight ?? Math.round((startStyle.fontSize ?? 14) * 1.4)

  const prepared = prepareWithSegments(textToLayout, font)
  const skipRange = getSkipRange(baseWidth, availableHeight, exclusions)

  let y = padding
  let cursor: any = { segmentIndex: 0, graphemeIndex: 0 }
  let charOffset = startOffset

  while (y + lineHeight <= availableHeight + padding) {
    // Check skip range
    if (skipRange && y >= skipRange.yStart && y < skipRange.yEnd) {
      y = skipRange.yEnd
      continue
    }

    const lineWidth = getAvailableWidthForLine(baseWidth, y - padding, lineHeight, exclusions)

    const line = layoutNextLine(prepared, cursor, lineWidth)
    if (line === null) break

    const lineChars = line.text.length
    const runStyles = resolveRunStyles(runs, charOffset, charOffset + lineChars, padding)

    lines.push({
      text: line.text,
      width: line.width,
      x: padding,
      y,
      height: lineHeight,
      runStyles
    })

    cursor = line.end
    charOffset += lineChars
    y += lineHeight
  }

  // Check if there's more text that didn't fit
  const overflow = charOffset < fullText.length

  return { lines, endOffset: charOffset, overflow }
}

function resolveRunStyles(
  runs: StyledRun[],
  from: number,
  to: number,
  xOffset: number
): LayoutLine['runStyles'] {
  // Simplified: return one entry per run that overlaps the range
  const result: LayoutLine['runStyles'] = []
  let pos = 0
  let x = xOffset

  for (const run of runs) {
    const runStart = pos
    const runEnd = pos + run.text.length
    pos = runEnd

    if (runEnd <= from || runStart >= to) continue

    const sliceStart = Math.max(from, runStart) - runStart
    const sliceEnd = Math.min(to, runEnd) - runStart
    const text = run.text.slice(sliceStart, sliceEnd)

    const width = measureRun(text, run.style)
    result.push({
      text,
      style: run.style,
      x,
      width
    })

    x += width
  }

  return result
}
