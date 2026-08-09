// src/renderer/canvas/text-painter.ts
import type { LayoutLine } from '@engine/layout-types'
import type { TextStyle } from '@model/types'

function buildFontString(style: TextStyle): string {
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
  frameY: number
): void {
  for (const line of lines) {
    if (line.runStyles.length > 0) {
      for (const runStyle of line.runStyles) {
        ctx.font = buildFontString(runStyle.style)
        ctx.fillStyle = runStyle.style.color ?? '#000000'
        ctx.fillText(runStyle.text, frameX + runStyle.x, frameY + line.y + line.height * 0.8)
      }
    } else {
      // Fallback: draw whole line
      ctx.fillStyle = '#000000'
      ctx.fillText(line.text, frameX + line.x, frameY + line.y + line.height * 0.8)
    }
  }
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
