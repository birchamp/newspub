// src/renderer/canvas/text-painter.ts
import type { LayoutLine } from '@engine/layout-types'
import type { TextStyle } from '@model/types'
import { lineAlignOffset } from '@engine/alignment'

export function buildFontString(style: TextStyle): string {
  const weight = style.bold ? 'bold' : 'normal'
  const slant = style.italic ? 'italic' : 'normal'
  const size = style.fontSize ?? 14
  const family = style.fontFamily ?? 'sans-serif'
  return `${slant} ${weight} ${size}px ${family}`
}

export function paintTextLines(
  ctx: CanvasRenderingContext2D,
  lines: LayoutLine[],
  frameX: number,
  frameY: number,
  frameWidth: number
): void {
  for (const line of lines) {
    const align = lineAlignOffset(line, frameWidth)
    if (line.runStyles.length > 0) {
      // Paint runs sequentially with real measured advances.
      let x = frameX + line.x + align
      for (const runStyle of line.runStyles) {
        ctx.font = buildFontString(runStyle.style)
        ctx.fillStyle = runStyle.style.color ?? '#000000'
        ctx.fillText(runStyle.text, x, frameY + line.y + line.height * 0.8)
        x += ctx.measureText(runStyle.text).width
      }
    } else {
      // Fallback: draw whole line
      ctx.fillStyle = '#000000'
      ctx.fillText(line.text, frameX + line.x + align, frameY + line.y + line.height * 0.8)
    }
  }
}

/** Measured x-advance of the first `chars` characters of a line (line-local px) */
export function measureLinePrefix(
  ctx: CanvasRenderingContext2D,
  line: LayoutLine,
  chars: number
): number {
  let remaining = Math.max(0, Math.min(chars, line.text.length))
  let x = 0
  ctx.save()
  if (line.runStyles.length > 0) {
    for (const runStyle of line.runStyles) {
      if (remaining <= 0) break
      ctx.font = buildFontString(runStyle.style)
      const take = Math.min(remaining, runStyle.text.length)
      x += ctx.measureText(runStyle.text.slice(0, take)).width
      remaining -= take
    }
  } else {
    x = ctx.measureText(line.text.slice(0, remaining)).width
  }
  ctx.restore()
  return x
}

/** Character index within a line closest to the given line-local x position */
export function characterIndexAtX(
  ctx: CanvasRenderingContext2D,
  line: LayoutLine,
  targetX: number
): number {
  if (targetX <= 0) return 0
  let prev = 0
  for (let i = 1; i <= line.text.length; i++) {
    const w = measureLinePrefix(ctx, line, i)
    if (w >= targetX) {
      // Snap to whichever side of the character is closer
      return targetX - prev <= w - targetX ? i - 1 : i
    }
    prev = w
  }
  return line.text.length
}

export function paintContinuationMarker(
  ctx: CanvasRenderingContext2D,
  text: string,
  x: number,
  y: number
): void {
  ctx.save()
  ctx.font = 'italic 10px sans-serif'
  ctx.fillStyle = '#888888'
  ctx.fillText(text, x, y)
  ctx.restore()
}

export function paintOverflowIndicator(
  ctx: CanvasRenderingContext2D,
  x: number,
  y: number,
  size: number = 12
): void {
  ctx.save()
  ctx.fillStyle = '#ef4444'
  ctx.beginPath()
  ctx.arc(x, y, size / 2, 0, Math.PI * 2)
  ctx.fill()
  ctx.fillStyle = '#ffffff'
  ctx.font = `bold ${size - 2}px sans-serif`
  ctx.textAlign = 'center'
  ctx.textBaseline = 'middle'
  ctx.fillText('+', x, y)
  ctx.restore()
}
