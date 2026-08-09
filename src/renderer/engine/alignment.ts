// src/renderer/engine/alignment.ts
import type { LayoutLine } from './layout-types'

const FRAME_PADDING = 8

/**
 * Horizontal offset to apply to a laid-out line for its paragraph alignment.
 * Pure — usable by the canvas painter, caret math, and the PDF exporter.
 */
export function lineAlignOffset(line: LayoutLine, frameWidth: number, padding: number = FRAME_PADDING): number {
  const alignment = line.runStyles[0]?.style.alignment ?? 'left'
  if (alignment === 'left' || alignment === 'justify') return 0
  const avail = frameWidth - padding * 2
  const slack = avail - line.width
  if (slack <= 0) return 0
  return alignment === 'center' ? slack / 2 : slack
}
