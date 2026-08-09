// src/renderer/engine/frame-layout.ts
import { prepareWithSegments, layoutNextLine } from '@chenglou/pretext'
import type { StyledRun, Rect, TextStyle } from '@model/types'
import type { LayoutLine, ExclusionZone } from './layout-types'
import { getAvailableWidthForLine, getSkipRange } from './wrap-calculator'

interface FrameLayoutInput {
  runs: StyledRun[]
  frameRect: Rect
  lineHeight: number
  exclusions: ExclusionZone[]
  startOffset: number // character offset into the thread to start from
  padding?: number
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
    lineHeight,
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

  // Determine font from first run (simplified — in production, handle multi-style)
  const firstStyle = runs.length > 0 ? runs[0].style : {}
  const font = `${firstStyle.fontSize ?? 14}px ${firstStyle.fontFamily ?? 'sans-serif'}`

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

    result.push({
      text,
      style: run.style,
      x,
      width: 0 // will be computed by renderer using measureText
    })

    x += text.length * ((run.style.fontSize ?? 14) * 0.6) // rough estimate
  }

  return result
}
